use meiki_domain::{ComparisonResult, Grade, ReviewEvent, ScheduleState};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::{Storage, StorageError, repository::load_schedule_row};

impl Storage {
    /// Atomically appends a review event, advances the schedule projection, and
    /// updates queue-relevant card metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::StaleReview`] when observed versions or
    /// snapshots are stale, or another [`StorageError`] when the transaction
    /// fails.
    pub fn commit_review(&mut self, event: &ReviewEvent) -> Result<ScheduleState, StorageError> {
        let transaction = self.connection.transaction()?;
        validate_review_preconditions(&transaction, event)?;
        insert_review_event(&transaction, event)?;

        let changed = transaction.execute(
            "UPDATE schedule_states
             SET version = ?1,
                 due_at_ms = ?2,
                 ideal_due_at_ms = ?3,
                 interval_milliseconds = ?4,
                 interval_seconds = ?5,
                 repetitions = ?6,
                 stability_milliseconds = ?7,
                 difficulty_millipoints = ?8,
                 last_reviewed_at_ms = ?9,
                 last_review_event_id = ?10
             WHERE card_id = ?11
               AND version = ?12
               AND due_at_ms = ?13
               AND ideal_due_at_ms = ?14
               AND interval_milliseconds = ?15
               AND interval_seconds = ?16
               AND repetitions = ?17
               AND stability_milliseconds = ?18
               AND difficulty_millipoints = ?19
               AND last_reviewed_at_ms IS ?20
               AND last_review_event_id IS ?21",
            params![
                event.next_schedule.version,
                event.next_schedule.due_at_ms,
                event.next_schedule.ideal_due_at_ms,
                event.next_schedule.interval_milliseconds,
                event.next_schedule.interval_seconds,
                event.next_schedule.repetitions,
                event.next_schedule.stability_milliseconds,
                event.next_schedule.difficulty_millipoints,
                event.next_schedule.last_reviewed_at_ms,
                event.id,
                event.card_id,
                event.previous_schedule.version,
                event.previous_schedule.due_at_ms,
                event.previous_schedule.ideal_due_at_ms,
                event.previous_schedule.interval_milliseconds,
                event.previous_schedule.interval_seconds,
                event.previous_schedule.repetitions,
                event.previous_schedule.stability_milliseconds,
                event.previous_schedule.difficulty_millipoints,
                event.previous_schedule.last_reviewed_at_ms,
                event.previous_schedule.last_review_event_id,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::StaleReview);
        }

        let changed = transaction.execute(
            "UPDATE cards
             SET queue_updated_at_ms = ?1
             WHERE id = ?2 AND content_version = ?3",
            params![
                event.reviewed_at_ms,
                event.card_id,
                event.card_content_version,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::StaleReview);
        }

        transaction.commit()?;
        let mut committed = event.next_schedule.clone();
        committed.last_review_event_id = Some(event.id.clone());
        Ok(committed)
    }

    /// Loads immutable review events in deterministic chronological order.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when stored values cannot be decoded.
    pub fn review_events(&self, card_id: &str) -> Result<Vec<ReviewEvent>, StorageError> {
        load_review_events(&self.connection, card_id)
    }

    /// Counts immutable review events for a card.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the query fails.
    pub fn review_count(&self, card_id: &str) -> Result<u64, StorageError> {
        let count = self.connection.query_row(
            "SELECT COUNT(*) FROM review_events WHERE card_id = ?1",
            [card_id],
            |row| row.get::<_, u64>(0),
        )?;
        Ok(count)
    }

