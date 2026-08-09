use std::collections::HashMap;

use crate::{ApplicationError, ApplicationService, DirectionDto, MatchingPolicyDto, desktop_u32};
use meiki_domain::{Deck, Direction, MatchingPolicy, StudySettingsOverride};
use meiki_storage::{
    DEFAULT_DECK_ID, DeckRepository, MediaRepository, SchedulerProfileRepository, Storage,
};
use meiki_text::normalize_for_search;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeckDto {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub note_count: u32,
    pub daily_time_budget_override_minutes: Option<u32>,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
    pub matching_policy: MatchingPolicyDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeckSummaryDto {
    pub id: String,
    pub name: String,
    pub total_cards: u32,
    pub due_cards: u32,
    pub new_cards: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct CreateDeckRequest {
    pub name: String,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct RenameDeckRequest {
    pub deck_id: String,
    pub name: String,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeleteDeckRequest {
    pub deck_id: String,
    pub move_cards_to_deck_id: Option<String>,
    pub confirmation: String,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeleteDeckResultDto {
    pub deleted_deck_id: String,
    pub affected_cards: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundleRemovalPreviewDto {
    pub language_tag: String,
    #[ts(type = "number")]
    pub decks: u64,
    #[ts(type = "number")]
    pub cards: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundleRemovalRequest {
    pub language_tag: String,
    #[ts(type = "number")]
    pub expected_decks: u64,
    #[ts(type = "number")]
    pub expected_cards: u64,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundleRemovalProgressDto {
    #[ts(type = "number")]
    pub removed_decks: u64,
    #[ts(type = "number")]
    pub total_decks: u64,
    #[ts(type = "number")]
    pub processed_cards: u64,
    #[ts(type = "number")]
    pub total_cards: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundleRemovalResultDto {
    pub language_tag: String,
    #[ts(type = "number")]
    pub removed_decks: u64,
    #[ts(type = "number")]
    pub affected_cards: u64,
}

impl ApplicationService {
    /// Lists installed bundles with the remaining decks and active cards that
    /// a bundle removal would affect.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted associations or card counts cannot be
    /// loaded.
    pub fn list_installed_bundles(&self) -> Result<Vec<BundleRemovalPreviewDto>, ApplicationError> {
        Ok(self
            .open_storage()?
            .installed_bundles()?
            .into_iter()
            .map(|bundle| BundleRemovalPreviewDto {
                language_tag: bundle.language_tag,
                decks: bundle.deck_count,
                cards: bundle.active_card_count,
            })
            .collect())
    }

    /// Removes all remaining decks in one installed bundle atomically.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, stale confirmation counts, missing
    /// bundle associations, or a failed durable write.
    pub fn remove_bundle(
        &self,
        request: &BundleRemovalRequest,
        mut on_progress: impl FnMut(BundleRemovalProgressDto),
    ) -> Result<BundleRemovalResultDto, ApplicationError> {
        if request.language_tag.trim().is_empty()
            || request.expected_decks == 0
            || request.now_ms < 0
        {
            return Err(ApplicationError::InvalidDeck(
                "bundle removal requires a language, at least one deck, and a valid timestamp"
                    .into(),
            ));
        }
        let mut storage = self.open_storage()?;
        self.create_recovery_backup(&storage, "pre-remove-bundle")?;
        let removed = storage.remove_bundle(
            &request.language_tag,
            request.expected_decks,
            request.expected_cards,
            request.now_ms,
            |removed_decks, processed_cards| {
                on_progress(BundleRemovalProgressDto {
                    removed_decks,
                    total_decks: request.expected_decks,
                    processed_cards,
                    total_cards: request.expected_cards,
                });
            },
        )?;
        self.remove_orphaned_media(&storage, &removed.orphaned_media_hashes)?;
        Ok(BundleRemovalResultDto {
            language_tag: removed.language_tag,
            removed_decks: removed.deck_count,
            affected_cards: removed.active_card_count,
        })
    }

    /// Lists the collection's flat decks with their local note counts.
    ///
    /// # Errors
    ///
    /// Returns an error when deck or scheduling metadata cannot be loaded.
    pub fn list_decks(&self) -> Result<Vec<DeckDto>, ApplicationError> {
        let storage = self.open_storage()?;
        storage
            .list_decks()?
            .into_iter()
            .map(|deck| deck_dto(&storage, deck))
            .collect()
    }

    /// Lists user-visible flat decks with current card counts.
    ///
    /// # Errors
    ///
    /// Returns an error when deck metadata or aggregate card state cannot be loaded.
    pub fn list_deck_summaries(
        &self,
        now_ms: i64,
    ) -> Result<Vec<DeckSummaryDto>, ApplicationError> {
        let storage = self.open_storage()?;
        let counts = storage
            .deck_card_counts(now_ms)?
            .into_iter()
            .map(|counts| (counts.deck_id.clone(), counts))
            .collect::<HashMap<_, _>>();
        let mut summaries = Vec::new();
        for deck in storage.list_decks()? {
            let counts = counts.get(&deck.id).ok_or_else(|| {
                ApplicationError::InvalidDeck("deck card counts are incomplete".into())
            })?;
            if deck.id == DEFAULT_DECK_ID && counts.all_cards == 0 {
                continue;
            }
            summaries.push(DeckSummaryDto {
                id: deck.id.clone(),
                name: if deck.id == DEFAULT_DECK_ID {
                    "Unsorted".into()
                } else {
                    deck.name
                },
                total_cards: desktop_u32(counts.total_cards, "deck card count")?,
                due_cards: desktop_u32(counts.due_cards, "deck due card count")?,
                new_cards: desktop_u32(counts.new_cards, "deck new card count")?,
            });
        }
        Ok(summaries)
    }

    /// Creates one flat deck with inherited collection scheduling defaults.
    ///
    /// # Errors
    ///
    /// Returns an error when the name is unsafe, already used, or persistence
    /// fails.
    pub fn create_deck(&self, request: &CreateDeckRequest) -> Result<DeckDto, ApplicationError> {
        let name = validated_name(&request.name)?;
        let mut storage = self.open_storage()?;
        ensure_unique_name(&storage, name, None)?;
        let deck = Deck {
            id: self.next_id("deck"),
            name: name.to_owned(),
            description: None,
            language_tag: None,
            direction: Direction::Auto,
            matching_policy: MatchingPolicy::Strict,
            settings: StudySettingsOverride::default(),
            created_at_ms: request.now_ms,
            updated_at_ms: request.now_ms,
        };
        storage.create_deck(&deck)?;
        deck_dto(&storage, deck)
    }

    /// Renames one flat deck without changing its notes or schedule.
    ///
    /// # Errors
    ///
    /// Returns an error when the deck is missing, the name is unsafe or
    /// already used, or persistence fails.
    pub fn rename_deck(&self, request: &RenameDeckRequest) -> Result<DeckDto, ApplicationError> {
        if request.deck_id == DEFAULT_DECK_ID {
            return Err(ApplicationError::InvalidDeck(
                "the default deck cannot be renamed".into(),
            ));
        }
        let name = validated_name(&request.name)?;
        let mut storage = self.open_storage()?;
        ensure_unique_name(&storage, name, Some(&request.deck_id))?;
        let mut deck = storage.get_deck(&request.deck_id)?;
        name.clone_into(&mut deck.name);
        deck.updated_at_ms = request.now_ms;
        storage.update_deck(&deck)?;
        deck_dto(&storage, deck)
    }

    /// Deletes an explicit non-default deck, moving active cards to Trash or
    /// into the selected destination atomically.
    ///
    /// # Errors
    ///
    /// Returns an error unless the exact deck name confirms the action and an
    /// optional destination differs from the deleted deck.
    pub fn delete_deck(
        &self,
        request: &DeleteDeckRequest,
    ) -> Result<DeleteDeckResultDto, ApplicationError> {
        if request.deck_id == DEFAULT_DECK_ID {
            return Err(ApplicationError::InvalidDeck(
                "the default deck cannot be deleted".into(),
            ));
        }
        let mut storage = self.open_storage()?;
        if storage.list_decks()?.len() <= 1 {
            return Err(ApplicationError::InvalidDeck(
                "the collection must keep at least one deck".into(),
            ));
        }
        let deck = storage.get_deck(&request.deck_id)?;
        if request.confirmation != deck.name {
            return Err(ApplicationError::InvalidDeck(format!(
                "type the exact deck name {:?} to confirm deletion",
                deck.name
            )));
        }
        if storage
            .bundle_language_for_deck(&request.deck_id)?
            .is_some()
        {
            self.create_recovery_backup(&storage, "pre-delete-bundle-stage")?;
        }
        let deletion = storage.delete_deck_and_rehome_notes(
            &request.deck_id,
            request.move_cards_to_deck_id.as_deref(),
            request.now_ms,
        )?;
        self.remove_orphaned_media(&storage, &deletion.orphaned_media_hashes)?;
        Ok(DeleteDeckResultDto {
            deleted_deck_id: request.deck_id.clone(),
            affected_cards: desktop_u32(deletion.active_card_count, "affected deck card count")?,
        })
    }

    fn remove_orphaned_media(
        &self,
        storage: &Storage,
        content_hashes: &[String],
    ) -> Result<(), ApplicationError> {
        for content_hash in content_hashes {
            if storage.media_reference_count_for_hash(content_hash)? != 0 {
                continue;
            }
            match self.media_store().remove(content_hash) {
                Ok(()) | Err(meiki_media::MediaError::MissingObject(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

fn deck_dto(storage: &Storage, deck: Deck) -> Result<DeckDto, ApplicationError> {
    let profile = storage.get_scheduler_profile(&deck.id)?;
    Ok(DeckDto {
        note_count: desktop_u32(storage.deck_note_count(&deck.id)?, "deck note count")?,
        daily_time_budget_override_minutes: profile.deck_daily_time_budget_minutes,
        is_default: deck.id == DEFAULT_DECK_ID,
        id: deck.id,
        name: deck.name,
        language_tag: deck.language_tag,
        direction: deck.direction.into(),
        matching_policy: deck.matching_policy.into(),
    })
}

fn validated_name(value: &str) -> Result<&str, ApplicationError> {
    let name = value.trim();
    if name.is_empty()
        || name.chars().count() > 80
        || name.chars().any(char::is_control)
        || matches!(name, "." | "..")
    {
        return Err(ApplicationError::InvalidDeck(
            "deck names must contain 1–80 visible characters".into(),
        ));
    }
    Ok(name)
}

fn ensure_unique_name(
    storage: &Storage,
    name: &str,
    except_id: Option<&str>,
) -> Result<(), ApplicationError> {
    let normalized = normalize_for_search(name);
    if storage.list_decks()?.into_iter().any(|deck| {
        Some(deck.id.as_str()) != except_id && normalize_for_search(&deck.name) == normalized
    }) {
        return Err(ApplicationError::InvalidDeck(
            "another deck already uses that name".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use meiki_domain::{
        Card, CardLifecycle, Cloze, Direction, MatchingPolicy, ReviewEvent, ScheduleState,
        SegmentContent, SemanticSegment, SourceItem,
    };
    use meiki_storage::{CardRepository, SourceNoteRepository, Storage, StoredSourceNote};
    use tempfile::tempdir;

    use super::{
        BundleRemovalProgressDto, BundleRemovalRequest, CreateDeckRequest, DeckSummaryDto,
        DeleteDeckRequest, RenameDeckRequest,
    };
    use crate::{
        ApplicationService, DeckCardActionDto, DeckCardActionRequest, DeckCardRequest,
        DeckCardTrashDto, GradeDto, GradeReviewRequest,
    };

    fn add_card(
        storage: &mut Storage,
        deck_id: &str,
        id: &str,
        lifecycle: CardLifecycle,
        due_at_ms: i64,
        suspended: bool,
    ) {
        let source_id = format!("{id}-source");
        let cloze_id = format!("{id}-cloze");
        storage
            .create_source_note(&StoredSourceNote {
                source_item: SourceItem {
                    id: source_id.clone(),
                    deck_id: deck_id.into(),
                    segments: vec![SemanticSegment {
                        id: format!("{id}-segment"),
                        ordinal: 0,
                        content: SegmentContent::Cloze {
                            cloze_id: cloze_id.clone(),
                            text: id.into(),
                        },
                    }],
                    language_tag: None,
                    direction: Direction::Auto,
                    tags: Vec::new(),
                    annotations: Vec::new(),
                    explanation: None,
                    media: Vec::new(),
                    created_at_ms: 1_000,
                    updated_at_ms: 1_000,
                },
                clozes: vec![Cloze {
                    id: cloze_id.clone(),
                    source_item_id: source_id,
                    answer: id.into(),
                    accepted_answers: Vec::new(),
                    hint: None,
                    language_tag: None,
                    direction: Direction::Auto,
                    matching_policy: Some(MatchingPolicy::Strict),
                    annotations: Vec::new(),
                    explanation: None,
                    media: Vec::new(),
                    created_at_ms: 1_000,
                    updated_at_ms: 1_000,
                }],
            })
            .unwrap();
        let introduced = lifecycle == CardLifecycle::Introduced;
        storage
            .create_card(
                &Card {
                    id: id.into(),
                    cloze_id,
                    content_version: 0,
                    suspended,
                    created_at_ms: 1_000,
                    updated_at_ms: 1_000,
                },
                &ScheduleState {
                    card_id: id.into(),
                    version: 0,
                    lifecycle,
                    due_at_ms,
                    ideal_due_at_ms: due_at_ms,
                    interval_milliseconds: if introduced { 86_400_000 } else { 0 },
                    interval_seconds: if introduced { 86_400 } else { 0 },
                    repetitions: u32::from(introduced),
                    stability_milliseconds: if introduced { 86_400_000 } else { 0 },
                    difficulty_millipoints: if introduced { 5_000 } else { 0 },
                    last_reviewed_at_ms: introduced.then_some(1_000),
                    last_review_event_id: None,
                },
            )
            .unwrap();
    }

    fn review_card(
        service: &ApplicationService,
        card_id: &str,
        review_event_id: &str,
        reviewed_at_ms: i64,
    ) -> (ScheduleState, Vec<ReviewEvent>) {
        let card = service.get_study_card(card_id).unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: review_event_id.into(),
                    card_id: card.card_id,
                    card_content_version: card.card_content_version,
                    schedule_version: card.schedule_version,
                    raw_response: card_id.into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 900,
                },
                reviewed_at_ms,
            )
            .unwrap();
        let storage = service.open_storage().unwrap();
        (
            storage.load_schedule(card_id).unwrap(),
            storage.review_events(card_id).unwrap(),
        )
    }

    fn deck_summary<'a>(summaries: &'a [DeckSummaryDto], deck_id: &str) -> &'a DeckSummaryDto {
        summaries.iter().find(|deck| deck.id == deck_id).unwrap()
    }

    #[test]
    fn rename_preserves_the_deck_cards_review_history_and_schedule() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        let created = service
            .create_deck(&CreateDeckRequest {
                name: " Listening ".into(),
                now_ms: 1_000,
            })
            .unwrap();
        assert_eq!(created.name, "Listening");

        let mut storage = service.open_storage().unwrap();
        add_card(
            &mut storage,
            &created.id,
            "listening-card",
            CardLifecycle::Unseen,
            1_000,
            false,
        );
        drop(storage);
        let (schedule_before, history_before) =
            review_card(&service, "listening-card", "listening-review", 2_000);

        let renamed = service
            .rename_deck(&RenameDeckRequest {
                deck_id: created.id.clone(),
                name: "Audio".into(),
                now_ms: 3_000,
            })
            .unwrap();
        assert_eq!(renamed.name, "Audio");
        let storage = service.open_storage().unwrap();
        assert_eq!(
            storage
                .get_source_note("listening-card-source")
                .unwrap()
                .source_item
                .deck_id,
            created.id
        );
        assert_eq!(
            storage.load_schedule("listening-card").unwrap(),
            schedule_before
        );
        assert_eq!(
            storage.review_events("listening-card").unwrap(),
            history_before
        );
    }

    #[test]
    fn direct_deletion_moves_all_remaining_cards_to_trash() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        let created = service
            .create_deck(&CreateDeckRequest {
                name: "Temporary".into(),
                now_ms: 1_000,
            })
            .unwrap();
        let mut storage = service.open_storage().unwrap();
        add_card(
            &mut storage,
            &created.id,
            "active-card",
            CardLifecycle::Unseen,
            1_000,
            false,
        );
        add_card(
            &mut storage,
            &created.id,
            "already-trashed-card",
            CardLifecycle::Introduced,
            1_000,
            false,
        );
        storage
            .set_deck_cards_deleted(&["already-trashed-card".into()], Some(1_500), 1_500)
            .unwrap();
        drop(storage);
        let (schedule_before, history_before) =
            review_card(&service, "active-card", "active-card-review", 1_750);

        let deleted = service
            .delete_deck(&DeleteDeckRequest {
                deck_id: created.id.clone(),
                move_cards_to_deck_id: None,
                confirmation: "Temporary".into(),
                now_ms: 2_000,
            })
            .unwrap();
        assert_eq!(deleted.affected_cards, 1);
        let storage = service.open_storage().unwrap();
        let notes = storage.library_notes().unwrap();
        for source_id in ["active-card-source", "already-trashed-card-source"] {
            let note = notes
                .iter()
                .find(|note| note.note.source_item.id == source_id)
                .unwrap();
            assert_eq!(
                note.note.source_item.deck_id,
                meiki_storage::DEFAULT_DECK_ID
            );
            assert!(note.deleted_at_ms.is_some());
        }
        assert_eq!(
            storage.load_schedule("active-card").unwrap(),
            schedule_before
        );
        assert_eq!(
            storage.review_events("active-card").unwrap(),
            history_before
        );
        drop(storage);

        let summaries = service.list_deck_summaries(2_000).unwrap();
        let unsorted = deck_summary(&summaries, meiki_storage::DEFAULT_DECK_ID);
        assert_eq!(unsorted.total_cards, 0);
        let trash = service
            .get_deck_cards(&DeckCardRequest {
                deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
                query: String::new(),
                trash: DeckCardTrashDto::Trash,
                now_ms: 2_000,
                offset: 0,
                limit: 25,
            })
            .unwrap();
        assert!(trash.cards.iter().any(|card| card.id == "active-card"));
        service
            .apply_deck_card_action(&DeckCardActionRequest {
                deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
                card_ids: vec!["active-card".into()],
                action: DeckCardActionDto::Restore,
                destination_deck_id: None,
                now_ms: 2_001,
            })
            .unwrap();
        let summaries = service.list_deck_summaries(2_001).unwrap();
        let unsorted = deck_summary(&summaries, meiki_storage::DEFAULT_DECK_ID);
        assert_eq!(unsorted.total_cards, 1);
    }

    #[test]
    fn secondary_move_deletes_the_deck_and_preserves_active_cards() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        let source = service
            .create_deck(&CreateDeckRequest {
                name: "Source".into(),
                now_ms: 1_000,
            })
            .unwrap();
        let destination = service
            .create_deck(&CreateDeckRequest {
                name: "Destination".into(),
                now_ms: 1_000,
            })
            .unwrap();
        let mut storage = service.open_storage().unwrap();
        add_card(
            &mut storage,
            &source.id,
            "moved-card",
            CardLifecycle::Unseen,
            1_000,
            false,
        );
        drop(storage);

        let deleted = service
            .delete_deck(&DeleteDeckRequest {
                deck_id: source.id,
                move_cards_to_deck_id: Some(destination.id.clone()),
                confirmation: "Source".into(),
                now_ms: 2_000,
            })
            .unwrap();
        assert_eq!(deleted.affected_cards, 1);
        let note = service
            .open_storage()
            .unwrap()
            .library_notes()
            .unwrap()
            .into_iter()
            .find(|note| note.note.source_item.id == "moved-card-source")
            .unwrap();
        assert_eq!(note.note.source_item.deck_id, destination.id);
        assert_eq!(note.deleted_at_ms, None);
    }

    #[test]
    fn empty_deck_deletion_is_explicit_and_keeps_the_default_deck() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        let empty = service
            .create_deck(&CreateDeckRequest {
                name: "Temporary".into(),
                now_ms: 1_000,
            })
            .unwrap();

        assert!(
            service
                .delete_deck(&DeleteDeckRequest {
                    deck_id: empty.id.clone(),
                    move_cards_to_deck_id: None,
                    confirmation: "Wrong name".into(),
                    now_ms: 2_000,
                })
                .is_err()
        );
        let deleted = service
            .delete_deck(&DeleteDeckRequest {
                deck_id: empty.id,
                move_cards_to_deck_id: None,
                confirmation: "Temporary".into(),
                now_ms: 2_000,
            })
            .unwrap();
        assert_eq!(deleted.affected_cards, 0);
        assert_eq!(service.list_decks().unwrap().len(), 1);
        assert!(service.list_decks().unwrap()[0].is_default);
        assert!(
            service
                .delete_deck(&DeleteDeckRequest {
                    deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
                    move_cards_to_deck_id: None,
                    confirmation: "Default".into(),
                    now_ms: 3_000,
                })
                .is_err()
        );
        assert!(
            service
                .rename_deck(&RenameDeckRequest {
                    deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
                    name: "Renamed".into(),
                    now_ms: 3_000,
                })
                .is_err()
        );
    }

    #[test]
    fn deck_summaries_count_suspended_cards_only_in_total_and_present_default_as_unsorted() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        let created = service
            .create_deck(&CreateDeckRequest {
                name: "Japanese".into(),
                now_ms: 1_000,
            })
            .unwrap();
        let mut storage = service.open_storage().unwrap();
        add_card(
            &mut storage,
            &created.id,
            "new-card",
            CardLifecycle::Unseen,
            10_000,
            false,
        );
        add_card(
            &mut storage,
            &created.id,
            "due-card",
            CardLifecycle::Introduced,
            9_000,
            false,
        );
        add_card(
            &mut storage,
            &created.id,
            "scheduled-card",
            CardLifecycle::Introduced,
            11_000,
            false,
        );
        add_card(
            &mut storage,
            &created.id,
            "suspended-card",
            CardLifecycle::Unseen,
            10_000,
            true,
        );
        add_card(
            &mut storage,
            &created.id,
            "trashed-card",
            CardLifecycle::Introduced,
            9_000,
            false,
        );
        storage
            .set_deck_cards_deleted(&["trashed-card".into()], Some(9_000), 9_000)
            .unwrap();
        drop(storage);

        let summaries = service.list_deck_summaries(10_000).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "Japanese");
        assert_eq!(summaries[0].total_cards, 4);
        assert_eq!(summaries[0].due_cards, 1);
        assert_eq!(summaries[0].new_cards, 1);

        let mut storage = service.open_storage().unwrap();
        add_card(
            &mut storage,
            meiki_storage::DEFAULT_DECK_ID,
            "trashed-unsorted-card",
            CardLifecycle::Unseen,
            10_000,
            false,
        );
        storage
            .set_deck_cards_deleted(&["trashed-unsorted-card".into()], Some(10_000), 10_000)
            .unwrap();
        drop(storage);
        let summaries = service.list_deck_summaries(10_000).unwrap();
        let unsorted = deck_summary(&summaries, meiki_storage::DEFAULT_DECK_ID);
        assert_eq!(unsorted.total_cards, 0);
        assert_eq!(unsorted.due_cards, 0);
        assert_eq!(unsorted.new_cards, 0);

        let mut storage = service.open_storage().unwrap();
        add_card(
            &mut storage,
            meiki_storage::DEFAULT_DECK_ID,
            "unsorted-card",
            CardLifecycle::Unseen,
            10_000,
            true,
        );
        drop(storage);
        let summaries = service.list_deck_summaries(10_000).unwrap();
        let unsorted = deck_summary(&summaries, meiki_storage::DEFAULT_DECK_ID);
        assert_eq!(unsorted.name, "Unsorted");
        assert_eq!(unsorted.total_cards, 1);
        assert_eq!(unsorted.due_cards, 0);
        assert_eq!(unsorted.new_cards, 0);
    }

    #[test]
    #[ignore = "release performance budget; run with scripts/performance"]
    fn release_budget_bundle_removal_9700_cards_ignores_unrelated_content() {
        const NOW_MS: i64 = 1_000_000_000;
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("bundle-removal.db");
        let mut storage = Storage::open(&collection_path).unwrap();
        storage.seed_bundle_removal_release_fixture(NOW_MS).unwrap();
        let unrelated_note = storage
            .get_source_note("performance-source-0000000")
            .unwrap();
        let unrelated_schedule = storage.load_schedule("performance-card-0000000").unwrap();
        let unrelated_history = storage.review_events("performance-card-0000000").unwrap();
        drop(storage);

        let service = ApplicationService::new(&collection_path);
        let preview = service.list_installed_bundles().unwrap().remove(0);
        assert_eq!((preview.decks, preview.cards), (6, 9_700));
        let mut progress = Vec::<BundleRemovalProgressDto>::new();
        let started = std::time::Instant::now();
        let removed = service
            .remove_bundle(
                &BundleRemovalRequest {
                    language_tag: preview.language_tag,
                    expected_decks: preview.decks,
                    expected_cards: preview.cards,
                    now_ms: NOW_MS + 1,
                },
                |update| progress.push(update),
            )
            .unwrap();
        let elapsed = started.elapsed();

        assert_eq!((removed.removed_decks, removed.affected_cards), (6, 9_700));
        assert_eq!(progress.len(), 6);
        assert_eq!(
            progress.last(),
            Some(&BundleRemovalProgressDto {
                removed_decks: 6,
                total_decks: 6,
                processed_cards: 9_700,
                total_cards: 9_700,
            })
        );
        assert!(progress.windows(2).all(|updates| {
            updates[0].removed_decks <= updates[1].removed_decks
                && updates[0].processed_cards <= updates[1].processed_cards
        }));
        assert!(
            elapsed <= std::time::Duration::from_secs(60),
            "9,700-card bundle removal exceeded 60 s: {elapsed:?}"
        );

        let storage = Storage::open(&collection_path).unwrap();
        assert!(storage.installed_bundles().unwrap().is_empty());
        assert_eq!(
            storage
                .get_source_note("performance-source-0000000")
                .unwrap(),
            unrelated_note
        );
        assert_eq!(
            storage.load_schedule("performance-card-0000000").unwrap(),
            unrelated_schedule
        );
        assert_eq!(
            storage.review_events("performance-card-0000000").unwrap(),
            unrelated_history
        );
        drop(storage);
        let unsorted = service
            .list_deck_summaries(NOW_MS + 1)
            .unwrap()
            .into_iter()
            .find(|deck| deck.id == meiki_storage::DEFAULT_DECK_ID)
            .unwrap();
        assert_eq!(unsorted.total_cards, 9_700);
        eprintln!(
            "release-budget bundle_removal_9700 elapsed_ms={}",
            elapsed.as_millis()
        );
    }
}
