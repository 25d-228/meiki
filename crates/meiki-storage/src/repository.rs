use std::collections::{HashMap, HashSet};

use meiki_domain::{
    Annotation, Card, CardLifecycle, Cloze, CollectionSchedulingSettings, Deck, LocalizedText,
    MediaKind, MediaReference, MediaRole, ScheduleState, SchedulerParameterSet, SchedulerProfile,
    SchedulingMode, SegmentContent, SemanticSegment, SourceItem, Tag,
};
use rusqlite::{
    Connection, OptionalExtension, Transaction, params, params_from_iter, types::Value,
};

use crate::{
    DEFAULT_DECK_ID, DEFAULT_SCHEDULER_PARAMETER_SET_ID, DeckCardCounts, InstalledBundle,
    PristineBundleImport, PristineBundleImportError, PristineBundleImportPlan, PristineDeckImport,
    PristineDeckImportStatus, SchedulingWorkload, Storage, StorageError, StoredDeckCard,
    StoredDeckCardPage, StoredDeckCardSearch, StoredLibraryCard, StoredLibraryNote,
    StoredSourceNote, StoredStudyCard, direction_from_database, direction_to_database,
    entity_not_found, matching_policy_from_database, matching_policy_to_database,
};

const MAXIMUM_RESPONSE_DURATION_SAMPLES: usize = 1_024;

/// Persistence operations for mutable decks.
///
/// # Errors
///
/// Methods return [`StorageError`] when validation, lookup, or persistence
/// fails.
#[allow(clippy::missing_errors_doc)]
pub trait DeckRepository {
    fn create_deck(&mut self, deck: &Deck) -> Result<(), StorageError>;
    fn get_deck(&self, id: &str) -> Result<Deck, StorageError>;
    fn list_decks(&self) -> Result<Vec<Deck>, StorageError>;
    fn update_deck(&mut self, deck: &Deck) -> Result<(), StorageError>;
    fn delete_deck(&mut self, id: &str) -> Result<(), StorageError>;
}

/// Persistence operations for mutable tags.
///
/// # Errors
///
/// Methods return [`StorageError`] when validation, lookup, or persistence
/// fails.
#[allow(clippy::missing_errors_doc)]
pub trait TagRepository {
    fn create_tag(&mut self, tag: &Tag) -> Result<(), StorageError>;
    fn get_tag(&self, id: &str) -> Result<Tag, StorageError>;
    fn list_tags(&self) -> Result<Vec<Tag>, StorageError>;
    fn update_tag(&mut self, tag: &Tag) -> Result<(), StorageError>;
    fn delete_tag(&mut self, id: &str) -> Result<(), StorageError>;
}

/// Persistence operations for generic annotations.
///
/// # Errors
///
/// Methods return [`StorageError`] when validation, lookup, or persistence
/// fails.
#[allow(clippy::missing_errors_doc)]
pub trait AnnotationRepository {
    fn create_annotation(&mut self, annotation: &Annotation) -> Result<(), StorageError>;
    fn get_annotation(&self, id: &str) -> Result<Annotation, StorageError>;
    fn update_annotation(&mut self, annotation: &Annotation) -> Result<(), StorageError>;
    fn delete_annotation(&mut self, id: &str) -> Result<(), StorageError>;
}

/// Persistence operations for media metadata references.
///
/// # Errors
///
/// Methods return [`StorageError`] when validation, lookup, or persistence
/// fails.
#[allow(clippy::missing_errors_doc)]
pub trait MediaRepository {
    fn create_media_reference(&mut self, media: &MediaReference) -> Result<(), StorageError>;
    fn get_media_reference(&self, id: &str) -> Result<MediaReference, StorageError>;
    fn update_media_reference(&mut self, media: &MediaReference) -> Result<(), StorageError>;
    fn delete_media_reference(&mut self, id: &str) -> Result<(), StorageError>;
    fn media_reference_usage(&self, id: &str) -> Result<u64, StorageError>;
    fn media_reference_count_for_hash(&self, content_hash: &str) -> Result<u64, StorageError>;
}

/// Persistence operations for versioned scheduler parameter sets.
///
/// # Errors
///
/// Methods return [`StorageError`] when validation, lookup, or persistence
/// fails.
#[allow(clippy::missing_errors_doc)]
pub trait SchedulerParameterSetRepository {
    fn create_scheduler_parameter_set(
        &mut self,
        parameter_set: &SchedulerParameterSet,
    ) -> Result<(), StorageError>;
    fn get_scheduler_parameter_set(&self, id: &str) -> Result<SchedulerParameterSet, StorageError>;
    fn update_scheduler_parameter_set(
        &mut self,
        parameter_set: &SchedulerParameterSet,
    ) -> Result<(), StorageError>;
    fn delete_scheduler_parameter_set(&mut self, id: &str) -> Result<(), StorageError>;
}

/// Persistence operations for per-deck scheduling controls and engine state.
///
/// # Errors
///
/// Methods return [`StorageError`] when validation, lookup, or persistence
/// fails.
#[allow(clippy::missing_errors_doc)]
pub trait SchedulerProfileRepository {
    fn get_scheduler_profile(&self, deck_id: &str) -> Result<SchedulerProfile, StorageError>;
    fn update_scheduler_profile(&mut self, profile: &SchedulerProfile) -> Result<(), StorageError>;
}

/// Persistence operations for source-note aggregates.
///
/// # Errors
///
/// Methods return [`StorageError`] when validation, lookup, or persistence
/// fails.
#[allow(clippy::missing_errors_doc)]
pub trait SourceNoteRepository {
    fn create_source_note(&mut self, note: &StoredSourceNote) -> Result<(), StorageError>;
    fn get_source_note(&self, id: &str) -> Result<StoredSourceNote, StorageError>;
    fn update_source_note(&mut self, note: &StoredSourceNote) -> Result<(), StorageError>;
    fn delete_source_note(&mut self, id: &str) -> Result<(), StorageError>;
}

/// Persistence operations for clozes owned by source notes.
///
/// # Errors
///
/// Methods return [`StorageError`] when validation, lookup, or persistence
/// fails.
#[allow(clippy::missing_errors_doc)]
pub trait ClozeRepository {
    fn get_cloze(&self, id: &str) -> Result<Cloze, StorageError>;
    fn update_cloze(&mut self, cloze: &Cloze) -> Result<(), StorageError>;
    fn delete_cloze(&mut self, id: &str) -> Result<(), StorageError>;
}

/// Persistence operations for cards and their initial projections.
///
/// # Errors
///
/// Methods return [`StorageError`] when validation, lookup, or persistence
/// fails.
#[allow(clippy::missing_errors_doc)]
pub trait CardRepository {
    fn create_card(
        &mut self,
        card: &Card,
        initial_schedule: &ScheduleState,
    ) -> Result<(), StorageError>;
    fn get_card(&self, id: &str) -> Result<Card, StorageError>;
    fn get_card_for_cloze(&self, cloze_id: &str) -> Result<Card, StorageError>;
    fn update_card(&mut self, card: &Card) -> Result<(), StorageError>;
    fn delete_card(&mut self, id: &str) -> Result<(), StorageError>;
}

/// Persistence operations for adding one validated pristine archive deck.
///
/// # Errors
///
/// Methods return [`StorageError`] when imported identities collide, the
/// aggregate is invalid, or the transaction cannot be committed.
#[allow(clippy::missing_errors_doc)]
pub trait PristineDeckRepository {
    fn validate_pristine_deck_import(
        &self,
        import: &PristineDeckImport,
    ) -> Result<PristineDeckImportStatus, StorageError>;
    fn import_pristine_deck(
        &mut self,
        import: &PristineDeckImport,
    ) -> Result<PristineDeckImportStatus, StorageError>;
}

impl DeckRepository for Storage {
    fn create_deck(&mut self, deck: &Deck) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        insert_deck(&transaction, deck)?;
        insert_default_scheduler_profile(&transaction, &deck.id, deck.created_at_ms)?;
        transaction.commit()?;
        Ok(())
    }

    fn get_deck(&self, id: &str) -> Result<Deck, StorageError> {
        load_deck(&self.connection, id)
    }

    fn list_decks(&self) -> Result<Vec<Deck>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM decks ORDER BY name, id")?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| load_deck(&self.connection, id))
            .collect()
    }

    fn update_deck(&mut self, deck: &Deck) -> Result<(), StorageError> {
        let changed = self.connection.execute(
            "UPDATE decks
             SET name = ?1,
                 description = ?2,
                 language_tag = ?3,
                 direction = ?4,
                 matching_policy = ?5,
                 target_retention_basis_points = ?6,
                 new_cards_per_day = ?7,
                 maximum_interval_days = ?8,
                 updated_at_ms = ?9
             WHERE id = ?10",
            params![
                deck.name,
                deck.description,
                deck.language_tag,
                direction_to_database(deck.direction),
                matching_policy_to_database(deck.matching_policy),
                deck.settings.target_retention_basis_points,
                deck.settings.new_cards_per_day,
                deck.settings.maximum_interval_days,
                deck.updated_at_ms,
                deck.id,
            ],
        )?;
        ensure_changed(changed, "deck", &deck.id)
    }

    fn delete_deck(&mut self, id: &str) -> Result<(), StorageError> {
        let changed = self
            .connection
            .execute("DELETE FROM decks WHERE id = ?1", [id])?;
        ensure_changed(changed, "deck", id)
    }
}

impl TagRepository for Storage {
    fn create_tag(&mut self, tag: &Tag) -> Result<(), StorageError> {
        insert_tag(&self.connection, tag)
    }

    fn get_tag(&self, id: &str) -> Result<Tag, StorageError> {
        load_tag(&self.connection, id)
    }

    fn list_tags(&self) -> Result<Vec<Tag>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM tags ORDER BY name, id")?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| load_tag(&self.connection, id))
            .collect()
    }

    fn update_tag(&mut self, tag: &Tag) -> Result<(), StorageError> {
        let changed = self.connection.execute(
            "UPDATE tags
             SET name = ?1, updated_at_ms = ?2
             WHERE id = ?3",
            params![tag.name, tag.updated_at_ms, tag.id],
        )?;
        ensure_changed(changed, "tag", &tag.id)
    }

    fn delete_tag(&mut self, id: &str) -> Result<(), StorageError> {
        let changed = self
            .connection
            .execute("DELETE FROM tags WHERE id = ?1", [id])?;
        ensure_changed(changed, "tag", id)
    }
}

impl AnnotationRepository for Storage {
    fn create_annotation(&mut self, annotation: &Annotation) -> Result<(), StorageError> {
        insert_annotation(&self.connection, annotation)
    }

    fn get_annotation(&self, id: &str) -> Result<Annotation, StorageError> {
        load_annotation(&self.connection, id)
    }

    fn update_annotation(&mut self, annotation: &Annotation) -> Result<(), StorageError> {
        let changed = self.connection.execute(
            "UPDATE annotations
             SET label = ?1, value = ?2, language_tag = ?3, direction = ?4
             WHERE id = ?5",
            params![
                annotation.label,
                annotation.value,
                annotation.language_tag,
                direction_to_database(annotation.direction),
                annotation.id,
            ],
        )?;
        ensure_changed(changed, "annotation", &annotation.id)
    }

    fn delete_annotation(&mut self, id: &str) -> Result<(), StorageError> {
        let changed = self
            .connection
            .execute("DELETE FROM annotations WHERE id = ?1", [id])?;
        ensure_changed(changed, "annotation", id)
    }
}

impl MediaRepository for Storage {
    fn create_media_reference(&mut self, media: &MediaReference) -> Result<(), StorageError> {
        insert_media(&self.connection, media)
    }

    fn get_media_reference(&self, id: &str) -> Result<MediaReference, StorageError> {
        load_media(&self.connection, id)
    }

    fn update_media_reference(&mut self, media: &MediaReference) -> Result<(), StorageError> {
        let changed = self.connection.execute(
            "UPDATE media_references
             SET role = ?1,
                 original_file_name = ?2,
                 alt_text = ?3,
                 language_tag = ?4,
                 direction = ?5
             WHERE id = ?6
               AND content_hash = ?7
               AND kind = ?8
               AND media_type = ?9
               AND byte_size = ?10
               AND width IS ?11
               AND height IS ?12
               AND duration_ms IS ?13",
            params![
                media_role_to_database(media.role),
                media.original_file_name,
                media.alt_text,
                media.language_tag,
                direction_to_database(media.direction),
                media.id,
                media.content_hash,
                media_kind_to_database(media.kind),
                media.media_type,
                media.byte_size,
                media.width,
                media.height,
                media.duration_ms,
            ],
        )?;
        ensure_changed(changed, "media reference", &media.id)
    }

    fn delete_media_reference(&mut self, id: &str) -> Result<(), StorageError> {
        let references = self.media_reference_usage(id)?;
        if references > 0 {
            return Err(StorageError::MediaInUse {
                id: id.to_owned(),
                references,
            });
        }
        let changed = self
            .connection
            .execute("DELETE FROM media_references WHERE id = ?1", [id])?;
        ensure_changed(changed, "media reference", id)
    }