    /// Loads all immutable review events for a deck in chronological order.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the deck or stored history cannot be
    /// loaded.
    pub fn review_events_for_deck(&self, deck_id: &str) -> Result<Vec<ReviewEvent>, StorageError> {
        let card_ids = self.card_ids_for_deck(deck_id)?;
        let mut events = Vec::new();
        for card_id in card_ids {
            events.extend(load_review_events(&self.connection, &card_id)?);
        }
        events.sort_by(|left, right| {
            left.reviewed_at_ms
                .cmp(&right.reviewed_at_ms)
                .then_with(|| left.card_id.cmp(&right.card_id))
                .then_with(|| {
                    left.previous_schedule
                        .version
                        .cmp(&right.previous_schedule.version)
                })
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(events)
    }

    /// Loads complete study-card aggregates for one deck.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the deck or any aggregate cannot be
    /// loaded.
    pub fn study_cards_for_deck(
        &self,
        deck_id: &str,
    ) -> Result<Vec<crate::StoredStudyCard>, StorageError> {
        self.card_ids_for_deck(deck_id)?
            .into_iter()
            .map(|card_id| self.load_study_card(&card_id))
            .collect()
    }

    /// Atomically replaces every schedule projection in a deck.
    ///
    /// This is used only by an explicit full rebuild. Immutable review events
    /// and baselines are never changed.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the supplied schedules do not exactly
    /// cover the deck or the transaction cannot be committed.
    pub fn replace_schedule_projections(
        &mut self,
        deck_id: &str,
        schedules: &[ScheduleState],
    ) -> Result<(), StorageError> {
        let expected = self.card_ids_for_deck(deck_id)?;
        let mut supplied = schedules
            .iter()
            .map(|schedule| schedule.card_id.as_str())
            .collect::<Vec<_>>();
        supplied.sort_unstable();
        if expected.iter().map(String::as_str).collect::<Vec<_>>() != supplied {
            return Err(StorageError::InvalidAggregate(
                "a full rebuild must provide exactly one schedule for every deck card".into(),
            ));
        }

        let transaction = self.connection.transaction()?;
        for schedule in schedules {
            let changed = transaction.execute(
                "UPDATE schedule_states
                 SET version = ?1,
                     due_at_ms = ?2,
                     ideal_due_at_ms = ?3,
                     interval_milliseconds = ?4,
                     interval_seconds = ?5,
                     repetitions = ?6,
                     stability_milliseconds = ?7,
                     difficulty_millipoints = ?8,
                     last_reviewed_at_ms = ?9,
                     last_review_event_id = ?10
                 WHERE card_id = ?11",
                params![
                    schedule.version,
                    schedule.due_at_ms,
                    schedule.ideal_due_at_ms,
                    schedule.interval_milliseconds,
                    schedule.interval_seconds,
                    schedule.repetitions,
                    schedule.stability_milliseconds,
                    schedule.difficulty_millipoints,
                    schedule.last_reviewed_at_ms,
                    schedule.last_review_event_id,
                    schedule.card_id,
                ],
            )?;
            if changed != 1 {
                return Err(StorageError::CardNotFound(schedule.card_id.clone()));
            }
            transaction.execute(
                "UPDATE cards
                 SET queue_updated_at_ms = ?1
                 WHERE id = ?2",
                params![
                    schedule.last_reviewed_at_ms.unwrap_or(schedule.due_at_ms),
                    schedule.card_id
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Rebuilds the current schedule projection from its baseline and immutable
    /// review events.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::ProjectionMismatch`] when the history is not a
    /// contiguous state transition chain.
    pub fn rebuild_schedule_projection(
        &mut self,
        card_id: &str,
    ) -> Result<ScheduleState, StorageError> {
        let transaction = self.connection.transaction()?;
        let mut projected = load_schedule_row(&transaction, "schedule_baselines", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
        let events = load_review_events(&transaction, card_id)?;
        let mut queue_updated_at_ms = 0;

        for event in events {
            if event.previous_schedule != projected
                || event.next_schedule.card_id != card_id
                || event.next_schedule.version != projected.version + 1
            {
                return Err(StorageError::ProjectionMismatch(format!(
                    "event {} does not continue version {}",
                    event.id, projected.version
                )));
            }
            queue_updated_at_ms = event.reviewed_at_ms;
            projected = event.next_schedule;
        }

        let changed = transaction.execute(
            "UPDATE schedule_states
             SET version = ?1,
                 due_at_ms = ?2,
                 ideal_due_at_ms = ?3,
                 interval_milliseconds = ?4,
                 interval_seconds = ?5,
                 repetitions = ?6,
                 stability_milliseconds = ?7,
                 difficulty_millipoints = ?8,
                 last_reviewed_at_ms = ?9,
                 last_review_event_id = ?10
             WHERE card_id = ?11",
            params![
                projected.version,
                projected.due_at_ms,
                projected.ideal_due_at_ms,
                projected.interval_milliseconds,
                projected.interval_seconds,
                projected.repetitions,
                projected.stability_milliseconds,
                projected.difficulty_millipoints,
                projected.last_reviewed_at_ms,
                projected.last_review_event_id,
                card_id,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::CardNotFound(card_id.to_owned()));
        }
        transaction.execute(
            "UPDATE cards
             SET queue_updated_at_ms = ?1
             WHERE id = ?2",
            params![queue_updated_at_ms, card_id],
        )?;
        transaction.commit()?;
        Ok(projected)
    }

    fn card_ids_for_deck(&self, deck_id: &str) -> Result<Vec<String>, StorageError> {
        let exists = self
            .connection
            .query_row("SELECT 1 FROM decks WHERE id = ?1", [deck_id], |_| Ok(()))
            .optional()?
            .is_some();
        if !exists {
            return Err(crate::entity_not_found("deck", deck_id));
        }
        let mut statement = self.connection.prepare(
            "SELECT cards.id
             FROM cards
             JOIN clozes ON clozes.id = cards.cloze_id
             JOIN source_items ON source_items.id = clozes.source_item_id
             WHERE source_items.deck_id = ?1
             ORDER BY cards.id",
        )?;
        Ok(statement
            .query_map([deck_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?)
    }
}

fn validate_review_preconditions(
    transaction: &Transaction<'_>,
    event: &ReviewEvent,
) -> Result<(), StorageError> {
    let card_version = transaction
        .query_row(
            "SELECT content_version FROM cards WHERE id = ?1",
            [&event.card_id],
            |row| row.get::<_, u64>(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::CardNotFound(event.card_id.clone()))?;
    let stored_schedule = load_schedule_row(transaction, "schedule_states", &event.card_id)?
        .ok_or_else(|| StorageError::CardNotFound(event.card_id.clone()))?;

    if event.previous_schedule.card_id != event.card_id
        || event.next_schedule.card_id != event.card_id
        || event.next_schedule.last_review_event_id.as_deref() != Some(event.id.as_str())
        || card_version != event.card_content_version
        || stored_schedule != event.previous_schedule
        || event.next_schedule.version != event.previous_schedule.version + 1
    {
        return Err(StorageError::StaleReview);
    }
    Ok(())
}

fn insert_review_event(
    transaction: &Transaction<'_>,
    event: &ReviewEvent,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO review_events(
            id,
            card_id,
            card_content_version,
            raw_response,
            normalized_response,
            comparison,
            suggested_grade,
            chosen_grade,
            reviewed_at_ms,
            scheduler_version,
            scheduler_parameter_set_id,
            target_retention_basis_points,
            previous_schedule_version,
            previous_due_at_ms,
            previous_ideal_due_at_ms,
            previous_interval_milliseconds,
            previous_interval_seconds,
            previous_repetitions,
            previous_stability_milliseconds,
            previous_difficulty_millipoints,
            previous_last_reviewed_at_ms,
            next_schedule_version,
            next_due_at_ms,
            next_ideal_due_at_ms,
            next_interval_milliseconds,
            next_interval_seconds,
            next_repetitions,
            next_stability_milliseconds,
            next_difficulty_millipoints,
            next_last_reviewed_at_ms
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30
         )",
        params![
            event.id,
            event.card_id,
            event.card_content_version,
            event.raw_response,
            event.normalized_response,
            comparison_to_database(event.comparison),
            grade_to_database(event.suggested_grade),
            grade_to_database(event.chosen_grade),
            event.reviewed_at_ms,
            event.scheduler_version,
            event.scheduler_parameter_set_id,
            event.target_retention_basis_points,
            event.previous_schedule.version,
            event.previous_schedule.due_at_ms,
            event.previous_schedule.ideal_due_at_ms,
            event.previous_schedule.interval_milliseconds,
            event.previous_schedule.interval_seconds,
            event.previous_schedule.repetitions,
            event.previous_schedule.stability_milliseconds,
            event.previous_schedule.difficulty_millipoints,
            event.previous_schedule.last_reviewed_at_ms,
            event.next_schedule.version,
            event.next_schedule.due_at_ms,
            event.next_schedule.ideal_due_at_ms,
            event.next_schedule.interval_milliseconds,
            event.next_schedule.interval_seconds,
            event.next_schedule.repetitions,
            event.next_schedule.stability_milliseconds,
            event.next_schedule.difficulty_millipoints,
            event.next_schedule.last_reviewed_at_ms,
        ],
    )?;
    Ok(())
}

fn load_review_events(
    connection: &Connection,
    card_id: &str,
) -> Result<Vec<ReviewEvent>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT
            id,
            card_id,
            card_content_version,
            raw_response,
            normalized_response,
            comparison,
            suggested_grade,
            chosen_grade,
            reviewed_at_ms,
            scheduler_version,
            scheduler_parameter_set_id,
            target_retention_basis_points,
            previous_schedule_version,
            previous_due_at_ms,
            previous_ideal_due_at_ms,
            previous_interval_milliseconds,
            previous_interval_seconds,
            previous_repetitions,
            previous_stability_milliseconds,
            previous_difficulty_millipoints,
            previous_last_reviewed_at_ms,
            next_schedule_version,
            next_due_at_ms,
            next_ideal_due_at_ms,
            next_interval_milliseconds,
            next_interval_seconds,
            next_repetitions,
            next_stability_milliseconds,
            next_difficulty_millipoints,
            next_last_reviewed_at_ms
         FROM review_events
         WHERE card_id = ?1
         ORDER BY previous_schedule_version, reviewed_at_ms, id",
    )?;
    let stored = statement
        .query_map([card_id], stored_review_event_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    let baseline = load_schedule_row(connection, "schedule_baselines", card_id)?
        .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
    let mut previous_event_id = baseline.last_review_event_id;
    let mut events = Vec::with_capacity(stored.len());
    for stored in stored {
        let event = stored.into_domain(previous_event_id)?;
        previous_event_id = Some(event.id.clone());
        events.push(event);
    }
    Ok(events)
}

struct StoredReviewEvent {
    id: String,
    card_id: String,
    card_content_version: u64,
    raw_response: String,
    normalized_response: String,
    comparison: String,
    suggested_grade: String,
    chosen_grade: String,
    reviewed_at_ms: i64,
    scheduler_version: String,
    scheduler_parameter_set_id: Option<String>,
    target_retention_basis_points: u16,
    previous_version: u64,
    previous_due_at_ms: i64,
    previous_ideal_due_at_ms: i64,
    previous_interval_milliseconds: u64,
    previous_interval_seconds: u64,
    previous_repetitions: u32,
    previous_stability_milliseconds: u64,
    previous_difficulty_millipoints: u32,
    previous_last_reviewed_at_ms: Option<i64>,
    next_version: u64,
    next_due_at_ms: i64,
    next_ideal_due_at_ms: i64,
    next_interval_milliseconds: u64,
    next_interval_seconds: u64,
    next_repetitions: u32,
    next_stability_milliseconds: u64,
    next_difficulty_millipoints: u32,
    next_last_reviewed_at_ms: Option<i64>,
}

impl StoredReviewEvent {
    fn into_domain(self, previous_event_id: Option<String>) -> Result<ReviewEvent, StorageError> {
        let is_legacy = self.scheduler_version != "fsrs-7";
        let previous_schedule = ScheduleState {
            card_id: self.card_id.clone(),
            version: self.previous_version,
            due_at_ms: self.previous_due_at_ms,
            ideal_due_at_ms: if is_legacy {
                self.previous_due_at_ms
            } else {
                self.previous_ideal_due_at_ms
            },
            interval_milliseconds: if is_legacy {
                self.previous_interval_seconds.saturating_mul(1_000)
            } else {
                self.previous_interval_milliseconds
            },
            interval_seconds: self.previous_interval_seconds,
            repetitions: self.previous_repetitions,
            stability_milliseconds: self.previous_stability_milliseconds,
            difficulty_millipoints: self.previous_difficulty_millipoints,
            last_reviewed_at_ms: self.previous_last_reviewed_at_ms,
            last_review_event_id: previous_event_id,
        };
        let next_schedule = ScheduleState {
            card_id: self.card_id.clone(),
            version: self.next_version,
            due_at_ms: self.next_due_at_ms,
            ideal_due_at_ms: if is_legacy {
                self.next_due_at_ms
            } else {
                self.next_ideal_due_at_ms
            },
            interval_milliseconds: if is_legacy {
                self.next_interval_seconds.saturating_mul(1_000)
            } else {
                self.next_interval_milliseconds
            },
            interval_seconds: self.next_interval_seconds,
            repetitions: self.next_repetitions,
            stability_milliseconds: self.next_stability_milliseconds,
            difficulty_millipoints: self.next_difficulty_millipoints,
            last_reviewed_at_ms: self.next_last_reviewed_at_ms,
            last_review_event_id: Some(self.id.clone()),
        };
        Ok(ReviewEvent {
            id: self.id,
            card_id: self.card_id,
            card_content_version: self.card_content_version,
            raw_response: self.raw_response,
            normalized_response: self.normalized_response,
            comparison: comparison_from_database(&self.comparison)?,
            suggested_grade: grade_from_database(&self.suggested_grade)?,
            chosen_grade: grade_from_database(&self.chosen_grade)?,
            reviewed_at_ms: self.reviewed_at_ms,
            scheduler_version: self.scheduler_version,
            scheduler_parameter_set_id: self.scheduler_parameter_set_id,
            target_retention_basis_points: self.target_retention_basis_points,
            previous_schedule,
            next_schedule,
        })
    }
}

fn stored_review_event_from_row(row: &Row<'_>) -> rusqlite::Result<StoredReviewEvent> {
    Ok(StoredReviewEvent {
        id: row.get(0)?,
        card_id: row.get(1)?,
        card_content_version: row.get(2)?,
        raw_response: row.get(3)?,
        normalized_response: row.get(4)?,
        comparison: row.get(5)?,
        suggested_grade: row.get(6)?,
        chosen_grade: row.get(7)?,
        reviewed_at_ms: row.get(8)?,
        scheduler_version: row.get(9)?,
        scheduler_parameter_set_id: row.get(10)?,
        target_retention_basis_points: row.get(11)?,
        previous_version: row.get(12)?,
        previous_due_at_ms: row.get(13)?,
        previous_ideal_due_at_ms: row.get(14)?,
        previous_interval_milliseconds: row.get(15)?,
        previous_interval_seconds: row.get(16)?,
        previous_repetitions: row.get(17)?,
        previous_stability_milliseconds: row.get(18)?,
        previous_difficulty_millipoints: row.get(19)?,
        previous_last_reviewed_at_ms: row.get(20)?,
        next_version: row.get(21)?,
        next_due_at_ms: row.get(22)?,
        next_ideal_due_at_ms: row.get(23)?,
        next_interval_milliseconds: row.get(24)?,
        next_interval_seconds: row.get(25)?,
        next_repetitions: row.get(26)?,
        next_stability_milliseconds: row.get(27)?,
        next_difficulty_millipoints: row.get(28)?,
        next_last_reviewed_at_ms: row.get(29)?,
    })
}

const fn comparison_to_database(value: ComparisonResult) -> &'static str {
    match value {
        ComparisonResult::Exact => "exact",
        ComparisonResult::AcceptedVariant => "accepted_variant",
        ComparisonResult::NearMatch => "near_match",
        ComparisonResult::Incorrect => "incorrect",
        ComparisonResult::Empty => "empty",
    }
}

fn comparison_from_database(value: &str) -> Result<ComparisonResult, StorageError> {
    match value {
        "exact" => Ok(ComparisonResult::Exact),
        "accepted_variant" => Ok(ComparisonResult::AcceptedVariant),
        "near_match" => Ok(ComparisonResult::NearMatch),
        "incorrect" => Ok(ComparisonResult::Incorrect),
        "empty" => Ok(ComparisonResult::Empty),
        _ => Err(StorageError::InvalidStoredValue {
            field: "comparison",
            value: value.to_owned(),
        }),
    }
}

const fn grade_to_database(value: Grade) -> &'static str {
    match value {
        Grade::Again => "again",
        Grade::Hard => "hard",
        Grade::Good => "good",
        Grade::Easy => "easy",
    }
}

fn grade_from_database(value: &str) -> Result<Grade, StorageError> {
    match value {
        "again" => Ok(Grade::Again),
        "hard" => Ok(Grade::Hard),
        "good" => Ok(Grade::Good),
        "easy" => Ok(Grade::Easy),
        _ => Err(StorageError::InvalidStoredValue {
            field: "grade",
            value: value.to_owned(),
        }),
    }
}
