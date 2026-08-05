use std::collections::HashSet;

use meiki_domain::{CardLifecycle, Deck, SegmentContent, SemanticSegment, SourceItem};
use meiki_storage::{
    DEFAULT_DECK_ID, DeckRepository, SourceNoteRepository, Storage, StoredSourceNote,
};
use meiki_text::normalize_for_search;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ApplicationError, ApplicationService, DirectionDto, desktop_u32};

const DEFAULT_PAGE_SIZE: usize = 25;
const MAX_PAGE_SIZE: u32 = 100;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum DeckCardTrashDto {
    Active,
    Trash,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum DeckCardStatusDto {
    New,
    Due,
    Scheduled,
    Suspended,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeckCardRequest {
    pub deck_id: String,
    pub query: String,
    pub trash: DeckCardTrashDto,
    #[ts(type = "number")]
    pub now_ms: i64,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeckCardDto {
    pub id: String,
    pub sentence: String,
    pub answer: String,
    pub status: DeckCardStatusDto,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeckCardDeckDto {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeckCardOverviewDto {
    pub cards: Vec<DeckCardDto>,
    pub decks: Vec<DeckCardDeckDto>,
    pub total_matches: u32,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum DeckCardActionDto {
    Move,
    Suspend,
    Unsuspend,
    Trash,
    Restore,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeckCardActionRequest {
    pub deck_id: String,
    pub card_ids: Vec<String>,
    pub action: DeckCardActionDto,
    pub destination_deck_id: Option<String>,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct DeckCardActionResultDto {
    pub affected_cards: u32,
}

impl ApplicationService {
    /// Returns one deterministic page of cards from one flat deck.
    ///
    /// # Errors
    ///
    /// Returns an error when the deck, page bounds, or stored card data is invalid.
    pub fn get_deck_cards(
        &self,
        request: &DeckCardRequest,
    ) -> Result<DeckCardOverviewDto, ApplicationError> {
        validate_request(request)?;
        let storage = self.open_storage()?;
        let deck = storage.get_deck(&request.deck_id)?;
        let normalized_query = normalize_for_search(&request.query);
        let mut cards = Vec::new();
        for stored in storage.deck_library_notes(&request.deck_id)? {
            let is_trashed = stored.deleted_at_ms.is_some();
            if is_trashed != (request.trash == DeckCardTrashDto::Trash) {
                continue;
            }
            for (cloze, stored_card) in stored.note.clozes.iter().zip(&stored.cards) {
                let sentence = card_sentence(&stored.note.source_item, &cloze.id);
                if !card_matches(
                    &stored.note.source_item,
                    cloze,
                    &sentence,
                    &normalized_query,
                ) {
                    continue;
                }
                cards.push(DeckCardDto {
                    id: stored_card.card.id.clone(),
                    sentence,
                    answer: cloze.answer.clone(),
                    status: card_status(
                        stored_card.card.suspended,
                        stored_card.schedule.lifecycle,
                        stored_card.schedule.due_at_ms,
                        request.now_ms,
                    ),
                    language_tag: cloze
                        .language_tag
                        .clone()
                        .or_else(|| stored.note.source_item.language_tag.clone())
                        .or_else(|| deck.language_tag.clone()),
                    direction: resolved_direction(cloze.direction, &stored.note.source_item, &deck),
                });
            }
        }
        let total_matches = desktop_u32(cards.len() as u64, "deck card match count")?;
        let offset = usize::try_from(request.offset).unwrap_or(usize::MAX);
        let limit = page_size(request.limit);
        let cards = cards.into_iter().skip(offset).take(limit).collect();
        let decks = storage
            .list_decks()?
            .into_iter()
            .map(|deck| DeckCardDeckDto {
                name: visible_deck_name(&deck),
                id: deck.id,
            })
            .collect();
        Ok(DeckCardOverviewDto {
            cards,
            decks,
            total_matches,
            offset: request.offset,
            limit: u32::try_from(limit)
                .map_err(|_| ApplicationError::NumericRange("deck card page size"))?,
        })
    }

    /// Applies one card action without changing unselected sibling cards.
    ///
    /// # Errors
    ///
    /// Returns an error when the selection, source deck, or destination is invalid.
    pub fn apply_deck_card_action(
        &self,
        request: &DeckCardActionRequest,
    ) -> Result<DeckCardActionResultDto, ApplicationError> {
        validate_action_request(request)?;
        let mut storage = self.open_storage()?;
        storage.get_deck(&request.deck_id)?;
        for card_id in &request.card_ids {
            let stored = storage.load_study_card(card_id)?;
            if stored.source_item.deck_id != request.deck_id {
                return Err(ApplicationError::InvalidDeckCard(
                    "every selected card must belong to the opened deck".into(),
                ));
            }
        }
        if matches!(
            request.action,
            DeckCardActionDto::Move | DeckCardActionDto::Trash | DeckCardActionDto::Restore
        ) {
            for card_id in &request.card_ids {
                self.isolate_card_source(&mut storage, card_id, request.now_ms)?;
            }
        }
        match request.action {
            DeckCardActionDto::Move => {
                let destination = request
                    .destination_deck_id
                    .as_deref()
                    .filter(|value| !value.trim().is_empty() && *value != request.deck_id)
                    .ok_or_else(|| {
                        ApplicationError::InvalidDeckCard(
                            "choose a different destination deck".into(),
                        )
                    })?;
                storage.move_deck_cards(&request.card_ids, destination, request.now_ms)?;
            }
            DeckCardActionDto::Suspend => {
                storage.set_deck_cards_suspended(&request.card_ids, true, request.now_ms)?;
            }
            DeckCardActionDto::Unsuspend => {
                storage.set_deck_cards_suspended(&request.card_ids, false, request.now_ms)?;
            }
            DeckCardActionDto::Trash => {
                storage.set_deck_cards_deleted(
                    &request.card_ids,
                    Some(request.now_ms),
                    request.now_ms,
                )?;
            }
            DeckCardActionDto::Restore => {
                storage.set_deck_cards_deleted(&request.card_ids, None, request.now_ms)?;
            }
        }
        Ok(DeckCardActionResultDto {
            affected_cards: desktop_u32(request.card_ids.len() as u64, "affected card count")?,
        })
    }

    pub(crate) fn isolate_card_source(
        &self,
        storage: &mut Storage,
        card_id: &str,
        updated_at_ms: i64,
    ) -> Result<(), ApplicationError> {
        let stored = storage.load_study_card(card_id)?;
        let note = storage.get_source_note(&stored.source_item.id)?;
        if note.clozes.len() == 1 {
            return Ok(());
        }
        let source_id = self.next_id("source");
        let segments = note
            .source_item
            .segments
            .iter()
            .map(|segment| SemanticSegment {
                id: self.next_id("segment"),
                ordinal: segment.ordinal,
                content: match &segment.content {
                    SegmentContent::Cloze { cloze_id, text } if cloze_id != &stored.cloze.id => {
                        SegmentContent::Text(text.clone())
                    }
                    content => content.clone(),
                },
            })
            .collect();
        let mut cloze = stored.cloze;
        cloze.source_item_id.clone_from(&source_id);
        let isolated = StoredSourceNote {
            source_item: SourceItem {
                id: source_id,
                deck_id: note.source_item.deck_id,
                segments,
                language_tag: note.source_item.language_tag,
                direction: note.source_item.direction,
                tags: note.source_item.tags,
                annotations: note
                    .source_item
                    .annotations
                    .into_iter()
                    .map(|mut annotation| {
                        annotation.id = self.next_id("annotation");
                        annotation
                    })
                    .collect(),
                explanation: note.source_item.explanation,
                media: note.source_item.media,
                created_at_ms: note.source_item.created_at_ms,
                updated_at_ms,
            },
            clozes: vec![cloze],
        };
        storage.isolate_card_source(card_id, &isolated)?;
        Ok(())
    }
}

fn validate_request(request: &DeckCardRequest) -> Result<(), ApplicationError> {
    if request.deck_id.trim().is_empty() || request.limit > MAX_PAGE_SIZE {
        return Err(ApplicationError::InvalidDeckCard(
            "the deck or page bounds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_action_request(request: &DeckCardActionRequest) -> Result<(), ApplicationError> {
    let unique = request.card_ids.iter().collect::<HashSet<_>>();
    if request.deck_id.trim().is_empty()
        || request.card_ids.is_empty()
        || unique.len() != request.card_ids.len()
        || request.card_ids.iter().any(|id| id.trim().is_empty())
    {
        return Err(ApplicationError::InvalidDeckCard(
            "select distinct cards from one opened deck".into(),
        ));
    }
    Ok(())
}

fn page_size(requested: u32) -> usize {
    if requested == 0 {
        DEFAULT_PAGE_SIZE
    } else {
        usize::try_from(requested).unwrap_or(DEFAULT_PAGE_SIZE)
    }
}

fn card_sentence(source: &SourceItem, active_cloze_id: &str) -> String {
    source
        .segments
        .iter()
        .map(|segment| match &segment.content {
            SegmentContent::Cloze { cloze_id, .. } if cloze_id == active_cloze_id => "[…]",
            SegmentContent::Text(text) | SegmentContent::Cloze { text, .. } => text,
        })
        .collect()
}

fn card_matches(
    source: &SourceItem,
    cloze: &meiki_domain::Cloze,
    sentence: &str,
    normalized_query: &str,
) -> bool {
    if normalized_query.is_empty() {
        return true;
    }
    let mut values = vec![sentence, cloze.answer.as_str()];
    values.extend(cloze.accepted_answers.iter().map(String::as_str));
    values.extend(source.tags.iter().map(|tag| tag.name.as_str()));
    if let Some(hint) = &cloze.hint {
        values.push(&hint.value);
    }
    values
        .into_iter()
        .any(|value| normalize_for_search(value).contains(normalized_query))
}

fn card_status(
    suspended: bool,
    lifecycle: CardLifecycle,
    due_at_ms: i64,
    now_ms: i64,
) -> DeckCardStatusDto {
    if suspended {
        DeckCardStatusDto::Suspended
    } else if lifecycle == CardLifecycle::Unseen {
        DeckCardStatusDto::New
    } else if due_at_ms <= now_ms {
        DeckCardStatusDto::Due
    } else {
        DeckCardStatusDto::Scheduled
    }
}

fn resolved_direction(
    cloze_direction: meiki_domain::Direction,
    source: &SourceItem,
    deck: &Deck,
) -> DirectionDto {
    match cloze_direction {
        meiki_domain::Direction::Auto => match source.direction {
            meiki_domain::Direction::Auto => deck.direction.into(),
            direction => direction.into(),
        },
        direction => direction.into(),
    }
}

fn visible_deck_name(deck: &Deck) -> String {
    if deck.id == DEFAULT_DECK_ID {
        "Unsorted".into()
    } else {
        deck.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use meiki_domain::{
        Card, CardLifecycle, Cloze, Direction, MatchingPolicy, ScheduleState, SegmentContent,
        SemanticSegment, SourceItem,
    };
    use meiki_storage::{
        CardRepository, DEFAULT_DECK_ID, SAMPLE_CARD_ID, SourceNoteRepository, Storage,
        StoredSourceNote,
    };
    use tempfile::tempdir;

    use super::{
        DeckCardActionDto, DeckCardActionRequest, DeckCardRequest, DeckCardTrashDto, card_sentence,
    };
    use crate::{
        ApplicationService, CreateDeckRequest, GradeDto, GradeReviewRequest, TodayRequest,
    };

    fn request(deck_id: &str, query: &str, trash: DeckCardTrashDto) -> DeckCardRequest {
        DeckCardRequest {
            deck_id: deck_id.into(),
            query: query.into(),
            trash,
            now_ms: 2_000,
            offset: 0,
            limit: 25,
        }
    }

    fn add_note(storage: &mut Storage, deck_id: &str, source_id: &str, answers: &[&str]) {
        let clozes = answers
            .iter()
            .enumerate()
            .map(|(index, answer)| Cloze {
                id: format!("{source_id}-cloze-{index}"),
                source_item_id: source_id.into(),
                answer: (*answer).into(),
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
            })
            .collect::<Vec<_>>();
        let segments = answers
            .iter()
            .enumerate()
            .flat_map(|(index, answer)| {
                [
                    SemanticSegment {
                        id: format!("{source_id}-text-{index}"),
                        ordinal: u32::try_from(index * 2).unwrap(),
                        content: SegmentContent::Text(if index == 0 {
                            "Sentence ".into()
                        } else {
                            " / ".into()
                        }),
                    },
                    SemanticSegment {
                        id: format!("{source_id}-segment-{index}"),
                        ordinal: u32::try_from(index * 2 + 1).unwrap(),
                        content: SegmentContent::Cloze {
                            cloze_id: format!("{source_id}-cloze-{index}"),
                            text: (*answer).into(),
                        },
                    },
                ]
            })
            .collect();
        storage
            .create_source_note(&StoredSourceNote {
                source_item: SourceItem {
                    id: source_id.into(),
                    deck_id: deck_id.into(),
                    segments,
                    language_tag: None,
                    direction: Direction::Auto,
                    tags: Vec::new(),
                    annotations: Vec::new(),
                    explanation: None,
                    media: Vec::new(),
                    created_at_ms: 1_000,
                    updated_at_ms: 1_000,
                },
                clozes,
            })
            .unwrap();
        for index in 0..answers.len() {
            let card_id = format!("{source_id}-card-{index}");
            storage
                .create_card(
                    &Card {
                        id: card_id.clone(),
                        cloze_id: format!("{source_id}-cloze-{index}"),
                        content_version: 0,
                        suspended: false,
                        created_at_ms: 1_000,
                        updated_at_ms: 1_000,
                    },
                    &ScheduleState {
                        card_id,
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
        }
    }

    #[test]
    fn card_page_is_deck_scoped_and_searches_mixed_unicode_scripts() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        service.seed_test_collection(1_000).unwrap();
        let other = service
            .create_deck(&CreateDeckRequest {
                name: "Other".into(),
                now_ms: 1_000,
            })
            .unwrap();
        let mut storage = service.open_storage().unwrap();
        add_note(&mut storage, DEFAULT_DECK_ID, "arabic-source", &["كتابًا"]);
        drop(storage);

        assert_eq!(
            service
                .get_deck_cards(&request(
                    DEFAULT_DECK_ID,
                    "図書館",
                    DeckCardTrashDto::Active,
                ))
                .unwrap()
                .total_matches,
            1
        );
        assert_eq!(
            service
                .get_deck_cards(&request(DEFAULT_DECK_ID, "كتابًا", DeckCardTrashDto::Active,))
                .unwrap()
                .total_matches,
            1
        );
        assert!(
            service
                .get_deck_cards(&request(&other.id, "", DeckCardTrashDto::Active))
                .unwrap()
                .cards
                .is_empty()
        );
    }

    #[test]
    fn card_actions_preserve_history_and_schedule_and_only_suspension_changes_eligibility() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        service.seed_test_collection(1_000).unwrap();
        let destination = service
            .create_deck(&CreateDeckRequest {
                name: "Moved".into(),
                now_ms: 1_000,
            })
            .unwrap();
        let today = TodayRequest {
            deck_id: DEFAULT_DECK_ID.into(),
            now_ms: 2_000,
            day_start_ms: 0,
            day_end_ms: 86_400_000,
        };
        assert_eq!(service.get_today_overview(&today).unwrap().queue.len(), 1);

        service
            .apply_deck_card_action(&DeckCardActionRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                card_ids: vec![SAMPLE_CARD_ID.into()],
                action: DeckCardActionDto::Suspend,
                destination_deck_id: None,
                now_ms: 2_001,
            })
            .unwrap();
        assert!(service.get_today_overview(&today).unwrap().queue.is_empty());
        service
            .apply_deck_card_action(&DeckCardActionRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                card_ids: vec![SAMPLE_CARD_ID.into()],
                action: DeckCardActionDto::Unsuspend,
                destination_deck_id: None,
                now_ms: 2_002,
            })
            .unwrap();
        assert_eq!(service.get_today_overview(&today).unwrap().queue.len(), 1);

        let study = service.get_study_card(SAMPLE_CARD_ID).unwrap();
        service
            .grade_review(&GradeReviewRequest {
                review_event_id: "deck-card-review".into(),
                card_id: SAMPLE_CARD_ID.into(),
                card_content_version: study.card_content_version,
                schedule_version: study.schedule_version,
                raw_response: "行きます".into(),
                chosen_grade: GradeDto::Good,
                response_duration_ms: 1_000,
            })
            .unwrap();
        let storage = service.open_storage().unwrap();
        let schedule = storage.load_schedule(SAMPLE_CARD_ID).unwrap();
        let history = storage.review_count(SAMPLE_CARD_ID).unwrap();
        drop(storage);

        service
            .apply_deck_card_action(&DeckCardActionRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                card_ids: vec![SAMPLE_CARD_ID.into()],
                action: DeckCardActionDto::Move,
                destination_deck_id: Some(destination.id.clone()),
                now_ms: 3_000,
            })
            .unwrap();
        service
            .apply_deck_card_action(&DeckCardActionRequest {
                deck_id: destination.id.clone(),
                card_ids: vec![SAMPLE_CARD_ID.into()],
                action: DeckCardActionDto::Trash,
                destination_deck_id: None,
                now_ms: 3_001,
            })
            .unwrap();
        assert!(
            service
                .get_deck_cards(&request(&destination.id, "", DeckCardTrashDto::Active,))
                .unwrap()
                .cards
                .is_empty()
        );
        assert_eq!(
            service
                .get_deck_cards(&request(&destination.id, "", DeckCardTrashDto::Trash,))
                .unwrap()
                .cards
                .len(),
            1
        );
        service
            .apply_deck_card_action(&DeckCardActionRequest {
                deck_id: destination.id,
                card_ids: vec![SAMPLE_CARD_ID.into()],
                action: DeckCardActionDto::Restore,
                destination_deck_id: None,
                now_ms: 3_002,
            })
            .unwrap();
        let storage = service.open_storage().unwrap();
        assert_eq!(storage.load_schedule(SAMPLE_CARD_ID).unwrap(), schedule);
        assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), history);
    }

    #[test]
    fn editing_moving_and_trashing_one_legacy_cloze_do_not_change_its_sibling_card() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        let destination = service
            .create_deck(&CreateDeckRequest {
                name: "Destination".into(),
                now_ms: 1_000,
            })
            .unwrap();
        let mut storage = service.open_storage().unwrap();
        add_note(
            &mut storage,
            DEFAULT_DECK_ID,
            "legacy",
            &["first", "second"],
        );
        let sibling_before = storage.load_study_card("legacy-card-1").unwrap();
        let sibling_card = sibling_before.card.clone();
        let sibling_cloze = sibling_before.cloze.clone();
        let sibling_sentence = card_sentence(&sibling_before.source_item, &sibling_cloze.id);
        let sibling_schedule = storage.load_schedule("legacy-card-1").unwrap();
        let selected_schedule = storage.load_schedule("legacy-card-0").unwrap();
        drop(storage);

        let mut draft = service
            .get_authoring_draft_for_card("legacy-card-0")
            .unwrap();
        assert_eq!(draft.clozes.len(), 1);
        draft.clozes[0].answer = "changed".into();
        draft
            .segments
            .iter_mut()
            .find(|segment| segment.cloze_id.as_deref() == Some(&draft.clozes[0].id))
            .unwrap()
            .text = "changed".into();
        service.save_authoring_draft(&draft).unwrap();
        service
            .apply_deck_card_action(&DeckCardActionRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                card_ids: vec!["legacy-card-0".into()],
                action: DeckCardActionDto::Move,
                destination_deck_id: Some(destination.id.clone()),
                now_ms: 2_000,
            })
            .unwrap();
        service
            .apply_deck_card_action(&DeckCardActionRequest {
                deck_id: destination.id.clone(),
                card_ids: vec!["legacy-card-0".into()],
                action: DeckCardActionDto::Trash,
                destination_deck_id: None,
                now_ms: 2_001,
            })
            .unwrap();

        let storage = service.open_storage().unwrap();
        let sibling = storage.load_study_card("legacy-card-1").unwrap();
        assert_eq!(sibling.card, sibling_card);
        assert_eq!(sibling.cloze, sibling_cloze);
        assert_eq!(
            card_sentence(&sibling.source_item, &sibling.cloze.id),
            sibling_sentence
        );
        assert_eq!(sibling.schedule, sibling_schedule);
        assert_eq!(sibling.source_item.deck_id, DEFAULT_DECK_ID);
        let selected = storage.load_study_card("legacy-card-0").unwrap();
        assert_eq!(selected.source_item.deck_id, destination.id);
        assert_eq!(selected.schedule, selected_schedule);
        assert_eq!(selected.cloze.answer, "changed");
        drop(storage);
        assert_eq!(
            service
                .get_deck_cards(&request(DEFAULT_DECK_ID, "", DeckCardTrashDto::Active,))
                .unwrap()
                .cards
                .len(),
            1
        );
        assert_eq!(
            service
                .get_deck_cards(&request(&destination.id, "", DeckCardTrashDto::Trash,))
                .unwrap()
                .cards
                .len(),
            1
        );
    }

    #[test]
    fn three_thousand_card_deck_returns_one_bounded_page_through_sqlite() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        let answers = (0..3_000)
            .map(|index| format!("answer-{index}"))
            .collect::<Vec<_>>();
        let answer_refs = answers.iter().map(String::as_str).collect::<Vec<_>>();
        let mut storage = service.open_storage().unwrap();
        add_note(&mut storage, DEFAULT_DECK_ID, "large", &answer_refs);
        drop(storage);

        let started = Instant::now();
        let overview = service
            .get_deck_cards(&request(DEFAULT_DECK_ID, "", DeckCardTrashDto::Active))
            .unwrap();
        let elapsed = started.elapsed();

        assert_eq!(overview.total_matches, 3_000);
        assert_eq!(overview.cards.len(), 25);
        assert!(
            elapsed < Duration::from_secs(10),
            "3,000-card deck page took {elapsed:?}"
        );
    }
}