    fn media_reference_usage(&self, id: &str) -> Result<u64, StorageError> {
        let count = self.connection.query_row(
            "SELECT
                (SELECT COUNT(*) FROM source_item_media WHERE media_reference_id = ?1)
                + (SELECT COUNT(*) FROM cloze_media WHERE media_reference_id = ?1)",
            [id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    fn media_reference_count_for_hash(&self, content_hash: &str) -> Result<u64, StorageError> {
        let count = self.connection.query_row(
            "SELECT COUNT(*) FROM media_references WHERE content_hash = ?1",
            [content_hash],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

impl SchedulerParameterSetRepository for Storage {
    fn create_scheduler_parameter_set(
        &mut self,
        parameter_set: &SchedulerParameterSet,
    ) -> Result<(), StorageError> {
        let parameters = serde_json::to_string(&parameter_set.parameters)?;
        self.connection.execute(
            "INSERT INTO scheduler_parameter_sets(
                id, engine_version, parameters_json, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                parameter_set.id,
                parameter_set.engine_version,
                parameters,
                parameter_set.created_at_ms,
            ],
        )?;
        Ok(())
    }

    fn get_scheduler_parameter_set(&self, id: &str) -> Result<SchedulerParameterSet, StorageError> {
        let stored = self
            .connection
            .query_row(
                "SELECT id, engine_version, parameters_json, created_at_ms
                 FROM scheduler_parameter_sets
                 WHERE id = ?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| entity_not_found("scheduler parameter set", id))?;
        Ok(SchedulerParameterSet {
            id: stored.0,
            engine_version: stored.1,
            parameters: serde_json::from_str(&stored.2)?,
            created_at_ms: stored.3,
        })
    }

    fn update_scheduler_parameter_set(
        &mut self,
        parameter_set: &SchedulerParameterSet,
    ) -> Result<(), StorageError> {
        let parameters = serde_json::to_string(&parameter_set.parameters)?;
        let changed = self.connection.execute(
            "UPDATE scheduler_parameter_sets
             SET engine_version = ?1, parameters_json = ?2
             WHERE id = ?3",
            params![parameter_set.engine_version, parameters, parameter_set.id,],
        )?;
        ensure_changed(changed, "scheduler parameter set", &parameter_set.id)
    }

    fn delete_scheduler_parameter_set(&mut self, id: &str) -> Result<(), StorageError> {
        let changed = self
            .connection
            .execute("DELETE FROM scheduler_parameter_sets WHERE id = ?1", [id])?;
        ensure_changed(changed, "scheduler parameter set", id)
    }
}

impl SchedulerProfileRepository for Storage {
    fn get_scheduler_profile(&self, deck_id: &str) -> Result<SchedulerProfile, StorageError> {
        load_scheduler_profile(&self.connection, deck_id)
    }

    fn update_scheduler_profile(&mut self, profile: &SchedulerProfile) -> Result<(), StorageError> {
        validate_scheduler_profile(&self.connection, profile)?;
        let changed = self.connection.execute(
            "UPDATE scheduler_profiles
             SET engine_version = ?1,
                 active_parameter_set_id = ?2,
                 scheduling_mode = ?3,
                 daily_time_budget_minutes = ?4,
                 controller_version = ?5,
                 controller_target_retention_basis_points = ?6,
                 controller_new_cards_per_day = ?7,
                 controller_last_evaluated_day_start_ms = ?8,
                 controller_review_count = ?9,
                 controller_unseen_count = ?10,
                 controller_forecast_review_seconds_per_day = ?11,
                 controller_backlog_exceeds_budget = ?12,
                 controller_explanation = ?13,
                 day_boundary_minutes = ?14,
                 updated_at_ms = ?15
             WHERE deck_id = ?16",
            params![
                profile.engine_version,
                profile.active_parameter_set_id,
                scheduling_mode_to_database(profile.scheduling_mode),
                profile.deck_daily_time_budget_minutes,
                profile.controller_version,
                profile.controller_target_retention_basis_points,
                profile.controller_new_cards_per_day,
                profile.controller_last_evaluated_day_start_ms,
                profile.controller_review_count,
                profile.controller_unseen_count,
                profile.controller_forecast_review_seconds_per_day,
                profile.controller_backlog_exceeds_budget,
                profile.controller_explanation,
                profile.day_boundary_minutes,
                profile.updated_at_ms,
                profile.deck_id,
            ],
        )?;
        ensure_changed(changed, "scheduler profile", &profile.deck_id)
    }
}

impl Storage {
    /// Counts active and trashed source notes owned by a deck.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the query fails.
    pub fn deck_note_count(&self, deck_id: &str) -> Result<u64, StorageError> {
        Ok(self.connection.query_row(
            "SELECT COUNT(*) FROM source_items WHERE deck_id = ?1",
            [deck_id],
            |row| row.get(0),
        )?)
    }

    /// Aggregates card counts for every flat deck in one bounded query.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the aggregate query fails.
    pub fn deck_card_counts(&self, now_ms: i64) -> Result<Vec<DeckCardCounts>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT
                decks.id,
                COUNT(cards.id),
                COALESCE(SUM(CASE
                    WHEN source_items.deleted_at_ms IS NULL
                     AND cards.id IS NOT NULL
                    THEN 1 ELSE 0
                END), 0),
                COALESCE(SUM(CASE
                    WHEN source_items.deleted_at_ms IS NULL
                     AND cards.suspended = 0
                     AND schedule_states.lifecycle = 'introduced'
                     AND schedule_states.due_at_ms <= ?1
                    THEN 1 ELSE 0
                END), 0),
                COALESCE(SUM(CASE
                    WHEN source_items.deleted_at_ms IS NULL
                     AND cards.suspended = 0
                     AND schedule_states.lifecycle = 'unseen'
                    THEN 1 ELSE 0
                END), 0)
             FROM decks
             LEFT JOIN source_items ON source_items.deck_id = decks.id
             LEFT JOIN clozes ON clozes.source_item_id = source_items.id
             LEFT JOIN cards ON cards.cloze_id = clozes.id
             LEFT JOIN schedule_states ON schedule_states.card_id = cards.id
             GROUP BY decks.id
             ORDER BY decks.id",
        )?;
        statement
            .query_map([now_ms], |row| {
                Ok(DeckCardCounts {
                    deck_id: row.get(0)?,
                    all_cards: row.get(1)?,
                    total_cards: row.get(2)?,
                    due_cards: row.get(3)?,
                    new_cards: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Deletes a deck atomically, rehoming its notes and moving active cards to
    /// Trash unless a destination deck is selected.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when either deck is missing, the destination is
    /// invalid, or the transaction cannot be committed.
    pub fn delete_deck_and_rehome_notes(
        &mut self,
        deck_id: &str,
        destination_deck_id: Option<&str>,
        updated_at_ms: i64,
    ) -> Result<u64, StorageError> {
        let transaction = self.connection.transaction()?;
        ensure_entity_exists(&transaction, "decks", "deck", deck_id)?;
        let active_card_count = transaction.query_row(
            "SELECT COUNT(cards.id)
             FROM cards
             JOIN clozes ON clozes.id = cards.cloze_id
             JOIN source_items ON source_items.id = clozes.source_item_id
             WHERE source_items.deck_id = ?1
               AND source_items.deleted_at_ms IS NULL",
            [deck_id],
            |row| row.get::<_, u64>(0),
        )?;
        let destination = destination_deck_id.unwrap_or(DEFAULT_DECK_ID);
        if destination_deck_id.is_some() && destination == deck_id {
            return Err(StorageError::InvalidAggregate(
                "a deck cannot move cards into itself before deletion".into(),
            ));
        }
        ensure_entity_exists(&transaction, "decks", "destination deck", destination)?;
        if destination_deck_id.is_some() {
            transaction.execute(
                "UPDATE source_items
                 SET deck_id = ?1, updated_at_ms = ?2
                 WHERE deck_id = ?3",
                params![destination, updated_at_ms, deck_id],
            )?;
        } else {
            transaction.execute(
                "UPDATE source_items
                 SET deck_id = ?1,
                     deleted_at_ms = COALESCE(deleted_at_ms, ?2),
                     updated_at_ms = ?2
                 WHERE deck_id = ?3",
                params![destination, updated_at_ms, deck_id],
            )?;
        }
        let changed = transaction.execute("DELETE FROM decks WHERE id = ?1", [deck_id])?;
        ensure_changed(changed, "deck", deck_id)?;
        transaction.commit()?;
        Ok(active_card_count)
    }

    /// Loads the collection-wide default daily study budget.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the singleton settings row is missing or
    /// cannot be decoded.
    pub fn collection_scheduling_settings(
        &self,
    ) -> Result<CollectionSchedulingSettings, StorageError> {
        Ok(self.connection.query_row(
            "SELECT daily_time_budget_minutes, updated_at_ms
             FROM collection_scheduler_settings
             WHERE singleton = 1",
            [],
            |row| {
                Ok(CollectionSchedulingSettings {
                    daily_time_budget_minutes: row.get(0)?,
                    updated_at_ms: row.get(1)?,
                })
            },
        )?)
    }

    /// Updates the collection-wide default daily study budget.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the budget is invalid or cannot be
    /// persisted.
    pub fn update_collection_scheduling_settings(
        &mut self,
        settings: &CollectionSchedulingSettings,
    ) -> Result<(), StorageError> {
        if !(1..=1_440).contains(&settings.daily_time_budget_minutes) {
            return Err(StorageError::InvalidAggregate(
                "collection daily time budget must be between 1 and 1440 minutes".into(),
            ));
        }
        let changed = self.connection.execute(
            "UPDATE collection_scheduler_settings
             SET daily_time_budget_minutes = ?1, updated_at_ms = ?2
             WHERE singleton = 1",
            params![settings.daily_time_budget_minutes, settings.updated_at_ms],
        )?;
        ensure_changed(changed, "collection scheduler settings", "1")
    }

    /// Aggregates bounded controller inputs without loading card content.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the deck is missing, bounds are invalid,
    /// or aggregate state cannot be queried.
    pub fn scheduling_workload(
        &self,
        deck_id: &str,
        now_ms: i64,
        horizon_end_ms: i64,
    ) -> Result<SchedulingWorkload, StorageError> {
        const DAY_MS: i64 = 86_400_000;
        if horizon_end_ms <= now_ms {
            return Err(StorageError::InvalidAggregate(
                "the scheduling forecast horizon must follow the current time".into(),
            ));
        }
        self.get_deck(deck_id)?;
        let (unseen_cards, due_cards_now, forecast_review_occurrences) =
            self.connection.query_row(
                "SELECT
                    COALESCE(SUM(CASE
                        WHEN schedule_states.lifecycle = 'unseen' THEN 1 ELSE 0
                    END), 0),
                    COALESCE(SUM(CASE
                        WHEN schedule_states.lifecycle = 'introduced'
                         AND schedule_states.due_at_ms <= ?2
                        THEN 1 ELSE 0
                    END), 0),
                    COALESCE(SUM(CASE
                        WHEN schedule_states.lifecycle = 'introduced'
                         AND schedule_states.due_at_ms < ?3
                        THEN 1 + MIN(
                            27,
                            MAX(
                                0,
                                (?3 - MAX(schedule_states.due_at_ms, ?2))
                                / MAX(schedule_states.interval_milliseconds, ?4)
                            )
                        )
                        ELSE 0
                    END), 0)
                 FROM schedule_states
                 JOIN cards ON cards.id = schedule_states.card_id
                 JOIN clozes ON clozes.id = cards.cloze_id
                 JOIN source_items ON source_items.id = clozes.source_item_id
                 WHERE source_items.deck_id = ?1
                   AND source_items.deleted_at_ms IS NULL
                   AND cards.suspended = 0",
                params![deck_id, now_ms, horizon_end_ms, DAY_MS],
                |row| {
                    Ok((
                        row.get::<_, u64>(0)?,
                        row.get::<_, u64>(1)?,
                        row.get::<_, u64>(2)?,
                    ))
                },
            )?;
        let mut response_durations = self
            .connection
            .prepare(
                "SELECT review_events.response_duration_ms
                 FROM review_events
                 JOIN cards ON cards.id = review_events.card_id
                 JOIN clozes ON clozes.id = cards.cloze_id
                 JOIN source_items ON source_items.id = clozes.source_item_id
                 WHERE source_items.deck_id = ?1
                   AND review_events.event_kind = 'review'
                   AND review_events.response_duration_ms BETWEEN 1000 AND 600000
                 ORDER BY review_events.reviewed_at_ms DESC, review_events.id DESC
                 LIMIT ?2",
            )?
            .query_map(params![deck_id, MAXIMUM_RESPONSE_DURATION_SAMPLES], |row| {
                row.get::<_, u64>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        response_durations.sort_unstable();
        let response_duration_samples = u64::try_from(response_durations.len())
            .map_err(|_| StorageError::NumericRange("controller response duration sample count"))?;
        let median_response_duration_ms = response_durations
            .get(response_durations.len() / 2)
            .copied();
        let review_count = self.connection.query_row(
            "SELECT COUNT(*)
             FROM review_events
             JOIN cards ON cards.id = review_events.card_id
             JOIN clozes ON clozes.id = cards.cloze_id
             JOIN source_items ON source_items.id = clozes.source_item_id
             WHERE source_items.deck_id = ?1
               AND review_events.event_kind = 'review'",
            [deck_id],
            |row| row.get::<_, u64>(0),
        )?;
        Ok(SchedulingWorkload {
            unseen_cards,
            due_cards_now,
            forecast_review_occurrences,
            response_duration_samples,
            median_response_duration_ms,
            review_count,
        })
    }

    /// Atomically stores and activates a new immutable parameter set.
    ///
    /// Existing schedule projections are deliberately left unchanged; the new
    /// parameters apply only to future review decisions.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the profile and parameter engine versions
    /// differ or the transaction cannot be committed.
    pub fn adopt_scheduler_parameter_set(
        &mut self,
        deck_id: &str,
        parameter_set: &SchedulerParameterSet,
        updated_at_ms: i64,
    ) -> Result<SchedulerProfile, StorageError> {
        let transaction = self.connection.transaction()?;
        let current = load_scheduler_profile(&transaction, deck_id)?;
        if current.engine_version != parameter_set.engine_version {
            return Err(StorageError::InvalidAggregate(
                "a scheduler profile and parameter set must use the same engine version".into(),
            ));
        }
        let parameters = serde_json::to_string(&parameter_set.parameters)?;
        transaction.execute(
            "INSERT INTO scheduler_parameter_sets(
                id, engine_version, parameters_json, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                parameter_set.id,
                parameter_set.engine_version,
                parameters,
                parameter_set.created_at_ms,
            ],
        )?;
        transaction.execute(
            "UPDATE scheduler_profiles
             SET active_parameter_set_id = ?1,
                 updated_at_ms = ?2
             WHERE deck_id = ?3",
            params![parameter_set.id, updated_at_ms, deck_id],
        )?;
        transaction.commit()?;
        load_scheduler_profile(&self.connection, deck_id)
    }
}

impl SourceNoteRepository for Storage {
    fn create_source_note(&mut self, note: &StoredSourceNote) -> Result<(), StorageError> {
        validate_note(note)?;
        let transaction = self.connection.transaction()?;
        insert_source(&transaction, &note.source_item)?;
        insert_note_children(&transaction, note)?;
        transaction.commit()?;
        Ok(())
    }

    fn get_source_note(&self, id: &str) -> Result<StoredSourceNote, StorageError> {
        load_source_note(&self.connection, id)
    }

    fn update_source_note(&mut self, note: &StoredSourceNote) -> Result<(), StorageError> {
        validate_note(note)?;
        let transaction = self.connection.transaction()?;
        let changed = update_source(&transaction, &note.source_item)?;
        ensure_changed(changed, "source note", &note.source_item.id)?;

        transaction.execute(
            "DELETE FROM semantic_segments WHERE source_item_id = ?1",
            [&note.source_item.id],
        )?;
        replace_owned_annotations(
            &transaction,
            "source_item_annotations",
            "source_item_id",
            &note.source_item.id,
            &note.source_item.annotations,
        )?;
        replace_source_tags(&transaction, &note.source_item)?;
        replace_source_media(&transaction, &note.source_item)?;

        let existing = source_cloze_ids(&transaction, &note.source_item.id)?;
        let requested = note
            .clozes
            .iter()
            .map(|cloze| cloze.id.as_str())
            .collect::<HashSet<_>>();
        for cloze_id in existing {
            if !requested.contains(cloze_id.as_str()) {
                delete_owned_annotations(&transaction, "cloze_annotations", "cloze_id", &cloze_id)?;
                transaction.execute("DELETE FROM clozes WHERE id = ?1", [&cloze_id])?;
            }
        }

        for cloze in &note.clozes {
            let exists = transaction
                .query_row(
                    "SELECT 1 FROM clozes WHERE id = ?1",
                    [&cloze.id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if exists {
                update_cloze_row(&transaction, cloze)?;
                replace_owned_annotations(
                    &transaction,
                    "cloze_annotations",
                    "cloze_id",
                    &cloze.id,
                    &cloze.annotations,
                )?;
                replace_cloze_media(&transaction, cloze)?;
            } else {
                insert_cloze(&transaction, cloze)?;
                insert_cloze_children(&transaction, cloze)?;
            }
        }
        insert_segments(&transaction, &note.source_item)?;
        transaction.commit()?;
        Ok(())
    }

    fn delete_source_note(&mut self, id: &str) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        let cloze_ids = source_cloze_ids(&transaction, id)?;
        delete_owned_annotations(
            &transaction,
            "source_item_annotations",
            "source_item_id",
            id,
        )?;
        for cloze_id in &cloze_ids {
            delete_owned_annotations(&transaction, "cloze_annotations", "cloze_id", cloze_id)?;
        }
        transaction.execute(
            "DELETE FROM cards
             WHERE cloze_id IN (
                SELECT id FROM clozes WHERE source_item_id = ?1
             )",
            [id],
        )?;
        transaction.execute(
            "DELETE FROM semantic_segments WHERE source_item_id = ?1",
            [id],
        )?;
        let changed = transaction.execute("DELETE FROM source_items WHERE id = ?1", [id])?;
        ensure_changed(changed, "source note", id)?;
        transaction.commit()?;
        Ok(())
    }
}

impl ClozeRepository for Storage {
    fn get_cloze(&self, id: &str) -> Result<Cloze, StorageError> {
        load_cloze(&self.connection, id)
    }

    fn update_cloze(&mut self, cloze: &Cloze) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        let changed = update_cloze_row(&transaction, cloze)?;
        ensure_changed(changed, "cloze", &cloze.id)?;
        let changed = transaction.execute(
            "UPDATE semantic_segments
             SET text = ?1
             WHERE cloze_id = ?2",
            params![cloze.answer, cloze.id],
        )?;
        if changed != 1 {
            return Err(StorageError::InvalidAggregate(format!(
                "cloze {} must own exactly one semantic segment",
                cloze.id
            )));
        }
        replace_owned_annotations(
            &transaction,
            "cloze_annotations",
            "cloze_id",
            &cloze.id,
            &cloze.annotations,
        )?;
        replace_cloze_media(&transaction, cloze)?;
        transaction.commit()?;
        Ok(())
    }

    fn delete_cloze(&mut self, id: &str) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM semantic_segments WHERE cloze_id = ?1", [id])?;
        delete_owned_annotations(&transaction, "cloze_annotations", "cloze_id", id)?;
        let changed = transaction.execute("DELETE FROM clozes WHERE id = ?1", [id])?;
        ensure_changed(changed, "cloze", id)?;
        transaction.commit()?;
        Ok(())
    }
}

impl CardRepository for Storage {
    fn create_card(
        &mut self,
        card: &Card,
        initial_schedule: &ScheduleState,
    ) -> Result<(), StorageError> {
        if initial_schedule.card_id != card.id
            || initial_schedule.version != 0
            || initial_schedule.last_review_event_id.is_some()
        {
            return Err(StorageError::InvalidAggregate(
                "a new card requires its own version-zero schedule".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        insert_card_with_schedule(&transaction, card, initial_schedule)?;
        transaction.commit()?;
        Ok(())
    }

    fn get_card(&self, id: &str) -> Result<Card, StorageError> {
        load_card(&self.connection, id)
    }

    fn get_card_for_cloze(&self, cloze_id: &str) -> Result<Card, StorageError> {
        let card_id = self
            .connection
            .query_row(
                "SELECT id FROM cards WHERE cloze_id = ?1",
                [cloze_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| entity_not_found("card for cloze", cloze_id))?;
        load_card(&self.connection, &card_id)
    }

    fn update_card(&mut self, card: &Card) -> Result<(), StorageError> {
        let stored = load_card(&self.connection, &card.id)?;
        if stored.cloze_id != card.cloze_id {
            return Err(StorageError::InvalidAggregate(format!(
                "card {} cannot move between clozes",
                card.id
            )));
        }
        let changed = self.connection.execute(
            "UPDATE cards
             SET content_version = ?1,
                 suspended = ?2,
                 updated_at_ms = ?3
             WHERE id = ?4",
            params![
                card.content_version,
                card.suspended,
                card.updated_at_ms,
                card.id,
            ],
        )?;
        ensure_changed(changed, "card", &card.id)
    }

    fn delete_card(&mut self, id: &str) -> Result<(), StorageError> {
        let changed = self
            .connection
            .execute("DELETE FROM cards WHERE id = ?1", [id])?;
        ensure_changed(changed, "card", id)
    }
}

impl PristineDeckRepository for Storage {
    fn validate_pristine_deck_import(
        &self,
        import: &PristineDeckImport,
    ) -> Result<PristineDeckImportStatus, StorageError> {
        validate_pristine_deck_import(&self.connection, import)
    }

    fn import_pristine_deck(
        &mut self,
        import: &PristineDeckImport,
    ) -> Result<PristineDeckImportStatus, StorageError> {
        let transaction = self.connection.transaction()?;
        let status = validate_pristine_deck_import(&transaction, import)?;
        if status == PristineDeckImportStatus::AlreadyInstalled {
            return Ok(status);
        }
        persist_pristine_deck_import(&transaction, import, &mut || {})?;
        transaction.commit()?;
        Ok(PristineDeckImportStatus::Ready)
    }
}

impl Storage {
    /// Lists the remaining deck identities for one installed bundle in their
    /// original stage order.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when bundle associations cannot be read.
    pub fn bundle_deck_ids(&self, language_tag: &str) -> Result<Vec<String>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT deck_id
             FROM bundle_decks
             WHERE language_tag = ?1
             ORDER BY ordinal",
        )?;
        statement
            .query_map([language_tag], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Lists installed language bundles that still have associated decks.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when bundle associations or card counts cannot
    /// be read.
    pub fn installed_bundles(&self) -> Result<Vec<InstalledBundle>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT bundle_installations.language_tag,
                    COUNT(DISTINCT bundle_decks.deck_id),
                    COUNT(cards.id)
             FROM bundle_installations
             JOIN bundle_decks
               ON bundle_decks.language_tag = bundle_installations.language_tag
             LEFT JOIN source_items
               ON source_items.deck_id = bundle_decks.deck_id
              AND source_items.deleted_at_ms IS NULL
             LEFT JOIN clozes ON clozes.source_item_id = source_items.id
             LEFT JOIN cards ON cards.cloze_id = clozes.id
             GROUP BY bundle_installations.language_tag
             ORDER BY bundle_installations.language_tag",
        )?;
        statement
            .query_map([], |row| {
                Ok(InstalledBundle {
                    language_tag: row.get(0)?,
                    deck_count: row.get(1)?,
                    active_card_count: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Removes every remaining deck associated with one language bundle in a
    /// single transaction, moving its active cards to Trash.
    ///
    /// The callback receives cumulative deck and card counts after each deck
    /// is staged. Expected counts prevent confirming a stale preview.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the bundle is missing, its contents
    /// changed after preview, or any durable write fails.
    pub fn remove_bundle(
        &mut self,
        language_tag: &str,
        expected_deck_count: u64,
        expected_active_card_count: u64,
        updated_at_ms: i64,
        mut on_progress: impl FnMut(u64, u64),
    ) -> Result<InstalledBundle, StorageError> {
        let transaction = self.connection.transaction()?;
        let associated_decks = {
            let mut statement = transaction.prepare(
                "SELECT bundle_decks.deck_id, COUNT(cards.id)
                 FROM bundle_decks
                 LEFT JOIN source_items
                   ON source_items.deck_id = bundle_decks.deck_id
                  AND source_items.deleted_at_ms IS NULL
                 LEFT JOIN clozes ON clozes.source_item_id = source_items.id
                 LEFT JOIN cards ON cards.cloze_id = clozes.id
                 WHERE bundle_decks.language_tag = ?1
                 GROUP BY bundle_decks.deck_id, bundle_decks.ordinal
                 ORDER BY bundle_decks.ordinal",
            )?;
            statement
                .query_map([language_tag], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        if associated_decks.is_empty() {
            return Err(StorageError::EntityNotFound {
                entity: "installed bundle",
                id: language_tag.into(),
            });
        }
        let deck_count = u64::try_from(associated_decks.len())
            .map_err(|_| StorageError::NumericRange("bundle deck count"))?;
        let active_card_count = associated_decks
            .iter()
            .try_fold(0_u64, |total, (_, cards)| total.checked_add(*cards))
            .ok_or(StorageError::NumericRange("bundle active card count"))?;
        if deck_count != expected_deck_count || active_card_count != expected_active_card_count {
            return Err(StorageError::InvalidAggregate(
                "the installed bundle changed after confirmation details were loaded".into(),
            ));
        }

        let mut removed_decks = 0_u64;
        let mut moved_cards = 0_u64;
        for (deck_id, deck_card_count) in associated_decks {
            transaction.execute(
                "UPDATE source_items
                 SET deck_id = ?1,
                     deleted_at_ms = COALESCE(deleted_at_ms, ?2),
                     updated_at_ms = ?2
                 WHERE deck_id = ?3",
                params![DEFAULT_DECK_ID, updated_at_ms, deck_id],
            )?;
            let changed = transaction.execute("DELETE FROM decks WHERE id = ?1", [&deck_id])?;
            ensure_changed(changed, "deck", &deck_id)?;
            removed_decks += 1;
            moved_cards = moved_cards
                .checked_add(deck_card_count)
                .ok_or(StorageError::NumericRange("removed bundle card count"))?;
            on_progress(removed_decks, moved_cards);
        }
        let changed = transaction.execute(
            "DELETE FROM bundle_installations WHERE language_tag = ?1",
            [language_tag],
        )?;
        ensure_changed(changed, "installed bundle", language_tag)?;
        transaction.commit()?;
        Ok(InstalledBundle {
            language_tag: language_tag.into(),
            deck_count,
            active_card_count,
        })
    }

    /// Validates a pristine language bundle without changing the collection.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the bundle is empty, language membership
    /// is inconsistent, or any missing deck identity collides.
    pub fn validate_pristine_bundle_import(
        &self,
        import: &PristineBundleImport,
    ) -> Result<PristineBundleImportPlan, StorageError> {
        validate_pristine_bundle_import(&self.connection, import)
    }

    /// Adds every missing pristine bundle deck in one transaction.
    ///
    /// The card callback runs after each missing card is staged. The final
    /// callback runs after all database writes are staged but before commit.
    ///
    /// # Errors
    ///
    /// Returns [`PristineBundleImportError::Storage`] when validation or a
    /// database write fails, or [`PristineBundleImportError::BeforeCommit`]
    /// when the final callback fails. No bundle change is committed in either
    /// case.
    pub fn import_pristine_bundle<T, E>(
        &mut self,
        import: &PristineBundleImport,
        mut on_card_imported: impl FnMut(),
        before_commit: impl FnOnce() -> Result<T, E>,
    ) -> Result<(PristineBundleImportPlan, T), PristineBundleImportError<E>> {
        let transaction = self
            .connection
            .transaction()
            .map_err(StorageError::from)
            .map_err(PristineBundleImportError::Storage)?;
        let plan = validate_pristine_bundle_import(&transaction, import)
            .map_err(PristineBundleImportError::Storage)?;
        persist_pristine_bundle_import(&transaction, import, &plan, &mut on_card_imported)
            .map_err(PristineBundleImportError::Storage)?;
        let prepared = before_commit().map_err(PristineBundleImportError::BeforeCommit)?;
        transaction
            .commit()
            .map_err(StorageError::from)
            .map_err(PristineBundleImportError::Storage)?;
        Ok((plan, prepared))
    }

    /// Exercises rollback after all pristine-deck writes but before commit.
    ///
    /// This bounded fault is available only to local tests and fixture builds.
    ///
    /// # Errors
    ///
    /// Always returns [`StorageError::InjectedTestFailure`] after issuing the
    /// same database writes as
    /// [`PristineDeckRepository::import_pristine_deck`] inside an uncommitted
    /// transaction.
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn import_pristine_deck_failing_before_commit(
        &mut self,
        import: &PristineDeckImport,
    ) -> Result<PristineDeckImportStatus, StorageError> {
        let transaction = self.connection.transaction()?;
        let status = validate_pristine_deck_import(&transaction, import)?;
        if status == PristineDeckImportStatus::AlreadyInstalled {
            return Ok(status);
        }
        persist_pristine_deck_import(&transaction, import, &mut || {})?;
        Err(StorageError::InjectedTestFailure(
            "pristine deck transaction before commit",
        ))
    }

    /// Exercises rollback after all pristine-bundle writes but before commit.
    ///
    /// # Errors
    ///
    /// Always returns [`StorageError::InjectedTestFailure`] after staging the
    /// same database writes as [`Self::import_pristine_bundle`].
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn import_pristine_bundle_failing_before_commit(
        &mut self,
        import: &PristineBundleImport,
    ) -> Result<PristineBundleImportPlan, StorageError> {
        let transaction = self.connection.transaction()?;
        let plan = validate_pristine_bundle_import(&transaction, import)?;
        persist_pristine_bundle_import(&transaction, import, &plan, &mut || {})?;
        Err(StorageError::InjectedTestFailure(
            "pristine bundle transaction before commit",
        ))
    }
}

fn validate_pristine_deck_import(
    connection: &Connection,
    import: &PristineDeckImport,
) -> Result<PristineDeckImportStatus, StorageError> {
    if entity_id_exists(
        connection,
        "SELECT 1 FROM decks WHERE id = ?1",
        &import.deck.id,
    )? {
        return Ok(PristineDeckImportStatus::AlreadyInstalled);
    }

    let mut identities = PristineImportIdentities::default();
    for imported_note in &import.notes {
        validate_pristine_note(imported_note, &import.deck.id, &mut identities)?;
    }
    ensure_pristine_identities_available(connection, &identities)?;

    Ok(PristineDeckImportStatus::Ready)
}

fn validate_pristine_bundle_import(
    connection: &Connection,
    import: &PristineBundleImport,
) -> Result<PristineBundleImportPlan, StorageError> {
    if import.language_tag.trim().is_empty() || import.decks.is_empty() {
        return Err(StorageError::InvalidAggregate(
            "a pristine bundle requires a language and at least one deck".into(),
        ));
    }

    let mut deck_ids = HashSet::new();
    let mut identities = PristineImportIdentities::default();
    let mut installed_deck_ids = Vec::new();
    let mut missing_deck_ids = Vec::new();
    let mut unassociated_deck_ids = Vec::new();
    let mut imported_ordinals = HashMap::new();
    let mut previous_associated_ordinal = None;
    for (ordinal, imported_deck) in import.decks.iter().enumerate() {
        let deck = &imported_deck.deck;
        let ordinal = u64::try_from(ordinal)
            .map_err(|_| StorageError::NumericRange("bundle deck ordinal"))?;
        if !deck_ids.insert(deck.id.as_str()) {
            return Err(StorageError::InvalidAggregate(format!(
                "bundle deck identity {} is duplicated",
                deck.id
            )));
        }
        imported_ordinals.insert(deck.id.as_str(), ordinal);
        if deck.language_tag.as_deref() != Some(import.language_tag.as_str()) {
            return Err(StorageError::InvalidAggregate(format!(
                "bundle deck {} does not use language {}",
                deck.id, import.language_tag
            )));
        }
        let deck_association = connection
            .query_row(
                "SELECT language_tag, ordinal FROM bundle_decks WHERE deck_id = ?1",
                [&deck.id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?)),
            )
            .optional()?;
        if deck_association
            .as_ref()
            .is_some_and(|(associated_language, _)| associated_language != &import.language_tag)
        {
            return Err(StorageError::InvalidAggregate(format!(
                "bundle deck {} is already associated with another bundle or stage",
                deck.id
            )));
        }
        if let Some((_, associated_ordinal)) = deck_association.as_ref() {
            if previous_associated_ordinal.is_some_and(|previous| previous >= *associated_ordinal) {
                return Err(StorageError::InvalidAggregate(format!(
                    "bundle deck {} is already associated with another bundle or stage",
                    deck.id
                )));
            }
            previous_associated_ordinal = Some(*associated_ordinal);
        }
        if entity_id_exists(connection, "SELECT 1 FROM decks WHERE id = ?1", &deck.id)? {
            installed_deck_ids.push(deck.id.clone());
            if deck_association.is_none() {
                unassociated_deck_ids.push(deck.id.clone());
            }
            continue;
        }
        for imported_note in &imported_deck.notes {
            validate_pristine_note(imported_note, &deck.id, &mut identities)?;
        }
        missing_deck_ids.push(deck.id.clone());
    }
    if !missing_deck_ids.is_empty() || !unassociated_deck_ids.is_empty() {
        let mut desired_stage_decks = HashMap::new();
        let mut statement = connection.prepare(
            "SELECT deck_id, ordinal
             FROM bundle_decks
             WHERE language_tag = ?1",
        )?;
        let associated_decks = statement
            .query_map([&import.language_tag], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (deck_id, ordinal) in associated_decks {
            if !imported_ordinals.contains_key(deck_id.as_str()) {
                desired_stage_decks.insert(ordinal, deck_id);
            }
        }
        for (deck_id, ordinal) in &imported_ordinals {
            if desired_stage_decks
                .insert(*ordinal, (*deck_id).to_owned())
                .is_some()
            {
                return Err(StorageError::InvalidAggregate(format!(
                    "bundle stage {ordinal} is already associated with another deck"
                )));
            }
        }
    }
    ensure_pristine_identities_available(connection, &identities)?;

    Ok(PristineBundleImportPlan {
        installed_deck_ids,
        missing_deck_ids,
        unassociated_deck_ids,
    })
}

#[derive(Default)]
struct PristineImportIdentities<'a> {
    aggregate: HashSet<&'a str>,
    annotations: HashSet<&'a str>,
    media: HashMap<&'a str, &'a MediaReference>,
    tags: HashMap<&'a str, &'a Tag>,
    tag_names: HashMap<&'a str, &'a str>,
}

fn validate_pristine_note<'a>(
    imported: &'a crate::PristineDeckNote,
    deck_id: &str,
    identities: &mut PristineImportIdentities<'a>,
) -> Result<(), StorageError> {
    validate_note(&imported.note)?;
    let source = &imported.note.source_item;
    if source.deck_id != deck_id {
        return Err(StorageError::InvalidAggregate(format!(
            "source note {} belongs to a different imported deck",
            source.id
        )));
    }
    insert_import_identity(&mut identities.aggregate, &source.id, "source note")?;
    for segment in &source.segments {
        insert_import_identity(&mut identities.aggregate, &segment.id, "semantic segment")?;
    }
    for cloze in &imported.note.clozes {
        insert_import_identity(&mut identities.aggregate, &cloze.id, "cloze")?;
    }
    validate_pristine_cards(imported, &mut identities.aggregate)?;
    collect_pristine_child_identities(imported, identities)
}

fn validate_pristine_cards<'a>(
    imported: &'a crate::PristineDeckNote,
    aggregate_ids: &mut HashSet<&'a str>,
) -> Result<(), StorageError> {
    let source_id = &imported.note.source_item.id;
    let cloze_ids = imported
        .note
        .clozes
        .iter()
        .map(|cloze| cloze.id.as_str())
        .collect::<HashSet<_>>();
    if imported.cards.len() != cloze_ids.len() {
        return Err(pristine_card_count_error(source_id));
    }
    let mut card_cloze_ids = HashSet::new();
    for imported_card in &imported.cards {
        let card = &imported_card.card;
        let schedule = &imported_card.initial_schedule;
        insert_import_identity(aggregate_ids, &card.id, "card")?;
        if !card_cloze_ids.insert(card.cloze_id.as_str())
            || !cloze_ids.contains(card.cloze_id.as_str())
            || card.suspended
            || !is_pristine_schedule(schedule, &card.id)
        {
            return Err(StorageError::InvalidAggregate(format!(
                "card {} is not a pristine unseen card",
                card.id
            )));
        }
    }
    if card_cloze_ids != cloze_ids {
        return Err(pristine_card_count_error(source_id));
    }
    Ok(())
}

fn is_pristine_schedule(schedule: &ScheduleState, card_id: &str) -> bool {
    schedule.card_id == card_id
        && schedule.version == 0
        && schedule.lifecycle == CardLifecycle::Unseen
        && schedule.interval_milliseconds == 0
        && schedule.interval_seconds == 0
        && schedule.repetitions == 0
        && schedule.stability_milliseconds == 0
        && schedule.difficulty_millipoints == 0
        && schedule.last_reviewed_at_ms.is_none()
        && schedule.last_review_event_id.is_none()
}

fn pristine_card_count_error(source_id: &str) -> StorageError {
    StorageError::InvalidAggregate(format!(
        "source note {source_id} must contain one pristine card per cloze"
    ))
}

fn collect_pristine_child_identities<'a>(
    imported: &'a crate::PristineDeckNote,
    identities: &mut PristineImportIdentities<'a>,
) -> Result<(), StorageError> {
    let source = &imported.note.source_item;
    for annotation in source.annotations.iter().chain(
        imported
            .note
            .clozes
            .iter()
            .flat_map(|cloze| cloze.annotations.iter()),
    ) {
        if !identities.annotations.insert(annotation.id.as_str()) {
            return Err(StorageError::InvalidAggregate(format!(
                "annotation identity {} is duplicated in the imported deck",
                annotation.id
            )));
        }
    }
    for media in source.media.iter().chain(
        imported
            .note
            .clozes
            .iter()
            .flat_map(|cloze| cloze.media.iter()),
    ) {
        if let Some(existing) = identities.media.insert(media.id.as_str(), media) {
            if existing != media {
                return Err(StorageError::InvalidAggregate(format!(
                    "media reference identity {} has conflicting imported metadata",
                    media.id
                )));
            }
        }
    }
    collect_pristine_tags(&source.tags, identities)
}

fn collect_pristine_tags<'a>(
    tags: &'a [Tag],
    identities: &mut PristineImportIdentities<'a>,
) -> Result<(), StorageError> {
    for tag in tags {
        if let Some(existing) = identities.tags.insert(tag.id.as_str(), tag) {
            if existing != tag {
                return Err(StorageError::InvalidAggregate(format!(
                    "tag identity {} has conflicting imported metadata",
                    tag.id
                )));
            }
        }
        if let Some(existing_id) = identities
            .tag_names
            .insert(tag.name.as_str(), tag.id.as_str())
        {
            if existing_id != tag.id {
                return Err(StorageError::InvalidAggregate(format!(
                    "tag name {:?} is duplicated by different imported identities",
                    tag.name
                )));
            }
        }
    }
    Ok(())
}

fn ensure_pristine_identities_available(
    connection: &Connection,
    identities: &PristineImportIdentities<'_>,
) -> Result<(), StorageError> {
    for id in &identities.aggregate {
        if let Some(entity) = existing_aggregate_identity(connection, id)? {
            return Err(StorageError::InvalidAggregate(format!(
                "imported {entity} identity {id} already exists"
            )));
        }
    }
    ensure_ids_available(
        connection,
        &identities.annotations,
        "annotations",
        "annotation",
    )?;
    ensure_ids_available(
        connection,
        &identities.media.keys().copied().collect(),
        "media_references",
        "media reference",
    )?;
    ensure_pristine_tags_available(connection, identities.tags.values().copied())
}

fn ensure_ids_available(
    connection: &Connection,
    ids: &HashSet<&str>,
    table: &str,
    entity: &str,
) -> Result<(), StorageError> {
    let sql = format!("SELECT 1 FROM {table} WHERE id = ?1");
    for id in ids {
        if entity_id_exists(connection, &sql, id)? {
            return Err(StorageError::InvalidAggregate(format!(
                "imported {entity} identity {id} already exists"
            )));
        }
    }
    Ok(())
}

fn ensure_pristine_tags_available<'a>(
    connection: &Connection,
    tags: impl Iterator<Item = &'a Tag>,
) -> Result<(), StorageError> {
    for tag in tags {
        if connection
            .query_row(
                "SELECT 1 FROM tags WHERE id = ?1 OR name = ?2",
                params![tag.id, tag.name],
                |_| Ok(()),
            )
            .optional()?
            .is_some()
        {
            return Err(StorageError::InvalidAggregate(format!(
                "imported tag identity {} or name {:?} already exists",
                tag.id, tag.name
            )));
        }
    }
    Ok(())
}

fn persist_pristine_deck_import(
    transaction: &Transaction<'_>,
    import: &PristineDeckImport,
    on_card_imported: &mut impl FnMut(),
) -> Result<(), StorageError> {
    insert_deck(transaction, &import.deck)?;
    insert_default_scheduler_profile(transaction, &import.deck.id, import.deck.created_at_ms)?;
    for imported_note in &import.notes {
        insert_source(transaction, &imported_note.note.source_item)?;
        insert_note_children(transaction, &imported_note.note)?;
        for imported_card in &imported_note.cards {
            insert_card_with_schedule(
                transaction,
                &imported_card.card,
                &imported_card.initial_schedule,
            )?;
            on_card_imported();
        }
    }
    Ok(())
}

fn persist_pristine_bundle_import(
    transaction: &Transaction<'_>,
    import: &PristineBundleImport,
    plan: &PristineBundleImportPlan,
    on_card_imported: &mut impl FnMut(),
) -> Result<(), StorageError> {
    if !plan.requires_changes() {
        return Ok(());
    }
    let installed_at_ms = import
        .decks
        .iter()
        .find(|deck| plan.missing_deck_ids.contains(&deck.deck.id))
        .map_or(0, |deck| deck.deck.created_at_ms);
    transaction.execute(
        "INSERT INTO bundle_installations(language_tag, installed_at_ms)
         VALUES (?1, ?2)
         ON CONFLICT(language_tag) DO NOTHING",
        params![import.language_tag, installed_at_ms],
    )?;
    let missing = plan
        .missing_deck_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    // Reinsert as a set because shifting one ordinal at a time can collide with another stage.
    for imported_deck in &import.decks {
        let deck_id = imported_deck.deck.id.as_str();
        if missing.contains(deck_id) {
            persist_pristine_deck_import(transaction, imported_deck, on_card_imported)?;
        }
        transaction.execute(
            "DELETE FROM bundle_decks WHERE language_tag = ?1 AND deck_id = ?2",
            params![import.language_tag, deck_id],
        )?;
    }
    for (ordinal, imported_deck) in import.decks.iter().enumerate() {
        transaction.execute(
            "INSERT INTO bundle_decks(language_tag, deck_id, ordinal)
             VALUES (?1, ?2, ?3)",
            params![
                import.language_tag,
                imported_deck.deck.id,
                u64::try_from(ordinal)
                    .map_err(|_| StorageError::NumericRange("bundle deck ordinal"))?
            ],
        )?;
    }
    Ok(())
}

fn insert_import_identity<'a>(
    identities: &mut HashSet<&'a str>,
    id: &'a str,
    entity: &str,
) -> Result<(), StorageError> {
    if id.trim().is_empty() || !identities.insert(id) {
        return Err(StorageError::InvalidAggregate(format!(
            "{entity} identity {id:?} is empty or duplicated in the imported deck"
        )));
    }
    Ok(())
}

fn existing_aggregate_identity(
    connection: &Connection,
    id: &str,
) -> Result<Option<&'static str>, StorageError> {
    for (entity, sql) in [
        ("source note", "SELECT 1 FROM source_items WHERE id = ?1"),
        (
            "semantic segment",
            "SELECT 1 FROM semantic_segments WHERE id = ?1",
        ),
        ("cloze", "SELECT 1 FROM clozes WHERE id = ?1"),
        ("card", "SELECT 1 FROM cards WHERE id = ?1"),
    ] {
        if entity_id_exists(connection, sql, id)? {
            return Ok(Some(entity));
        }
    }
    Ok(None)
}

fn entity_id_exists(connection: &Connection, sql: &str, id: &str) -> Result<bool, StorageError> {
    Ok(connection
        .query_row(sql, [id], |_| Ok(()))
        .optional()?
        .is_some())
}

fn insert_card_with_schedule(
    connection: &Connection,
    card: &Card,
    initial_schedule: &ScheduleState,
) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO cards(
            id,
            cloze_id,
            content_version,
            suspended,
            created_at_ms,
            updated_at_ms,
            queue_updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            card.id,
            card.cloze_id,
            card.content_version,
            card.suspended,
            card.created_at_ms,
            card.updated_at_ms,
        ],
    )?;
    insert_schedule(connection, "schedule_states", initial_schedule)?;
    insert_schedule(connection, "schedule_baselines", initial_schedule)
}

impl Storage {
    /// Loads every source note, its cards, current schedules, and trash state.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when any stored aggregate cannot be decoded.
    pub fn library_notes(&self) -> Result<Vec<StoredLibraryNote>, StorageError> {
        let stored = {
            let mut statement = self.connection.prepare(
                "SELECT id, deleted_at_ms
                 FROM source_items
                 ORDER BY updated_at_ms DESC, id",
            )?;
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };

        stored
            .into_iter()
            .map(|(source_id, deleted_at_ms)| {
                let note = load_source_note(&self.connection, &source_id)?;
                let cards = note
                    .clozes
                    .iter()
                    .map(|cloze| {
                        let card = self.get_card_for_cloze(&cloze.id)?;
                        let schedule = self.load_schedule(&card.id)?;
                        Ok(StoredLibraryCard { card, schedule })
                    })
                    .collect::<Result<Vec<_>, StorageError>>()?;
                Ok(StoredLibraryNote {
                    note,
                    cards,
                    deleted_at_ms,
                })
            })
            .collect()
    }

    /// Loads the minimal searchable text for cards in one deck and trash state.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the card search projection cannot be read.
    pub fn deck_card_search(
        &self,
        deck_id: &str,
        trashed: bool,
    ) -> Result<Vec<StoredDeckCardSearch>, StorageError> {
        // NUL separators prevent a query from matching across adjacent fields.
        let mut statement = self.connection.prepare(
            "SELECT
                cards.id,
                COALESCE((
                    SELECT group_concat(
                        CASE
                            WHEN semantic_segments.cloze_id = clozes.id THEN '[…]'
                            ELSE semantic_segments.text
                        END,
                        '' ORDER BY semantic_segments.ordinal
                    )
                    FROM semantic_segments
                    WHERE semantic_segments.source_item_id = source_items.id
                ), '') || char(0) ||
                clozes.answer || char(0) ||
                clozes.accepted_answers_json || char(0) ||
                COALESCE(clozes.hint, '') || char(0) ||
                COALESCE((
                    SELECT group_concat(
                        tags.name,
                        char(0) ORDER BY source_item_tags.ordinal
                    )
                    FROM source_item_tags
                    JOIN tags ON tags.id = source_item_tags.tag_id
                    WHERE source_item_tags.source_item_id = source_items.id
                ), '')
             FROM cards
             JOIN clozes ON clozes.id = cards.cloze_id
             JOIN source_items ON source_items.id = clozes.source_item_id
             WHERE source_items.deck_id = ?1
               AND (
                    (?2 = 1 AND source_items.deleted_at_ms IS NOT NULL)
                    OR (?2 = 0 AND source_items.deleted_at_ms IS NULL)
               )
             ORDER BY
                source_items.updated_at_ms DESC,
                source_items.id,
                (
                    SELECT semantic_segments.ordinal
                    FROM semantic_segments
                    WHERE semantic_segments.cloze_id = clozes.id
                ),
                clozes.id",
        )?;
        Ok(statement
            .query_map(params![deck_id, trashed], |row| {
                Ok(StoredDeckCardSearch {
                    card_id: row.get(0)?,
                    text: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Loads one bounded page from the card-list projection.
    ///
    /// `matching_card_ids` must retain the order returned by
    /// [`Storage::deck_card_search`].
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the page cannot be read or a matching card
    /// no longer belongs to the requested deck and trash state.
    pub fn deck_card_page(
        &self,
        deck_id: &str,
        trashed: bool,
        matching_card_ids: Option<&[String]>,
        offset: u32,
        limit: u32,
    ) -> Result<StoredDeckCardPage, StorageError> {
        if limit == 0 {
            return Err(StorageError::InvalidAggregate(
                "a deck card page requires a positive limit".into(),
            ));
        }
        let query = match matching_card_ids {
            Some(card_ids) => {
                let total_matches = u64::try_from(card_ids.len())
                    .map_err(|_| StorageError::NumericRange("deck card match count"))?;
                let start = usize::try_from(offset).unwrap_or(usize::MAX);
                let Some(remaining_ids) = card_ids.get(start..).filter(|ids| !ids.is_empty())
                else {
                    return Ok(StoredDeckCardPage {
                        cards: Vec::new(),
                        total_matches,
                    });
                };
                matching_deck_card_page_query(deck_id, trashed, remaining_ids, limit, total_matches)
            }
            None => all_deck_card_page_query(&self.connection, deck_id, trashed, offset, limit)?,
        };
        let cards = load_deck_card_rows(&self.connection, &query.sql, &query.parameters)?;
        if cards.len() != query.expected_rows {
            return Err(StorageError::InvalidAggregate(
                "the deck card page changed while it was loading".into(),
            ));
        }
        Ok(StoredDeckCardPage {
            cards,
            total_matches: query.total_matches,
        })
    }

    /// Moves one card into an independent source record without changing its
    /// card, schedule, or review identities.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the requested card and isolated record do
    /// not describe the same cloze, or when the transaction cannot be committed.
    pub fn isolate_card_source(
        &mut self,
        card_id: &str,
        isolated: &StoredSourceNote,
    ) -> Result<(), StorageError> {
        validate_note(isolated)?;
        if isolated.clozes.len() != 1 {
            return Err(StorageError::InvalidAggregate(
                "an isolated card source must contain exactly one cloze".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let (cloze_id, source_id) = transaction
            .query_row(
                "SELECT cards.cloze_id, clozes.source_item_id
                 FROM cards
                 JOIN clozes ON clozes.id = cards.cloze_id
                 WHERE cards.id = ?1",
                [card_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or_else(|| entity_not_found("card", card_id))?;
        let cloze_count = transaction.query_row(
            "SELECT COUNT(*) FROM clozes WHERE source_item_id = ?1",
            [&source_id],
            |row| row.get::<_, u64>(0),
        )?;
        if cloze_count == 1 {
            return Ok(());
        }
        let isolated_cloze = &isolated.clozes[0];
        if isolated_cloze.id != cloze_id
            || isolated_cloze.source_item_id != isolated.source_item.id
            || isolated.source_item.id == source_id
        {
            return Err(StorageError::InvalidAggregate(
                "an isolated card source must preserve its selected cloze".into(),
            ));
        }
        let deleted_at_ms = transaction.query_row(
            "SELECT deleted_at_ms FROM source_items WHERE id = ?1",
            [&source_id],
            |row| row.get::<_, Option<i64>>(0),
        )?;

        insert_source(&transaction, &isolated.source_item)?;
        transaction.execute(
            "UPDATE source_items SET deleted_at_ms = ?1 WHERE id = ?2",
            params![deleted_at_ms, isolated.source_item.id],
        )?;
        for tag in &isolated.source_item.tags {
            upsert_tag(&transaction, tag)?;
        }
        insert_source_tag_links(&transaction, &isolated.source_item)?;
        insert_owned_annotations(
            &transaction,
            "source_item_annotations",
            "source_item_id",
            &isolated.source_item.id,
            &isolated.source_item.annotations,
        )?;
        for media in &isolated.source_item.media {
            upsert_media(&transaction, media)?;
        }
        insert_source_media_links(&transaction, &isolated.source_item)?;
        insert_segments(&transaction, &isolated.source_item)?;

        let changed_segment = transaction.execute(
            "UPDATE semantic_segments
             SET kind = 'text', cloze_id = NULL
             WHERE source_item_id = ?1 AND cloze_id = ?2",
            params![source_id, cloze_id],
        )?;
        ensure_changed(changed_segment, "card segment", &cloze_id)?;
        let changed_cloze = transaction.execute(
            "UPDATE clozes SET source_item_id = ?1
             WHERE id = ?2 AND source_item_id = ?3",
            params![isolated.source_item.id, cloze_id, source_id],
        )?;
        ensure_changed(changed_cloze, "card cloze", &cloze_id)?;
        transaction.execute(
            "UPDATE source_items SET updated_at_ms = ?1 WHERE id = ?2",
            params![isolated.source_item.updated_at_ms, source_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Suspends or unsuspends only the selected card identities.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the selection is invalid or a write fails.
    pub fn set_deck_cards_suspended(
        &mut self,
        card_ids: &[String],
        suspended: bool,
        updated_at_ms: i64,
    ) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        ensure_cards_exist(&transaction, card_ids)?;
        for card_id in card_ids {
            transaction.execute(
                "UPDATE cards
                 SET suspended = ?1, updated_at_ms = ?2, queue_updated_at_ms = ?2
                 WHERE id = ?3",
                params![suspended, updated_at_ms, card_id],
            )?;
            transaction.execute(
                "UPDATE source_items
                 SET updated_at_ms = ?1
                 WHERE id = (
                    SELECT clozes.source_item_id
                    FROM cards JOIN clozes ON clozes.id = cards.cloze_id
                    WHERE cards.id = ?2
                 )",
                params![updated_at_ms, card_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Moves isolated card sources into or out of recoverable Trash.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the selection is invalid or a write fails.
    pub fn set_deck_cards_deleted(
        &mut self,
        card_ids: &[String],
        deleted_at_ms: Option<i64>,
        updated_at_ms: i64,
    ) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        ensure_cards_exist(&transaction, card_ids)?;
        for card_id in card_ids {
            transaction.execute(
                "UPDATE source_items
                 SET deleted_at_ms = ?1, updated_at_ms = ?2
                 WHERE id = (
                    SELECT clozes.source_item_id
                    FROM cards JOIN clozes ON clozes.id = cards.cloze_id
                    WHERE cards.id = ?3
                 )",
                params![deleted_at_ms, updated_at_ms, card_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Moves isolated card sources to an existing flat deck.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when a card or destination deck is missing.
    pub fn move_deck_cards(
        &mut self,
        card_ids: &[String],
        deck_id: &str,
        updated_at_ms: i64,
    ) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        ensure_entity_exists(&transaction, "decks", "deck", deck_id)?;
        ensure_cards_exist(&transaction, card_ids)?;
        for card_id in card_ids {
            transaction.execute(
                "UPDATE source_items
                 SET deck_id = ?1, updated_at_ms = ?2
                 WHERE id = (
                    SELECT clozes.source_item_id
                    FROM cards JOIN clozes ON clozes.id = cards.cloze_id
                    WHERE cards.id = ?3
                 )",
                params![deck_id, updated_at_ms, card_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Loads a card with its full source aggregate and projected schedule.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the card is missing or stored data cannot
    /// be decoded.
    pub fn load_study_card(&self, card_id: &str) -> Result<StoredStudyCard, StorageError> {
        let (card, source_item_id) = self
            .connection
            .query_row(
                "SELECT cards.id, clozes.source_item_id
                 FROM cards
                 JOIN clozes ON clozes.id = cards.cloze_id
                 WHERE cards.id = ?1",
                [card_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
        let note = load_source_note(&self.connection, &source_item_id)?;
        let card = load_card(&self.connection, &card)?;
        let cloze = note
            .clozes
            .into_iter()
            .find(|cloze| cloze.id == card.cloze_id)
            .ok_or_else(|| entity_not_found("cloze", &card.cloze_id))?;
        let schedule = self.load_schedule(card_id)?;
        Ok(StoredStudyCard {
            source_item: note.source_item,
            cloze,
            card,
            schedule,
        })
    }

    /// Loads the current projected schedule for a card.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the card is missing or the query fails.
    pub fn load_schedule(&self, card_id: &str) -> Result<ScheduleState, StorageError> {
        load_schedule_row(&self.connection, "schedule_states", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))
    }
}

fn insert_note_children(
    transaction: &Transaction<'_>,
    note: &StoredSourceNote,
) -> Result<(), StorageError> {
    for tag in &note.source_item.tags {
        upsert_tag(transaction, tag)?;
    }
    for media in &note.source_item.media {
        upsert_media(transaction, media)?;
    }
    insert_owned_annotations(
        transaction,
        "source_item_annotations",
        "source_item_id",
        &note.source_item.id,
        &note.source_item.annotations,
    )?;
    insert_source_tag_links(transaction, &note.source_item)?;
    insert_source_media_links(transaction, &note.source_item)?;

    for cloze in &note.clozes {
        insert_cloze(transaction, cloze)?;
        insert_cloze_children(transaction, cloze)?;
    }
    insert_segments(transaction, &note.source_item)?;
    Ok(())
}

fn insert_cloze_children(transaction: &Transaction<'_>, cloze: &Cloze) -> Result<(), StorageError> {
    for media in &cloze.media {
        upsert_media(transaction, media)?;
    }
    insert_owned_annotations(
        transaction,
        "cloze_annotations",
        "cloze_id",
        &cloze.id,
        &cloze.annotations,
    )?;
    insert_cloze_media_links(transaction, cloze)
}

fn validate_note(note: &StoredSourceNote) -> Result<(), StorageError> {
    let clozes = note
        .clozes
        .iter()
        .map(|cloze| (cloze.id.as_str(), cloze))
        .collect::<HashMap<_, _>>();
    if clozes.len() != note.clozes.len() {
        return Err(StorageError::InvalidAggregate(
            "cloze identifiers must be unique".into(),
        ));
    }
    for cloze in &note.clozes {
        if cloze.source_item_id != note.source_item.id {
            return Err(StorageError::InvalidAggregate(format!(
                "cloze {} belongs to a different source note",
                cloze.id
            )));
        }
    }
    for (index, segment) in note.source_item.segments.iter().enumerate() {
        let ordinal = u32::try_from(index).map_err(|_| StorageError::NumericRange("ordinal"))?;
        if segment.ordinal != ordinal {
            return Err(StorageError::InvalidAggregate(
                "segment ordinals must be contiguous and match vector order".into(),
            ));
        }
        if let SegmentContent::Cloze { cloze_id, text } = &segment.content {
            let cloze = clozes.get(cloze_id.as_str()).ok_or_else(|| {
                StorageError::InvalidAggregate(format!(
                    "segment {} references missing cloze {cloze_id}",
                    segment.id
                ))
            })?;
            if cloze.answer != *text {
                return Err(StorageError::InvalidAggregate(format!(
                    "segment {} does not preserve cloze {} surface text",
                    segment.id, cloze.id
                )));
            }
        }
    }
    let referenced = note
        .source_item
        .segments
        .iter()
        .filter_map(|segment| match &segment.content {
            SegmentContent::Cloze { cloze_id, .. } => Some(cloze_id.as_str()),
            SegmentContent::Text(_) => None,
        })
        .collect::<HashSet<_>>();
    if referenced.len() != clozes.len() || !clozes.keys().all(|id| referenced.contains(id)) {
        return Err(StorageError::InvalidAggregate(
            "each cloze must appear in exactly one semantic segment".into(),
        ));
    }
    Ok(())
}

fn insert_source(connection: &Connection, source: &SourceItem) -> Result<(), StorageError> {
    let (explanation, explanation_language, explanation_direction) =
        localized_text_columns(source.explanation.as_ref());
    connection.execute(
        "INSERT INTO source_items(
            id,
            language_tag,
            direction,
            created_at_ms,
            deck_id,
            explanation,
            explanation_language_tag,
            explanation_direction,
            updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            source.id,
            source.language_tag,
            direction_to_database(source.direction),
            source.created_at_ms,
            source.deck_id,
            explanation,
            explanation_language,
            explanation_direction,
            source.updated_at_ms,
        ],
    )?;
    Ok(())
}

fn update_source(connection: &Connection, source: &SourceItem) -> Result<usize, StorageError> {
    let (explanation, explanation_language, explanation_direction) =
        localized_text_columns(source.explanation.as_ref());
    Ok(connection.execute(
        "UPDATE source_items
         SET deck_id = ?1,
             language_tag = ?2,
             direction = ?3,
             explanation = ?4,
             explanation_language_tag = ?5,
             explanation_direction = ?6,
             updated_at_ms = ?7
         WHERE id = ?8",
        params![
            source.deck_id,
            source.language_tag,
            direction_to_database(source.direction),
            explanation,
            explanation_language,
            explanation_direction,
            source.updated_at_ms,
            source.id,
        ],
    )?)
}

fn load_source_note(connection: &Connection, id: &str) -> Result<StoredSourceNote, StorageError> {
    type StoredSource = (
        String,
        String,
        Option<String>,
        String,
        i64,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let stored = connection
        .query_row(
            "SELECT
                id,
                deck_id,
                language_tag,
                direction,
                created_at_ms,
                updated_at_ms,
                explanation,
                explanation_language_tag,
                explanation_direction
             FROM source_items
             WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| entity_not_found("source note", id))?;
    let stored: StoredSource = stored;
    let segments = load_segments(connection, id)?;
    let tags = load_source_tags(connection, id)?;
    let annotations =
        load_owned_annotations(connection, "source_item_annotations", "source_item_id", id)?;
    let media = load_linked_media(connection, "source_item_media", "source_item_id", id)?;
    let cloze_ids = source_cloze_ids(connection, id)?;
    let clozes = cloze_ids
        .iter()
        .map(|cloze_id| load_cloze(connection, cloze_id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(StoredSourceNote {
        source_item: SourceItem {
            id: stored.0,
            deck_id: stored.1,
            segments,
            language_tag: stored.2,
            direction: direction_from_database(&stored.3)?,
            tags,
            annotations,
            explanation: localized_text_from_columns(stored.6, stored.7, stored.8.as_deref())?,
            media,
            created_at_ms: stored.4,
            updated_at_ms: stored.5,
        },
        clozes,
    })
}

fn insert_cloze(connection: &Connection, cloze: &Cloze) -> Result<(), StorageError> {
    let accepted_answers = serde_json::to_string(&cloze.accepted_answers)?;
    let (hint, hint_language, hint_direction) = localized_text_columns(cloze.hint.as_ref());
    let (explanation, explanation_language, explanation_direction) =
        localized_text_columns(cloze.explanation.as_ref());
    connection.execute(
        "INSERT INTO clozes(
            id,
            source_item_id,
            answer,
            accepted_answers_json,
            hint,
            hint_language_tag,
            hint_direction,
            language_tag,
            direction,
            matching_policy,
            explanation,
            explanation_language_tag,
            explanation_direction,
            created_at_ms,
            updated_at_ms
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
         )",
        params![
            cloze.id,
            cloze.source_item_id,
            cloze.answer,
            accepted_answers,
            hint,
            hint_language,
            hint_direction,
            cloze.language_tag,
            direction_to_database(cloze.direction),
            cloze.matching_policy.map(matching_policy_to_database),
            explanation,
            explanation_language,
            explanation_direction,
            cloze.created_at_ms,
            cloze.updated_at_ms,
        ],
    )?;
    Ok(())
}

fn update_cloze_row(connection: &Connection, cloze: &Cloze) -> Result<usize, StorageError> {
    let accepted_answers = serde_json::to_string(&cloze.accepted_answers)?;
    let (hint, hint_language, hint_direction) = localized_text_columns(cloze.hint.as_ref());
    let (explanation, explanation_language, explanation_direction) =
        localized_text_columns(cloze.explanation.as_ref());
    Ok(connection.execute(
        "UPDATE clozes
         SET answer = ?1,
             accepted_answers_json = ?2,
             hint = ?3,
             hint_language_tag = ?4,
             hint_direction = ?5,
             language_tag = ?6,
             direction = ?7,
             matching_policy = ?8,
             explanation = ?9,
             explanation_language_tag = ?10,
             explanation_direction = ?11,
             updated_at_ms = ?12
         WHERE id = ?13 AND source_item_id = ?14",
        params![
            cloze.answer,
            accepted_answers,
            hint,
            hint_language,
            hint_direction,
            cloze.language_tag,
            direction_to_database(cloze.direction),
            cloze.matching_policy.map(matching_policy_to_database),
            explanation,
            explanation_language,
            explanation_direction,
            cloze.updated_at_ms,
            cloze.id,
            cloze.source_item_id,
        ],
    )?)
}

fn load_cloze(connection: &Connection, id: &str) -> Result<Cloze, StorageError> {
    type StoredCloze = (
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
        i64,
    );
    let stored = connection
        .query_row(
            "SELECT
                id,
                source_item_id,
                answer,
                accepted_answers_json,
                hint,
                hint_language_tag,
                hint_direction,
                language_tag,
                direction,
                matching_policy,
                explanation,
                explanation_language_tag,
                explanation_direction,
                created_at_ms,
                updated_at_ms
             FROM clozes
             WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| entity_not_found("cloze", id))?;
    let stored: StoredCloze = stored;
    Ok(Cloze {
        id: stored.0,
        source_item_id: stored.1,
        answer: stored.2,
        accepted_answers: serde_json::from_str(&stored.3)?,
        hint: localized_text_from_columns(stored.4, stored.5, stored.6.as_deref())?,
        language_tag: stored.7,
        direction: direction_from_database(&stored.8)?,
        matching_policy: stored
            .9
            .as_deref()
            .map(matching_policy_from_database)
            .transpose()?,
        annotations: load_owned_annotations(connection, "cloze_annotations", "cloze_id", id)?,
        explanation: localized_text_from_columns(stored.10, stored.11, stored.12.as_deref())?,
        media: load_linked_media(connection, "cloze_media", "cloze_id", id)?,
        created_at_ms: stored.13,
        updated_at_ms: stored.14,
    })
}

fn insert_segments(connection: &Connection, source: &SourceItem) -> Result<(), StorageError> {
    for segment in &source.segments {
        let (kind, text, cloze_id) = match &segment.content {
            SegmentContent::Text(text) => ("text", text.as_str(), None),
            SegmentContent::Cloze { cloze_id, text } => {
                ("cloze", text.as_str(), Some(cloze_id.as_str()))
            }
        };
        connection.execute(
            "INSERT INTO semantic_segments(
                id, source_item_id, ordinal, kind, text, cloze_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![segment.id, source.id, segment.ordinal, kind, text, cloze_id,],
        )?;
    }
    Ok(())
}

fn load_segments(
    connection: &Connection,
    source_item_id: &str,
) -> Result<Vec<SemanticSegment>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id, ordinal, kind, text, cloze_id
         FROM semantic_segments
         WHERE source_item_id = ?1
         ORDER BY ordinal",
    )?;
    let stored = statement
        .query_map([source_item_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    stored
        .into_iter()
        .map(|(id, ordinal, kind, text, cloze_id)| {
            let content = match (kind.as_str(), cloze_id) {
                ("text", None) => SegmentContent::Text(text),
                ("cloze", Some(cloze_id)) => SegmentContent::Cloze { cloze_id, text },
                _ => {
                    return Err(StorageError::InvalidStoredValue {
                        field: "semantic segment",
                        value: kind,
                    });
                }
            };
            Ok(SemanticSegment {
                id,
                ordinal,
                content,
            })
        })
        .collect()
}

fn insert_deck(connection: &Connection, deck: &Deck) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO decks(
            id,
            name,
            description,
            language_tag,
            direction,
            matching_policy,
            target_retention_basis_points,
            new_cards_per_day,
            maximum_interval_days,
            created_at_ms,
            updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            deck.id,
            deck.name,
            deck.description,
            deck.language_tag,
            direction_to_database(deck.direction),
            matching_policy_to_database(deck.matching_policy),
            deck.settings.target_retention_basis_points,
            deck.settings.new_cards_per_day,
            deck.settings.maximum_interval_days,
            deck.created_at_ms,
            deck.updated_at_ms,
        ],
    )?;
    Ok(())
}

fn insert_default_scheduler_profile(
    connection: &Connection,
    deck_id: &str,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    ensure_bundled_default_scheduler_parameter_set(connection, created_at_ms)?;
    connection.execute(
        "INSERT INTO scheduler_profiles(
            deck_id,
            engine_version,
            active_parameter_set_id,
            day_boundary_minutes,
            updated_at_ms
         ) VALUES (?1, 'fsrs-7', ?2, 240, ?3)",
        params![deck_id, DEFAULT_SCHEDULER_PARAMETER_SET_ID, created_at_ms],
    )?;
    Ok(())
}

fn ensure_bundled_default_scheduler_parameter_set(
    connection: &Connection,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    connection.execute(
        "INSERT OR IGNORE INTO scheduler_parameter_sets(
            id, engine_version, parameters_json, created_at_ms
         ) VALUES (?1, 'fsrs-7', ?2, ?3)",
        params![
            DEFAULT_SCHEDULER_PARAMETER_SET_ID,
            crate::DEFAULT_SCHEDULER_PARAMETERS_JSON,
            created_at_ms,
        ],
    )?;
    Ok(())
}

fn load_scheduler_profile(
    connection: &Connection,
    deck_id: &str,
) -> Result<SchedulerProfile, StorageError> {
    connection
        .query_row(
            "SELECT
                deck_id,
                engine_version,
                active_parameter_set_id,
                scheduling_mode,
                daily_time_budget_minutes,
                controller_version,
                controller_target_retention_basis_points,
                controller_new_cards_per_day,
                controller_last_evaluated_day_start_ms,
                controller_review_count,
                controller_unseen_count,
                controller_forecast_review_seconds_per_day,
                controller_backlog_exceeds_budget,
                controller_explanation,
                day_boundary_minutes,
                updated_at_ms
             FROM scheduler_profiles
             WHERE deck_id = ?1",
            [deck_id],
            |row| {
                Ok(SchedulerProfile {
                    deck_id: row.get(0)?,
                    engine_version: row.get(1)?,
                    active_parameter_set_id: row.get(2)?,
                    scheduling_mode: scheduling_mode_from_database(&row.get::<_, String>(3)?)
                        .map_err(to_sql_conversion_error)?,
                    deck_daily_time_budget_minutes: row.get(4)?,
                    controller_version: row.get(5)?,
                    controller_target_retention_basis_points: row.get(6)?,
                    controller_new_cards_per_day: row.get(7)?,
                    controller_last_evaluated_day_start_ms: row.get(8)?,
                    controller_review_count: row.get(9)?,
                    controller_unseen_count: row.get(10)?,
                    controller_forecast_review_seconds_per_day: row.get(11)?,
                    controller_backlog_exceeds_budget: row.get(12)?,
                    controller_explanation: row.get(13)?,
                    legacy_intensity: meiki_domain::LegacyStudyIntensity::default(),
                    day_boundary_minutes: row.get(14)?,
                    updated_at_ms: row.get(15)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| entity_not_found("scheduler profile", deck_id))
}

fn validate_scheduler_profile(
    connection: &Connection,
    profile: &SchedulerProfile,
) -> Result<(), StorageError> {
    if profile.day_boundary_minutes >= 1_440
        || profile
            .deck_daily_time_budget_minutes
            .is_some_and(|budget| !(1..=1_440).contains(&budget))
        || profile.engine_version.is_empty()
        || profile.controller_version.is_empty()
        || !(8_000..=9_500).contains(&profile.controller_target_retention_basis_points)
        || profile.controller_new_cards_per_day > 10_000
    {
        return Err(StorageError::InvalidAggregate(
            "scheduler profile controls are outside safe bounds".into(),
        ));
    }
    for parameter_set_id in std::iter::once(profile.active_parameter_set_id.as_str()) {
        let version = connection
            .query_row(
                "SELECT engine_version
                 FROM scheduler_parameter_sets
                 WHERE id = ?1",
                [parameter_set_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| entity_not_found("scheduler parameter set", parameter_set_id))?;
        if version != profile.engine_version {
            return Err(StorageError::InvalidAggregate(
                "scheduler profile parameter versions must match its engine version".into(),
            ));
        }
    }
    Ok(())
}

const fn scheduling_mode_to_database(value: SchedulingMode) -> &'static str {
    match value {
        SchedulingMode::Automatic => "automatic",
        SchedulingMode::Expert => "expert",
    }
}

fn scheduling_mode_from_database(value: &str) -> Result<SchedulingMode, StorageError> {
    match value {
        "automatic" => Ok(SchedulingMode::Automatic),
        "expert" => Ok(SchedulingMode::Expert),
        _ => Err(StorageError::InvalidStoredValue {
            field: "scheduling mode",
            value: value.to_owned(),
        }),
    }
}

fn load_deck(connection: &Connection, id: &str) -> Result<Deck, StorageError> {
    connection
        .query_row(
            "SELECT
                id,
                name,
                description,
                language_tag,
                direction,
                matching_policy,
                target_retention_basis_points,
                new_cards_per_day,
                maximum_interval_days,
                created_at_ms,
                updated_at_ms
             FROM decks
             WHERE id = ?1",
            [id],
            |row| {
                Ok(Deck {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    language_tag: row.get(3)?,
                    direction: direction_from_database(&row.get::<_, String>(4)?)
                        .map_err(to_sql_conversion_error)?,
                    matching_policy: matching_policy_from_database(&row.get::<_, String>(5)?)
                        .map_err(to_sql_conversion_error)?,
                    settings: meiki_domain::StudySettingsOverride {
                        target_retention_basis_points: row.get(6)?,
                        new_cards_per_day: row.get(7)?,
                        maximum_interval_days: row.get(8)?,
                    },
                    created_at_ms: row.get(9)?,
                    updated_at_ms: row.get(10)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| entity_not_found("deck", id))
}

fn to_sql_conversion_error(error: StorageError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn insert_tag(connection: &Connection, tag: &Tag) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO tags(id, name, created_at_ms, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4)",
        params![tag.id, tag.name, tag.created_at_ms, tag.updated_at_ms],
    )?;
    Ok(())
}

fn upsert_tag(connection: &Connection, tag: &Tag) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO tags(id, name, created_at_ms, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE
         SET name = excluded.name, updated_at_ms = excluded.updated_at_ms",
        params![tag.id, tag.name, tag.created_at_ms, tag.updated_at_ms],
    )?;
    Ok(())
}

fn load_tag(connection: &Connection, id: &str) -> Result<Tag, StorageError> {
    connection
        .query_row(
            "SELECT id, name, created_at_ms, updated_at_ms
             FROM tags
             WHERE id = ?1",
            [id],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at_ms: row.get(2)?,
                    updated_at_ms: row.get(3)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| entity_not_found("tag", id))
}

fn insert_annotation(connection: &Connection, annotation: &Annotation) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO annotations(id, label, value, language_tag, direction)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            annotation.id,
            annotation.label,
            annotation.value,
            annotation.language_tag,
            direction_to_database(annotation.direction),
        ],
    )?;
    Ok(())
}

fn load_annotation(connection: &Connection, id: &str) -> Result<Annotation, StorageError> {
    let stored = connection
        .query_row(
            "SELECT id, label, value, language_tag, direction
             FROM annotations
             WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| entity_not_found("annotation", id))?;
    Ok(Annotation {
        id: stored.0,
        label: stored.1,
        value: stored.2,
        language_tag: stored.3,
        direction: direction_from_database(&stored.4)?,
    })
}

fn insert_media(connection: &Connection, media: &MediaReference) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO media_references(
            id,
            content_hash,
            kind,
            role,
            media_type,
            byte_size,
            original_file_name,
            alt_text,
            width,
            height,
            duration_ms,
            language_tag,
            direction,
            created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            media.id,
            media.content_hash,
            media_kind_to_database(media.kind),
            media_role_to_database(media.role),
            media.media_type,
            media.byte_size,
            media.original_file_name,
            media.alt_text,
            media.width,
            media.height,
            media.duration_ms,
            media.language_tag,
            direction_to_database(media.direction),
            media.created_at_ms,
        ],
    )?;
    Ok(())
}

fn upsert_media(connection: &Connection, media: &MediaReference) -> Result<(), StorageError> {
    let changed = connection.execute(
        "INSERT INTO media_references(
            id,
            content_hash,
            kind,
            role,
            media_type,
            byte_size,
            original_file_name,
            alt_text,
            width,
            height,
            duration_ms,
            language_tag,
            direction,
            created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(id) DO UPDATE SET
            role = excluded.role,
            original_file_name = excluded.original_file_name,
            alt_text = excluded.alt_text,
            language_tag = excluded.language_tag,
            direction = excluded.direction
         WHERE
            media_references.content_hash = excluded.content_hash
            AND media_references.kind = excluded.kind
            AND media_references.media_type = excluded.media_type
            AND media_references.byte_size = excluded.byte_size
            AND media_references.width IS excluded.width
            AND media_references.height IS excluded.height
            AND media_references.duration_ms IS excluded.duration_ms",
        params![
            media.id,
            media.content_hash,
            media_kind_to_database(media.kind),
            media_role_to_database(media.role),
            media.media_type,
            media.byte_size,
            media.original_file_name,
            media.alt_text,
            media.width,
            media.height,
            media.duration_ms,
            media.language_tag,
            direction_to_database(media.direction),
            media.created_at_ms,
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::InvalidAggregate(format!(
            "media reference {} cannot change its stored object identity",
            media.id
        )));
    }
    Ok(())
}

fn load_media(connection: &Connection, id: &str) -> Result<MediaReference, StorageError> {
    let stored = connection
        .query_row(
            "SELECT
                id,
                content_hash,
                kind,
                role,
                media_type,
                byte_size,
                original_file_name,
                alt_text,
                width,
                height,
                duration_ms,
                language_tag,
                direction,
                created_at_ms
             FROM media_references
             WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, u64>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<u32>>(8)?,
                    row.get::<_, Option<u32>>(9)?,
                    row.get::<_, Option<u64>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, i64>(13)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| entity_not_found("media reference", id))?;
    Ok(MediaReference {
        id: stored.0,
        content_hash: stored.1,
        kind: media_kind_from_database(&stored.2)?,
        role: media_role_from_database(&stored.3)?,
        media_type: stored.4,
        byte_size: stored.5,
        original_file_name: stored.6,
        alt_text: stored.7,
        width: stored.8,
        height: stored.9,
        duration_ms: stored.10,
        language_tag: stored.11,
        direction: direction_from_database(&stored.12)?,
        created_at_ms: stored.13,
    })
}

fn source_cloze_ids(
    connection: &Connection,
    source_item_id: &str,
) -> Result<Vec<String>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT clozes.id
         FROM clozes
         LEFT JOIN semantic_segments
            ON semantic_segments.cloze_id = clozes.id
         WHERE clozes.source_item_id = ?1
         GROUP BY clozes.id
         ORDER BY MIN(semantic_segments.ordinal), clozes.id",
    )?;
    Ok(statement
        .query_map([source_item_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn insert_source_tag_links(
    connection: &Connection,
    source: &SourceItem,
) -> Result<(), StorageError> {
    for (index, tag) in source.tags.iter().enumerate() {
        connection.execute(
            "INSERT INTO source_item_tags(source_item_id, tag_id, ordinal)
             VALUES (?1, ?2, ?3)",
            params![source.id, tag.id, ordinal(index)?],
        )?;
    }
    Ok(())
}

fn replace_source_tags(connection: &Connection, source: &SourceItem) -> Result<(), StorageError> {
    connection.execute(
        "DELETE FROM source_item_tags WHERE source_item_id = ?1",
        [&source.id],
    )?;
    for tag in &source.tags {
        upsert_tag(connection, tag)?;
    }
    insert_source_tag_links(connection, source)
}

fn load_source_tags(
    connection: &Connection,
    source_item_id: &str,
) -> Result<Vec<Tag>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT tags.id, tags.name, tags.created_at_ms, tags.updated_at_ms
         FROM source_item_tags
         JOIN tags ON tags.id = source_item_tags.tag_id
         WHERE source_item_tags.source_item_id = ?1
         ORDER BY source_item_tags.ordinal",
    )?;
    Ok(statement
        .query_map([source_item_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at_ms: row.get(2)?,
                updated_at_ms: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

fn insert_owned_annotations(
    connection: &Connection,
    link_table: &str,
    owner_column: &str,
    owner_id: &str,
    annotations: &[Annotation],
) -> Result<(), StorageError> {
    for (index, annotation) in annotations.iter().enumerate() {
        insert_annotation(connection, annotation)?;
        let sql = format!(
            "INSERT INTO {link_table}({owner_column}, annotation_id, ordinal)
             VALUES (?1, ?2, ?3)"
        );
        connection.execute(&sql, params![owner_id, annotation.id, ordinal(index)?])?;
    }
    Ok(())
}

fn replace_owned_annotations(
    connection: &Connection,
    link_table: &str,
    owner_column: &str,
    owner_id: &str,
    annotations: &[Annotation],
) -> Result<(), StorageError> {
    delete_owned_annotations(connection, link_table, owner_column, owner_id)?;
    insert_owned_annotations(connection, link_table, owner_column, owner_id, annotations)
}

fn delete_owned_annotations(
    connection: &Connection,
    link_table: &str,
    owner_column: &str,
    owner_id: &str,
) -> Result<(), StorageError> {
    let select = format!("SELECT annotation_id FROM {link_table} WHERE {owner_column} = ?1");
    let annotation_ids = {
        let mut statement = connection.prepare(&select)?;
        statement
            .query_map([owner_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let delete_links = format!("DELETE FROM {link_table} WHERE {owner_column} = ?1");
    connection.execute(&delete_links, [owner_id])?;
    for annotation_id in annotation_ids {
        connection.execute("DELETE FROM annotations WHERE id = ?1", [&annotation_id])?;
    }
    Ok(())
}

fn load_owned_annotations(
    connection: &Connection,
    link_table: &str,
    owner_column: &str,
    owner_id: &str,
) -> Result<Vec<Annotation>, StorageError> {
    let sql = format!(
        "SELECT
            annotations.id,
            annotations.label,
            annotations.value,
            annotations.language_tag,
            annotations.direction
         FROM {link_table}
         JOIN annotations ON annotations.id = {link_table}.annotation_id
         WHERE {link_table}.{owner_column} = ?1
         ORDER BY {link_table}.ordinal"
    );
    let mut statement = connection.prepare(&sql)?;
    let stored = statement
        .query_map([owner_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    stored
        .into_iter()
        .map(|stored| {
            Ok(Annotation {
                id: stored.0,
                label: stored.1,
                value: stored.2,
                language_tag: stored.3,
                direction: direction_from_database(&stored.4)?,
            })
        })
        .collect()
}

fn insert_source_media_links(
    connection: &Connection,
    source: &SourceItem,
) -> Result<(), StorageError> {
    for (index, media) in source.media.iter().enumerate() {
        connection.execute(
            "INSERT INTO source_item_media(
                source_item_id, media_reference_id, ordinal
             ) VALUES (?1, ?2, ?3)",
            params![source.id, media.id, ordinal(index)?],
        )?;
    }
    Ok(())
}

fn replace_source_media(connection: &Connection, source: &SourceItem) -> Result<(), StorageError> {
    connection.execute(
        "DELETE FROM source_item_media WHERE source_item_id = ?1",
        [&source.id],
    )?;
    for media in &source.media {
        upsert_media(connection, media)?;
    }
    insert_source_media_links(connection, source)
}

fn insert_cloze_media_links(connection: &Connection, cloze: &Cloze) -> Result<(), StorageError> {
    for (index, media) in cloze.media.iter().enumerate() {
        connection.execute(
            "INSERT INTO cloze_media(cloze_id, media_reference_id, ordinal)
             VALUES (?1, ?2, ?3)",
            params![cloze.id, media.id, ordinal(index)?],
        )?;
    }
    Ok(())
}

fn replace_cloze_media(connection: &Connection, cloze: &Cloze) -> Result<(), StorageError> {
    connection.execute("DELETE FROM cloze_media WHERE cloze_id = ?1", [&cloze.id])?;
    for media in &cloze.media {
        upsert_media(connection, media)?;
    }
    insert_cloze_media_links(connection, cloze)
}

fn load_linked_media(
    connection: &Connection,
    link_table: &str,
    owner_column: &str,
    owner_id: &str,
) -> Result<Vec<MediaReference>, StorageError> {
    let sql = format!(
        "SELECT media_references.id
         FROM {link_table}
         JOIN media_references
            ON media_references.id = {link_table}.media_reference_id
         WHERE {link_table}.{owner_column} = ?1
         ORDER BY {link_table}.ordinal"
    );
    let media_ids = {
        let mut statement = connection.prepare(&sql)?;
        statement
            .query_map([owner_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    media_ids
        .iter()
        .map(|media_id| load_media(connection, media_id))
        .collect()
}

fn load_card(connection: &Connection, id: &str) -> Result<Card, StorageError> {
    connection
        .query_row(
            "SELECT
                id,
                cloze_id,
                content_version,
                suspended,
                created_at_ms,
                updated_at_ms
             FROM cards
             WHERE id = ?1",
            [id],
            |row| {
                Ok(Card {
                    id: row.get(0)?,
                    cloze_id: row.get(1)?,
                    content_version: row.get(2)?,
                    suspended: row.get(3)?,
                    created_at_ms: row.get(4)?,
                    updated_at_ms: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| entity_not_found("card", id))
}

fn insert_schedule(
    connection: &Connection,
    table: &str,
    schedule: &ScheduleState,
) -> Result<(), StorageError> {
    let sql = format!(
        "INSERT INTO {table}(
            card_id,
            version,
            lifecycle,
            due_at_ms,
            ideal_due_at_ms,
            interval_milliseconds,
            interval_seconds,
            repetitions,
            stability_milliseconds,
            difficulty_millipoints,
            last_reviewed_at_ms,
            last_review_event_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
    );
    connection.execute(
        &sql,
        params![
            schedule.card_id,
            schedule.version,
            card_lifecycle_to_database(schedule.lifecycle),
            schedule.due_at_ms,
            schedule.ideal_due_at_ms,
            schedule.interval_milliseconds,
            schedule.interval_seconds,
            schedule.repetitions,
            schedule.stability_milliseconds,
            schedule.difficulty_millipoints,
            schedule.last_reviewed_at_ms,
            schedule.last_review_event_id,
        ],
    )?;
    Ok(())
}

pub(crate) fn load_schedule_row(
    connection: &Connection,
    table: &str,
    card_id: &str,
) -> Result<Option<ScheduleState>, StorageError> {
    let sql = format!(
        "SELECT
            card_id, version, lifecycle, due_at_ms, ideal_due_at_ms,
            interval_milliseconds, interval_seconds, repetitions,
            stability_milliseconds, difficulty_millipoints,
            last_reviewed_at_ms, last_review_event_id
         FROM {table}
         WHERE card_id = ?1"
    );
    Ok(connection
        .query_row(&sql, [card_id], |row| {
            Ok(ScheduleState {
                card_id: row.get(0)?,
                version: row.get(1)?,
                lifecycle: card_lifecycle_from_database(&row.get::<_, String>(2)?)
                    .map_err(database_decode_error)?,
                due_at_ms: row.get(3)?,
                ideal_due_at_ms: row.get(4)?,
                interval_milliseconds: row.get(5)?,
                interval_seconds: row.get(6)?,
                repetitions: row.get(7)?,
                stability_milliseconds: row.get(8)?,
                difficulty_millipoints: row.get(9)?,
                last_reviewed_at_ms: row.get(10)?,
                last_review_event_id: row.get(11)?,
            })
        })
        .optional()?)
}

pub(crate) const fn card_lifecycle_to_database(value: CardLifecycle) -> &'static str {
    match value {
        CardLifecycle::Unseen => "unseen",
        CardLifecycle::Introduced => "introduced",
    }
}

pub(crate) fn card_lifecycle_from_database(value: &str) -> Result<CardLifecycle, StorageError> {
    match value {
        "unseen" => Ok(CardLifecycle::Unseen),
        "introduced" => Ok(CardLifecycle::Introduced),
        _ => Err(StorageError::InvalidStoredValue {
            field: "card lifecycle",
            value: value.to_owned(),
        }),
    }
}

fn database_decode_error(error: StorageError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(error))
}

fn localized_text_columns(
    localized: Option<&LocalizedText>,
) -> (Option<&str>, Option<&str>, Option<&str>) {
    localized.map_or((None, None, None), |localized| {
        (
            Some(localized.value.as_str()),
            localized.language_tag.as_deref(),
            Some(direction_to_database(localized.direction)),
        )
    })
}

fn localized_text_from_columns(
    value: Option<String>,
    language_tag: Option<String>,
    direction: Option<&str>,
) -> Result<Option<LocalizedText>, StorageError> {
    value
        .map(|value| {
            Ok(LocalizedText {
                value,
                language_tag,
                direction: direction_from_database(direction.unwrap_or("auto"))?,
            })
        })
        .transpose()
}

const fn media_kind_to_database(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Audio => "audio",
        MediaKind::Image => "image",
    }
}

fn media_kind_from_database(value: &str) -> Result<MediaKind, StorageError> {
    match value {
        "audio" => Ok(MediaKind::Audio),
        "image" => Ok(MediaKind::Image),
        _ => Err(StorageError::InvalidStoredValue {
            field: "media kind",
            value: value.to_owned(),
        }),
    }
}

const fn media_role_to_database(role: MediaRole) -> &'static str {
    match role {
        MediaRole::PromptAudio => "prompt_audio",
        MediaRole::AnswerAudio => "answer_audio",
        MediaRole::RevealImage => "reveal_image",
    }
}

fn media_role_from_database(value: &str) -> Result<MediaRole, StorageError> {
    match value {
        "prompt_audio" => Ok(MediaRole::PromptAudio),
        "answer_audio" => Ok(MediaRole::AnswerAudio),
        "reveal_image" => Ok(MediaRole::RevealImage),
        _ => Err(StorageError::InvalidStoredValue {
            field: "media role",
            value: value.to_owned(),
        }),
    }
}

fn ordinal(index: usize) -> Result<u32, StorageError> {
    u32::try_from(index).map_err(|_| StorageError::NumericRange("ordinal"))
}

fn ensure_changed(changed: usize, entity: &'static str, id: &str) -> Result<(), StorageError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(entity_not_found(entity, id))
    }
}

struct DeckCardPageQuery {
    total_matches: u64,
    sql: String,
    parameters: Vec<Value>,
    expected_rows: usize,
}

fn matching_deck_card_page_query(
    deck_id: &str,
    trashed: bool,
    remaining_ids: &[String],
    limit: u32,
    total_matches: u64,
) -> DeckCardPageQuery {
    let page_ids = remaining_ids
        .iter()
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .collect::<Vec<_>>();
    let expected_rows = page_ids.len();
    let requested = page_ids
        .iter()
        .enumerate()
        .map(|(position, _)| format!("(?, {position})"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "WITH requested(card_id, position) AS (VALUES {requested})
         SELECT
            cards.id,
            COALESCE((
                SELECT group_concat(
                    CASE
                        WHEN semantic_segments.cloze_id = clozes.id THEN '[…]'
                        ELSE semantic_segments.text
                    END,
                    '' ORDER BY semantic_segments.ordinal
                )
                FROM semantic_segments
                WHERE semantic_segments.source_item_id = source_items.id
            ), ''),
            clozes.answer,
            cards.suspended,
            schedule_states.lifecycle,
            schedule_states.due_at_ms,
            COALESCE(
                clozes.language_tag,
                source_items.language_tag,
                decks.language_tag
            ),
            CASE
                WHEN clozes.direction != 'auto' THEN clozes.direction
                WHEN source_items.direction != 'auto' THEN source_items.direction
                ELSE decks.direction
            END
         FROM requested
         JOIN cards ON cards.id = requested.card_id
         JOIN clozes ON clozes.id = cards.cloze_id
         JOIN source_items ON source_items.id = clozes.source_item_id
         JOIN decks ON decks.id = source_items.deck_id
         JOIN schedule_states ON schedule_states.card_id = cards.id
         WHERE source_items.deck_id = ?
           AND (
                (? = 1 AND source_items.deleted_at_ms IS NOT NULL)
                OR (? = 0 AND source_items.deleted_at_ms IS NULL)
           )
         ORDER BY requested.position"
    );
    let mut parameters = page_ids
        .into_iter()
        .map(|card_id| Value::Text((*card_id).clone()))
        .collect::<Vec<_>>();
    parameters.push(Value::Text(deck_id.to_owned()));
    parameters.push(Value::Integer(i64::from(trashed)));
    parameters.push(Value::Integer(i64::from(trashed)));
    DeckCardPageQuery {
        total_matches,
        sql,
        parameters,
        expected_rows,
    }
}

fn all_deck_card_page_query(
    connection: &Connection,
    deck_id: &str,
    trashed: bool,
    offset: u32,
    limit: u32,
) -> Result<DeckCardPageQuery, StorageError> {
    let total_matches = connection.query_row(
        "SELECT COUNT(*)
         FROM cards
         JOIN clozes ON clozes.id = cards.cloze_id
         JOIN source_items ON source_items.id = clozes.source_item_id
         WHERE source_items.deck_id = ?1
           AND (
                (?2 = 1 AND source_items.deleted_at_ms IS NOT NULL)
                OR (?2 = 0 AND source_items.deleted_at_ms IS NULL)
           )",
        params![deck_id, trashed],
        |row| row.get::<_, u64>(0),
    )?;
    let sql = "SELECT
            cards.id,
            COALESCE((
                SELECT group_concat(
                    CASE
                        WHEN semantic_segments.cloze_id = clozes.id THEN '[…]'
                        ELSE semantic_segments.text
                    END,
                    '' ORDER BY semantic_segments.ordinal
                )
                FROM semantic_segments
                WHERE semantic_segments.source_item_id = source_items.id
            ), ''),
            clozes.answer,
            cards.suspended,
            schedule_states.lifecycle,
            schedule_states.due_at_ms,
            COALESCE(
                clozes.language_tag,
                source_items.language_tag,
                decks.language_tag
            ),
            CASE
                WHEN clozes.direction != 'auto' THEN clozes.direction
                WHEN source_items.direction != 'auto' THEN source_items.direction
                ELSE decks.direction
            END
         FROM cards
         JOIN clozes ON clozes.id = cards.cloze_id
         JOIN source_items ON source_items.id = clozes.source_item_id
         JOIN decks ON decks.id = source_items.deck_id
         JOIN schedule_states ON schedule_states.card_id = cards.id
         WHERE source_items.deck_id = ?1
           AND (
                (?2 = 1 AND source_items.deleted_at_ms IS NOT NULL)
                OR (?2 = 0 AND source_items.deleted_at_ms IS NULL)
           )
         ORDER BY
            source_items.updated_at_ms DESC,
            source_items.id,
            (
                SELECT semantic_segments.ordinal
                FROM semantic_segments
                WHERE semantic_segments.cloze_id = clozes.id
            ),
            clozes.id
         LIMIT ?3 OFFSET ?4"
        .to_owned();
    let parameters = vec![
        Value::Text(deck_id.to_owned()),
        Value::Integer(i64::from(trashed)),
        Value::Integer(i64::from(limit)),
        Value::Integer(i64::from(offset)),
    ];
    let remaining = total_matches.saturating_sub(u64::from(offset));
    let expected_rows = usize::try_from(remaining.min(u64::from(limit))).unwrap_or(usize::MAX);
    Ok(DeckCardPageQuery {
        total_matches,
        sql,
        parameters,
        expected_rows,
    })
}

fn load_deck_card_rows(
    connection: &Connection,
    sql: &str,
    parameters: &[Value],
) -> Result<Vec<StoredDeckCard>, StorageError> {
    let mut statement = connection.prepare(sql)?;
    let stored = statement
        .query_map(params_from_iter(parameters.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    stored
        .into_iter()
        .map(
            |(id, sentence, answer, suspended, lifecycle, due_at_ms, language_tag, direction)| {
                Ok(StoredDeckCard {
                    id,
                    sentence,
                    answer,
                    suspended,
                    lifecycle: card_lifecycle_from_database(&lifecycle)?,
                    due_at_ms,
                    language_tag,
                    direction: direction_from_database(&direction)?,
                })
            },
        )
        .collect()
}

fn ensure_cards_exist(connection: &Connection, card_ids: &[String]) -> Result<(), StorageError> {
    if card_ids.is_empty() {
        return Err(StorageError::InvalidAggregate(
            "a card action requires at least one card".to_owned(),
        ));
    }
    let unique = card_ids.iter().collect::<HashSet<_>>();
    if unique.len() != card_ids.len() {
        return Err(StorageError::InvalidAggregate(
            "a card action cannot contain duplicate cards".to_owned(),
        ));
    }
    for card_id in card_ids {
        ensure_entity_exists(connection, "cards", "card", card_id)?;
    }
    Ok(())
}

fn ensure_entity_exists(
    connection: &Connection,
    table: &'static str,
    entity: &'static str,
    id: &str,
) -> Result<(), StorageError> {
    let sql = format!("SELECT 1 FROM {table} WHERE id = ?1");
    let exists = connection
        .query_row(&sql, [id], |_| Ok(()))
        .optional()?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(entity_not_found(entity, id))
    }
}
