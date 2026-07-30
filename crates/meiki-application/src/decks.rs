use crate::{ApplicationError, ApplicationService, DirectionDto, MatchingPolicyDto, desktop_u32};
use meiki_domain::{Deck, Direction, MatchingPolicy, StudySettingsOverride};
use meiki_storage::{DEFAULT_DECK_ID, DeckRepository, SchedulerProfileRepository, Storage};
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
    pub move_notes_to_deck_id: Option<String>,
    pub confirmation: String,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeleteDeckResultDto {
    pub deleted_deck_id: String,
    pub moved_notes: u32,
}

impl ApplicationService {
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
        let name = validated_name(&request.name)?;
        let mut storage = self.open_storage()?;
        ensure_unique_name(&storage, name, Some(&request.deck_id))?;
        let mut deck = storage.get_deck(&request.deck_id)?;
        name.clone_into(&mut deck.name);
        deck.updated_at_ms = request.now_ms;
        storage.update_deck(&deck)?;
        deck_dto(&storage, deck)
    }

    /// Deletes an explicit non-default deck, moving all notes atomically when
    /// it is not empty.
    ///
    /// # Errors
    ///
    /// Returns an error unless the exact deck name confirms the action and a
    /// non-empty deck has a distinct destination.
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
        let moved = storage.delete_deck_and_move_notes(
            &request.deck_id,
            request.move_notes_to_deck_id.as_deref(),
            request.now_ms,
        )?;
        Ok(DeleteDeckResultDto {
            deleted_deck_id: request.deck_id.clone(),
            moved_notes: desktop_u32(moved, "moved deck note count")?,
        })
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
        Card, CardLifecycle, Cloze, Direction, MatchingPolicy, ScheduleState, SegmentContent,
        SemanticSegment, SourceItem,
    };
    use meiki_storage::{CardRepository, SourceNoteRepository, StoredSourceNote};
    use tempfile::tempdir;

    use super::{CreateDeckRequest, DeleteDeckRequest, RenameDeckRequest};
    use crate::ApplicationService;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn flat_decks_create_rename_and_delete_with_an_explicit_note_move() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        let created = service
            .create_deck(&CreateDeckRequest {
                name: " Listening ".into(),
                now_ms: 1_000,
            })
            .unwrap();
        assert_eq!(created.name, "Listening");
        let renamed = service
            .rename_deck(&RenameDeckRequest {
                deck_id: created.id.clone(),
                name: "Audio".into(),
                now_ms: 2_000,
            })
            .unwrap();
        assert_eq!(renamed.name, "Audio");

        let mut storage = service.open_storage().unwrap();
        let note = StoredSourceNote {
            source_item: SourceItem {
                id: "deck-note".into(),
                deck_id: created.id.clone(),
                segments: vec![SemanticSegment {
                    id: "deck-segment".into(),
                    ordinal: 0,
                    content: SegmentContent::Cloze {
                        cloze_id: "deck-cloze".into(),
                        text: "listen".into(),
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
                id: "deck-cloze".into(),
                source_item_id: "deck-note".into(),
                answer: "listen".into(),
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
        };
        storage.create_source_note(&note).unwrap();
        storage
            .create_card(
                &Card {
                    id: "deck-card".into(),
                    cloze_id: "deck-cloze".into(),
                    content_version: 0,
                    suspended: false,
                    created_at_ms: 1_000,
                    updated_at_ms: 1_000,
                },
                &ScheduleState {
                    card_id: "deck-card".into(),
                    version: 0,
                    lifecycle: CardLifecycle::Unseen,
                    due_at_ms: 1_000,
                    ideal_due_at_ms: 1_000,
                    interval_milliseconds: 0,
                    interval_seconds: 0,
                    repetitions: 0,
                    stability_milliseconds: 0,
                    difficulty_millipoints: 0,
                    last_reviewed_at_ms: None,
                    last_review_event_id: None,
                },
            )
            .unwrap();
        drop(storage);

        assert!(
            service
                .delete_deck(&DeleteDeckRequest {
                    deck_id: created.id.clone(),
                    move_notes_to_deck_id: None,
                    confirmation: "Audio".into(),
                    now_ms: 3_000,
                })
                .is_err()
        );
        let default = service
            .list_decks()
            .unwrap()
            .into_iter()
            .find(|deck| deck.is_default)
            .unwrap();
        let deleted = service
            .delete_deck(&DeleteDeckRequest {
                deck_id: created.id,
                move_notes_to_deck_id: Some(default.id.clone()),
                confirmation: "Audio".into(),
                now_ms: 3_000,
            })
            .unwrap();
        assert_eq!(deleted.moved_notes, 1);
        assert_eq!(
            service
                .open_storage()
                .unwrap()
                .get_source_note("deck-note")
                .unwrap()
                .source_item
                .deck_id,
            default.id
        );
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
                    move_notes_to_deck_id: None,
                    confirmation: "Wrong name".into(),
                    now_ms: 2_000,
                })
                .is_err()
        );
        let deleted = service
            .delete_deck(&DeleteDeckRequest {
                deck_id: empty.id,
                move_notes_to_deck_id: None,
                confirmation: "Temporary".into(),
                now_ms: 2_000,
            })
            .unwrap();
        assert_eq!(deleted.moved_notes, 0);
        assert_eq!(service.list_decks().unwrap().len(), 1);
        assert!(service.list_decks().unwrap()[0].is_default);
        assert!(
            service
                .delete_deck(&DeleteDeckRequest {
                    deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
                    move_notes_to_deck_id: None,
                    confirmation: "Default".into(),
                    now_ms: 3_000,
                })
                .is_err()
        );
    }
}
