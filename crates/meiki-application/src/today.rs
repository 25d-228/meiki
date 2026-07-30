use std::collections::{BTreeMap, HashMap};

use meiki_domain::{CardLifecycle, ReviewEvent};
use meiki_scheduler::{DeckIntakeCandidate, allocate_unseen_round_robin};
use meiki_storage::{DeckRepository, SchedulerProfileRepository, Storage};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    ApplicationError, ApplicationService, BudgetSourceDto, ReconcileStudyQueueRequest,
    StudyQueueEntryDto, desktop_u32, effective_budget, effective_study_settings,
    evaluate_and_store_policy, timestamp_string,
};

const DEFAULT_RESPONSE_SECONDS: u64 = 20;
const MIN_RESPONSE_MILLISECONDS: u64 = 1_000;
const MAX_RESPONSE_MILLISECONDS: u64 = 10 * 60 * 1_000;
const MIN_ESTIMATE_SECONDS: u64 = 5;
const MAX_ESTIMATE_SECONDS: u64 = 120;
pub const ALL_DECKS_ID: &str = "__all_decks__";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct TodayRequest {
    pub deck_id: String,
    #[ts(type = "number")]
    pub now_ms: i64,
    #[ts(type = "number")]
    pub day_start_ms: i64,
    #[ts(type = "number")]
    pub day_end_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct TodayDeckDto {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct TodayQueueCardDto {
    pub card_id: String,
    pub deck_id: String,
    pub card_content_version: u32,
    pub schedule_version: u32,
    pub due_at: String,
    pub ideal_due_at: String,
    pub overdue: bool,
    pub is_new: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct TodayOverviewDto {
    pub deck_id: String,
    pub deck_name: String,
    pub decks: Vec<TodayDeckDto>,
    pub due_reviews: u32,
    pub overdue_reviews: u32,
    pub new_cards: u32,
    pub deferred_new_cards: u32,
    pub estimated_seconds: u32,
    pub estimate_uses_history: bool,
    pub response_time_samples: u32,
    pub daily_time_budget_minutes: Option<u32>,
    pub budget_source: BudgetSourceDto,
    pub target_retention_basis_points: u16,
    pub policy_explanation: String,
    pub backlog_exceeds_budget: bool,
    pub next_due_at: Option<String>,
    pub queue: Vec<TodayQueueCardDto>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum StudyAvailabilityDto {
    Ready,
    EmptyCollection,
    NothingDue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct StudyPlanDto {
    pub availability: StudyAvailabilityDto,
    pub overview: TodayOverviewDto,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueueCandidate {
    card_id: String,
    deck_id: String,
    card_content_version: u64,
    schedule_version: u64,
    due_at_ms: i64,
    ideal_due_at_ms: i64,
    lifecycle: CardLifecycle,
    created_at_ms: i64,
}

#[derive(Debug)]
struct QueuePlan {
    due_reviews: usize,
    overdue_reviews: usize,
    deferred_new_cards: usize,
    estimated_seconds: u64,
    estimate_uses_history: bool,
    response_time_samples: usize,
    next_due_at_ms: Option<i64>,
    cards: Vec<QueueCandidate>,
}

struct TodayInputs {
    all_decks: bool,
    deck_id: String,
    deck_name: String,
    decks: Vec<TodayDeckDto>,
    candidates: Vec<QueueCandidate>,
    reviews: Vec<ReviewEvent>,
    daily_budget: u32,
    budget_source: BudgetSourceDto,
    target_retention_basis_points: u16,
    new_cards_per_day: u32,
    policy_explanation: String,
    backlog_exceeds_budget: bool,
}

impl ApplicationService {
    /// Builds a deterministic queue projection for one local day.
    ///
    /// Automatic mode may persist derived controller metadata. Card
    /// projections and immutable review history remain unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested deck is missing, the day bounds are
    /// invalid, or queue inputs cannot be loaded.
    pub fn get_today_overview(
        &self,
        request: &TodayRequest,
    ) -> Result<TodayOverviewDto, ApplicationError> {
        validate_day(request)?;
        let mut storage = self.open_storage()?;
        let inputs = load_today_inputs(&mut storage, request)?;
        let plan = plan_today(
            &inputs.candidates,
            &inputs.reviews,
            request,
            inputs.new_cards_per_day,
            Some(inputs.daily_budget),
            inputs.all_decks,
        );
        let backlog_exceeds_budget = if inputs.all_decks {
            plan.estimated_seconds > u64::from(inputs.daily_budget).saturating_mul(60)
        } else {
            inputs.backlog_exceeds_budget
        };
        let policy_explanation = if inputs.all_decks {
            format!(
                "{} min/day across all decks\nTarget retention: {}%\nNew cards today: {}\nReason: the collection budget is shared while due cards remain globally ordered.",
                inputs.daily_budget,
                super::format_retention(inputs.target_retention_basis_points),
                plan.cards
                    .iter()
                    .filter(|card| card.lifecycle == CardLifecycle::Unseen)
                    .count()
            )
        } else {
            inputs.policy_explanation
        };

        Ok(TodayOverviewDto {
            deck_id: inputs.deck_id,
            deck_name: inputs.deck_name,
            decks: inputs.decks,
            due_reviews: desktop_count(plan.due_reviews, "due review count")?,
            overdue_reviews: desktop_count(plan.overdue_reviews, "overdue review count")?,
            new_cards: desktop_count(
                plan.cards
                    .iter()
                    .filter(|card| card.lifecycle == CardLifecycle::Unseen)
                    .count(),
                "new card count",
            )?,
            deferred_new_cards: desktop_count(plan.deferred_new_cards, "deferred new card count")?,
            estimated_seconds: u32::try_from(plan.estimated_seconds)
                .map_err(|_| ApplicationError::NumericRange("estimated session seconds"))?,
            estimate_uses_history: plan.estimate_uses_history,
            response_time_samples: desktop_count(
                plan.response_time_samples,
                "response time sample count",
            )?,
            daily_time_budget_minutes: Some(inputs.daily_budget),
            budget_source: inputs.budget_source,
            target_retention_basis_points: inputs.target_retention_basis_points,
            policy_explanation,
            backlog_exceeds_budget,
            next_due_at: plan.next_due_at_ms.map(timestamp_string).transpose()?,
            queue: plan
                .cards
                .into_iter()
                .map(|card| {
                    Ok(TodayQueueCardDto {
                        card_id: card.card_id,
                        deck_id: card.deck_id,
                        card_content_version: desktop_u32(
                            card.card_content_version,
                            "card content version",
                        )?,
                        schedule_version: desktop_u32(card.schedule_version, "schedule version")?,
                        due_at: timestamp_string(card.due_at_ms)?,
                        ideal_due_at: timestamp_string(card.ideal_due_at_ms)?,
                        overdue: card.lifecycle == CardLifecycle::Introduced
                            && card.due_at_ms < request.day_start_ms,
                        is_new: card.lifecycle == CardLifecycle::Unseen,
                    })
                })
                .collect::<Result<Vec<_>, ApplicationError>>()?,
        })
    }

    /// Opens the collection and returns an explicit, read-only study plan.
    ///
    /// A clean collection remains empty. Normal empty and nothing-due states
    /// are returned as data rather than inferred from a hard-coded card.
    ///
    /// # Errors
    ///
    /// Returns an error when the current Today plan or collection contents
    /// cannot be loaded.
    pub fn prepare_study(&self, request: &TodayRequest) -> Result<StudyPlanDto, ApplicationError> {
        let overview = self.get_today_overview(request)?;
        let availability = if !overview.queue.is_empty() {
            StudyAvailabilityDto::Ready
        } else if self.open_storage()?.has_learning_material()? {
            StudyAvailabilityDto::NothingDue
        } else {
            StudyAvailabilityDto::EmptyCollection
        };
        Ok(StudyPlanDto {
            availability,
            overview,
        })
    }

    /// Reconciles a cached study queue with the current persisted schedule.
    ///
    /// Cached order is preserved, while duplicate, missing, changed, inactive,
    /// no-longer-selected, and not-yet-due entries are removed.
    ///
    /// # Errors
    ///
    /// Returns an error when the current Today plan cannot be loaded.
    pub fn reconcile_study_queue(
        &self,
        request: &ReconcileStudyQueueRequest,
    ) -> Result<Vec<StudyQueueEntryDto>, ApplicationError> {
        let overview = self.get_today_overview(&TodayRequest {
            deck_id: request.deck_id.clone(),
            now_ms: request.now_ms,
            day_start_ms: request.day_start_ms,
            day_end_ms: request.day_end_ms,
        })?;
        let eligible = overview
            .queue
            .into_iter()
            .map(|card| (card.card_id.clone(), card))
            .collect::<std::collections::HashMap<_, _>>();
        let mut seen = std::collections::HashSet::new();

        Ok(request
            .entries
            .iter()
            .filter_map(|entry| {
                if !seen.insert(entry.card_id.as_str()) {
                    return None;
                }
                let current = eligible.get(&entry.card_id)?;
                (entry.card_content_version == current.card_content_version
                    && entry.schedule_version == current.schedule_version)
                    .then(|| StudyQueueEntryDto {
                        card_id: current.card_id.clone(),
                        card_content_version: current.card_content_version,
                        schedule_version: current.schedule_version,
                    })
            })
            .collect())
    }
}

fn load_today_inputs(
    storage: &mut Storage,
    request: &TodayRequest,
) -> Result<TodayInputs, ApplicationError> {
    let decks = storage.list_decks()?;
    let all_decks = request.deck_id == ALL_DECKS_ID;
    let selected_ids = if all_decks {
        decks.iter().map(|deck| deck.id.clone()).collect::<Vec<_>>()
    } else {
        vec![
            decks
                .iter()
                .find(|deck| deck.id == request.deck_id)
                .ok_or_else(|| {
                    ApplicationError::InvalidToday("the selected deck no longer exists".into())
                })?
                .id
                .clone(),
        ]
    };
    for deck_id in &selected_ids {
        evaluate_and_store_policy(
            storage,
            deck_id,
            request.now_ms,
            request.day_start_ms,
            false,
        )?;
    }
    let collection = storage.collection_scheduling_settings()?;
    let policy_deck = if all_decks {
        decks
            .iter()
            .find(|deck| deck.id == meiki_storage::DEFAULT_DECK_ID)
            .or_else(|| decks.first())
    } else {
        decks.iter().find(|deck| deck.id == request.deck_id)
    }
    .cloned()
    .ok_or_else(|| ApplicationError::InvalidToday("the collection has no deck".into()))?;
    let profile = storage.get_scheduler_profile(&policy_deck.id)?;
    let settings = effective_study_settings(&policy_deck, &profile);
    let (daily_budget, budget_source) = if all_decks {
        (
            collection.daily_time_budget_minutes,
            BudgetSourceDto::CollectionBudget,
        )
    } else {
        effective_budget(&collection, &profile)
    };
    let mut candidates = Vec::new();
    let mut reviews = Vec::new();
    for deck_id in selected_ids {
        candidates.extend(
            storage
                .study_cards_for_deck(&deck_id)?
                .into_iter()
                .filter(|stored| !stored.card.suspended)
                .map(queue_candidate),
        );
        reviews.extend(storage.active_review_events_for_deck(&deck_id)?);
    }
    Ok(TodayInputs {
        all_decks,
        deck_id: if all_decks {
            ALL_DECKS_ID.into()
        } else {
            policy_deck.id.clone()
        },
        deck_name: if all_decks {
            "All decks".into()
        } else {
            policy_deck.name.clone()
        },
        decks: decks
            .into_iter()
            .map(|deck| TodayDeckDto {
                id: deck.id,
                name: deck.name,
            })
            .collect(),
        candidates,
        reviews,
        daily_budget,
        budget_source,
        target_retention_basis_points: settings.target_retention_basis_points,
        new_cards_per_day: settings.new_cards_per_day,
        policy_explanation: profile.controller_explanation,
        backlog_exceeds_budget: profile.controller_backlog_exceeds_budget,
    })
}

fn queue_candidate(stored: meiki_storage::StoredStudyCard) -> QueueCandidate {
    QueueCandidate {
        card_id: stored.card.id,
        deck_id: stored.source_item.deck_id,
        card_content_version: stored.card.content_version,
        schedule_version: stored.schedule.version,
        due_at_ms: stored.schedule.due_at_ms,
        ideal_due_at_ms: stored.schedule.ideal_due_at_ms,
        lifecycle: stored.schedule.lifecycle,
        created_at_ms: stored.card.created_at_ms,
    }
}

fn validate_day(request: &TodayRequest) -> Result<(), ApplicationError> {
    let duration = request.day_end_ms.saturating_sub(request.day_start_ms);
    if request.deck_id.trim().is_empty()
        || request.now_ms < request.day_start_ms
        || request.now_ms >= request.day_end_ms
        || !(20 * 60 * 60 * 1_000..=28 * 60 * 60 * 1_000).contains(&duration)
    {
        return Err(ApplicationError::InvalidToday(
            "the selected deck and local day bounds are invalid".to_owned(),
        ));
    }
    Ok(())
}

fn plan_today(
    candidates: &[QueueCandidate],
    reviews: &[ReviewEvent],
    request: &TodayRequest,
    new_cards_per_day: u32,
    daily_time_budget_minutes: Option<u32>,
    fair_across_decks: bool,
) -> QueuePlan {
    let (review_seconds, uses_history, response_time_samples) = response_time_estimate(reviews);
    let new_seconds = review_seconds.saturating_mul(3).div_ceil(2);
    let reviewed_new_today = reviews
        .iter()
        .filter(|event| {
            event.reviewed_at_ms >= request.day_start_ms
                && event.reviewed_at_ms < request.day_end_ms
                && event.previous_schedule.lifecycle == CardLifecycle::Unseen
        })
        .count();

    let mut due = candidates
        .iter()
        .filter(|card| {
            card.lifecycle == CardLifecycle::Introduced && card.due_at_ms <= request.now_ms
        })
        .cloned()
        .collect::<Vec<_>>();
    due.sort_by(|left, right| {
        left.due_at_ms
            .cmp(&right.due_at_ms)
            .then_with(|| left.card_id.cmp(&right.card_id))
    });
    let overdue_reviews = due
        .iter()
        .filter(|card| card.due_at_ms < request.day_start_ms)
        .count();

    let mut new = candidates
        .iter()
        .filter(|card| card.lifecycle == CardLifecycle::Unseen)
        .cloned()
        .collect::<Vec<_>>();
    new.sort_by(|left, right| {
        left.created_at_ms
            .cmp(&right.created_at_ms)
            .then_with(|| left.card_id.cmp(&right.card_id))
    });

    let daily_remaining = usize::try_from(new_cards_per_day)
        .unwrap_or(usize::MAX)
        .saturating_sub(reviewed_new_today);
    let due_estimate = u64::try_from(due.len())
        .unwrap_or(u64::MAX)
        .saturating_mul(review_seconds);
    let budget_remaining = daily_time_budget_minutes.map_or(u64::MAX, |minutes| {
        u64::from(minutes)
            .saturating_mul(60)
            .saturating_sub(due_estimate)
    });
    let budget_new = usize::try_from(budget_remaining / new_seconds).unwrap_or(usize::MAX);
    let selected_new = new.len().min(daily_remaining).min(budget_new);
    let deferred_new_cards = new.len().saturating_sub(selected_new);
    if fair_across_decks {
        new = fair_new_cards(new, selected_new);
    } else {
        new.truncate(selected_new);
    }

    let due_reviews = due.len();
    let estimated_seconds = due_estimate.saturating_add(
        u64::try_from(new.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(new_seconds),
    );
    due.extend(new);
    let queued_ids = due
        .iter()
        .map(|card| card.card_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let next_due_at_ms = candidates
        .iter()
        .filter(|card| {
            card.lifecycle == CardLifecycle::Introduced
                && !queued_ids.contains(card.card_id.as_str())
        })
        .map(|card| card.due_at_ms)
        .min();

    QueuePlan {
        due_reviews,
        overdue_reviews,
        deferred_new_cards,
        estimated_seconds,
        estimate_uses_history: uses_history,
        response_time_samples,
        next_due_at_ms,
        cards: due,
    }
}

fn fair_new_cards(candidates: Vec<QueueCandidate>, allowance: usize) -> Vec<QueueCandidate> {
    let mut by_deck = BTreeMap::<String, Vec<QueueCandidate>>::new();
    for candidate in candidates {
        by_deck
            .entry(candidate.deck_id.clone())
            .or_default()
            .push(candidate);
    }
    let allocation_candidates = by_deck
        .iter()
        .map(|(deck_id, cards)| DeckIntakeCandidate {
            deck_id: deck_id.clone(),
            unseen_cards: u64::try_from(cards.len()).unwrap_or(u64::MAX),
        })
        .collect::<Vec<_>>();
    let allocations = allocate_unseen_round_robin(
        &allocation_candidates,
        u32::try_from(allowance).unwrap_or(u32::MAX),
    )
    .into_iter()
    .map(|allocation| (allocation.deck_id, allocation.new_cards))
    .collect::<HashMap<_, _>>();
    let mut selected = by_deck
        .into_iter()
        .flat_map(|(deck_id, mut cards)| {
            cards.truncate(
                usize::try_from(allocations.get(&deck_id).copied().unwrap_or(0))
                    .unwrap_or(usize::MAX),
            );
            cards
        })
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| {
        left.created_at_ms
            .cmp(&right.created_at_ms)
            .then_with(|| left.deck_id.cmp(&right.deck_id))
            .then_with(|| left.card_id.cmp(&right.card_id))
    });
    selected
}

fn response_time_estimate(reviews: &[ReviewEvent]) -> (u64, bool, usize) {
    let mut samples = reviews
        .iter()
        .map(|event| event.response_duration_ms)
        .filter(|duration| {
            (MIN_RESPONSE_MILLISECONDS..=MAX_RESPONSE_MILLISECONDS).contains(duration)
        })
        .collect::<Vec<_>>();
    if samples.is_empty() {
        return (DEFAULT_RESPONSE_SECONDS, false, 0);
    }
    samples.sort_unstable();
    let median_ms = samples[samples.len() / 2];
    (
        median_ms
            .div_ceil(1_000)
            .clamp(MIN_ESTIMATE_SECONDS, MAX_ESTIMATE_SECONDS),
        true,
        samples.len(),
    )
}

fn desktop_count(value: usize, field: &'static str) -> Result<u32, ApplicationError> {
    u32::try_from(value).map_err(|_| ApplicationError::NumericRange(field))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::Utc;
    use meiki_domain::{
        CardLifecycle, ComparisonResult, Deck, Direction, Grade, LocalizedText, MatchingPolicy,
        ReviewEventKind, ScheduleState, StudySettingsOverride,
    };
    use meiki_storage::{
        CardRepository, DeckRepository, SAMPLE_CARD_ID, SAMPLE_SOURCE_ID,
        SchedulerProfileRepository, SourceNoteRepository, Storage,
    };
    use tempfile::{TempDir, tempdir};

    use super::{
        ALL_DECKS_ID, ApplicationError, ApplicationService, QueueCandidate,
        ReconcileStudyQueueRequest, StudyAvailabilityDto, StudyQueueEntryDto, TodayRequest,
        plan_today,
    };
    use crate::{
        GradeDto, GradeReviewRequest, LibraryDueFilterDto, LibraryMediaFilterDto, LibraryRequest,
        LibrarySuspendedFilterDto, LibraryTrashFilterDto, MakeClozeRequest,
    };

    fn candidate(id: &str, due_at_ms: i64, lifecycle: CardLifecycle) -> QueueCandidate {
        QueueCandidate {
            card_id: id.to_owned(),
            deck_id: "deck".to_owned(),
            card_content_version: 0,
            schedule_version: 0,
            due_at_ms,
            ideal_due_at_ms: due_at_ms,
            lifecycle,
            created_at_ms: due_at_ms,
        }
    }

    fn review(
        id: &str,
        reviewed_at_ms: i64,
        response_duration_ms: u64,
        previous_repetitions: u32,
    ) -> meiki_domain::ReviewEvent {
        let previous = ScheduleState {
            card_id: id.to_owned(),
            version: 0,
            lifecycle: if previous_repetitions == 0 {
                CardLifecycle::Unseen
            } else {
                CardLifecycle::Introduced
            },
            due_at_ms: reviewed_at_ms,
            ideal_due_at_ms: reviewed_at_ms,
            interval_milliseconds: 0,
            interval_seconds: 0,
            repetitions: previous_repetitions,
            stability_milliseconds: 0,
            difficulty_millipoints: 0,
            last_reviewed_at_ms: None,
            last_review_event_id: None,
        };
        let mut next = previous.clone();
        next.version = 1;
        next.lifecycle = CardLifecycle::Introduced;
        next.repetitions += 1;
        next.last_review_event_id = Some(format!("review-{id}"));
        meiki_domain::ReviewEvent {
            id: format!("review-{id}"),
            card_id: id.to_owned(),
            card_content_version: 0,
            kind: ReviewEventKind::Review,
            undoes_review_event_id: None,
            raw_response: String::new(),
            normalized_response: String::new(),
            comparison: ComparisonResult::Exact,
            suggested_grade: Grade::Good,
            chosen_grade: Grade::Good,
            grade_overridden: false,
            response_duration_ms,
            reviewed_at_ms,
            scheduler_version: "fsrs-7".to_owned(),
            scheduler_parameter_set_id: None,
            target_retention_basis_points: 9_000,
            previous_schedule: previous,
            next_schedule: next,
        }
    }

    fn request() -> TodayRequest {
        TodayRequest {
            deck_id: "deck".to_owned(),
            now_ms: 100_000,
            day_start_ms: 0,
            day_end_ms: 86_400_000,
        }
    }

    fn request_at(now_ms: i64) -> TodayRequest {
        const DAY_MILLISECONDS: i64 = 86_400_000;
        let day_start_ms = now_ms.div_euclid(DAY_MILLISECONDS) * DAY_MILLISECONDS;
        TodayRequest {
            deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
            now_ms,
            day_start_ms,
            day_end_ms: day_start_ms + DAY_MILLISECONDS,
        }
    }

    fn seeded_session() -> (
        TempDir,
        PathBuf,
        ApplicationService,
        TodayRequest,
        StudyQueueEntryDto,
    ) {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let mut storage = Storage::open(&path).unwrap();
        storage.seed_walking_skeleton(100_000).unwrap();
        drop(storage);
        let service = ApplicationService::new(&path);
        let mut request = request();
        request.deck_id = meiki_storage::DEFAULT_DECK_ID.into();
        let queued = service.get_today_overview(&request).unwrap().queue[0].clone();
        let entry = StudyQueueEntryDto {
            card_id: queued.card_id,
            card_content_version: queued.card_content_version,
            schedule_version: queued.schedule_version,
        };
        (directory, path, service, request, entry)
    }

    fn reconcile(
        service: &ApplicationService,
        request: &TodayRequest,
        entries: Vec<StudyQueueEntryDto>,
    ) -> Vec<StudyQueueEntryDto> {
        service
            .reconcile_study_queue(&ReconcileStudyQueueRequest {
                deck_id: request.deck_id.clone(),
                now_ms: request.now_ms,
                day_start_ms: request.day_start_ms,
                day_end_ms: request.day_end_ms,
                entries,
            })
            .unwrap()
    }

    #[test]
    fn queue_is_deterministic_and_never_defers_due_reviews_for_budget() {
        let plan = plan_today(
            &[
                candidate("new-b", 2, CardLifecycle::Unseen),
                candidate("due-b", 90_000, CardLifecycle::Introduced),
                candidate("due-a", -1, CardLifecycle::Introduced),
                candidate("new-a", 1, CardLifecycle::Unseen),
            ],
            &[],
            &request(),
            20,
            Some(1),
            false,
        );
        assert_eq!(plan.due_reviews, 2);
        assert_eq!(plan.overdue_reviews, 1);
        assert_eq!(
            plan.cards
                .iter()
                .map(|card| card.card_id.as_str())
                .collect::<Vec<_>>(),
            vec!["due-a", "due-b"]
        );
        assert_eq!(plan.deferred_new_cards, 2);
        assert_eq!(plan.estimated_seconds, 40);
    }

    #[test]
    fn all_decks_orders_due_timestamps_globally_and_shares_new_intake() {
        let mut due_later = candidate("due-later", 90_000, CardLifecycle::Introduced);
        due_later.deck_id = "deck-b".into();
        let mut due_first = candidate("due-first", 80_000, CardLifecycle::Introduced);
        due_first.deck_id = "deck-a".into();
        let mut new_a = candidate("new-a", 1, CardLifecycle::Unseen);
        new_a.deck_id = "deck-a".into();
        let mut new_b = candidate("new-b", 2, CardLifecycle::Unseen);
        new_b.deck_id = "deck-b".into();
        let mut new_a_second = candidate("new-a-second", 3, CardLifecycle::Unseen);
        new_a_second.deck_id = "deck-a".into();

        let plan = plan_today(
            &[new_a_second, due_later, new_b, due_first, new_a],
            &[],
            &request(),
            2,
            Some(10),
            true,
        );
        assert_eq!(
            plan.cards
                .iter()
                .map(|card| card.card_id.as_str())
                .collect::<Vec<_>>(),
            vec!["due-first", "due-later", "new-a", "new-b"]
        );
        assert_eq!(plan.deferred_new_cards, 1);

        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        service.seed_test_collection(1_000).unwrap();
        let mut all_request = request();
        all_request.deck_id = ALL_DECKS_ID.into();
        let overview = service.get_today_overview(&all_request).unwrap();
        assert_eq!(overview.deck_id, ALL_DECKS_ID);
        assert_eq!(overview.deck_name, "All decks");
        assert_eq!(
            overview.budget_source,
            crate::BudgetSourceDto::CollectionBudget
        );
        assert_eq!(overview.queue.len(), 1);
    }

    #[test]
    fn daily_limit_history_and_time_budget_cap_only_new_intake() {
        let request = request();
        let mut lapsed = review("lapsed", 60_000, 0, 0);
        lapsed.previous_schedule.lifecycle = CardLifecycle::Introduced;
        let history = vec![
            review("studied-new", 50_000, 10_000, 0),
            lapsed,
            review("reviewed", -1, 30_000, 2),
            review("outlier", -2, 900_000, 2),
        ];
        let plan = plan_today(
            &[
                candidate("due", 99_000, CardLifecycle::Introduced),
                candidate("new-a", 1, CardLifecycle::Unseen),
                candidate("new-b", 2, CardLifecycle::Unseen),
                candidate("new-c", 3, CardLifecycle::Unseen),
            ],
            &history,
            &request,
            3,
            Some(2),
            false,
        );
        assert!(plan.estimate_uses_history);
        assert_eq!(plan.response_time_samples, 2);
        assert_eq!(plan.due_reviews, 1);
        assert_eq!(
            plan.cards
                .iter()
                .map(|card| card.card_id.as_str())
                .collect::<Vec<_>>(),
            vec!["due", "new-a", "new-b"]
        );
        assert_eq!(plan.deferred_new_cards, 1);
        assert_eq!(plan.estimated_seconds, 120);
    }

    #[test]
    fn empty_queue_reports_the_next_future_due_timestamp() {
        let plan = plan_today(
            &[candidate("later", 90_000_000, CardLifecycle::Introduced)],
            &[],
            &request(),
            20,
            None,
            false,
        );
        assert!(plan.cards.is_empty());
        assert_eq!(plan.next_due_at_ms, Some(90_000_000));
    }

    #[test]
    fn queue_excludes_reviews_due_later_in_the_local_day() {
        let plan = plan_today(
            &[candidate(
                "later-today",
                1_000_000,
                CardLifecycle::Introduced,
            )],
            &[],
            &request(),
            20,
            None,
            false,
        );
        assert_eq!(plan.due_reviews, 0);
        assert!(plan.cards.is_empty());
        assert_eq!(plan.next_due_at_ms, Some(1_000_000));
    }

    #[test]
    fn queue_includes_reviews_at_the_exact_current_timestamp() {
        let plan = plan_today(
            &[
                candidate("due-now", 100_000, CardLifecycle::Introduced),
                candidate("future", 100_001, CardLifecycle::Introduced),
            ],
            &[],
            &request(),
            20,
            None,
            false,
        );
        assert_eq!(plan.due_reviews, 1);
        assert_eq!(plan.cards[0].card_id, "due-now");
        assert_eq!(plan.next_due_at_ms, Some(100_001));
    }

    #[test]
    fn exact_due_matrix_excludes_only_the_future_millisecond() {
        let plan = plan_today(
            &[
                candidate("one-ms-before", 99_999, CardLifecycle::Introduced),
                candidate("exactly-now", 100_000, CardLifecycle::Introduced),
                candidate("one-ms-after", 100_001, CardLifecycle::Introduced),
            ],
            &[],
            &request(),
            20,
            None,
            false,
        );
        assert_eq!(
            plan.cards
                .iter()
                .map(|entry| entry.card_id.as_str())
                .collect::<Vec<_>>(),
            ["one-ms-before", "exactly-now"]
        );
        assert_eq!(plan.next_due_at_ms, Some(100_001));
    }

    #[test]
    fn resumed_queue_reconciles_against_current_persisted_state() {
        let (_directory, _path, service, request, entry) = seeded_session();
        let unknown = StudyQueueEntryDto {
            card_id: "missing-card".into(),
            card_content_version: 0,
            schedule_version: 0,
        };
        assert_eq!(
            reconcile(
                &service,
                &request,
                vec![entry.clone(), entry.clone(), unknown]
            ),
            vec![entry]
        );

        let (_directory, path, service, request, entry) = seeded_session();
        let mut storage = Storage::open(&path).unwrap();
        let mut card = storage.get_card(SAMPLE_CARD_ID).unwrap();
        card.content_version += 1;
        card.updated_at_ms += 1;
        storage.update_card(&card).unwrap();
        drop(storage);
        assert!(reconcile(&service, &request, vec![entry]).is_empty());

        let (_directory, path, service, request, entry) = seeded_session();
        let mut storage = Storage::open(&path).unwrap();
        storage
            .set_library_notes_suspended(&[SAMPLE_SOURCE_ID.into()], true, 100_001)
            .unwrap();
        drop(storage);
        assert!(reconcile(&service, &request, vec![entry]).is_empty());

        let (_directory, path, service, request, entry) = seeded_session();
        let mut storage = Storage::open(&path).unwrap();
        storage
            .set_library_notes_deleted(&[SAMPLE_SOURCE_ID.into()], Some(100_001), 100_001)
            .unwrap();
        drop(storage);
        assert!(reconcile(&service, &request, vec![entry]).is_empty());

        let (_directory, path, service, request, entry) = seeded_session();
        let mut storage = Storage::open(&path).unwrap();
        storage
            .create_deck(&Deck {
                id: "moved-deck".into(),
                name: "Moved".into(),
                description: None,
                language_tag: None,
                direction: Direction::Auto,
                matching_policy: MatchingPolicy::Strict,
                settings: StudySettingsOverride::default(),
                created_at_ms: 100_001,
                updated_at_ms: 100_001,
            })
            .unwrap();
        storage
            .move_library_notes(&[SAMPLE_SOURCE_ID.into()], "moved-deck", 100_001)
            .unwrap();
        drop(storage);
        assert!(reconcile(&service, &request, vec![entry]).is_empty());

        let (_directory, path, service, request, entry) = seeded_session();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "committed-before-response".into(),
                    card_id: entry.card_id.clone(),
                    card_content_version: entry.card_content_version,
                    schedule_version: entry.schedule_version,
                    raw_response: "行きます".into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 1_000,
                },
                request.now_ms,
            )
            .unwrap();
        assert!(reconcile(&service, &request, vec![entry]).is_empty());
        let scheduled = Storage::open(&path)
            .unwrap()
            .load_schedule(SAMPLE_CARD_ID)
            .unwrap();
        let due_request = request_at(scheduled.due_at_ms);
        let due_card = service.get_today_overview(&due_request).unwrap().queue[0].clone();
        let current_entry = StudyQueueEntryDto {
            card_id: due_card.card_id,
            card_content_version: due_card.card_content_version,
            schedule_version: due_card.schedule_version,
        };
        let rewound_request = request_at(scheduled.due_at_ms - 1);
        assert!(reconcile(&service, &rewound_request, vec![current_entry]).is_empty());
        assert_eq!(
            Storage::open(&path)
                .unwrap()
                .review_events(SAMPLE_CARD_ID)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn lapsed_introduced_card_is_due_and_never_consumes_new_quota() {
        let plan = plan_today(
            &[
                candidate("lapsed", 50_000, CardLifecycle::Introduced),
                candidate("unseen", 1, CardLifecycle::Unseen),
            ],
            &[],
            &request(),
            0,
            None,
            false,
        );

        assert_eq!(plan.due_reviews, 1);
        assert_eq!(plan.deferred_new_cards, 1);
        assert_eq!(plan.cards.len(), 1);
        assert_eq!(plan.cards[0].card_id, "lapsed");
        assert_eq!(plan.cards[0].lifecycle, CardLifecycle::Introduced);
    }

    #[test]
    fn overview_preserves_learning_state_and_is_restart_safe() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        service.seed_test_collection(1_000).unwrap();
        let before = Storage::open(&path)
            .unwrap()
            .load_study_card(SAMPLE_CARD_ID)
            .unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let request = TodayRequest {
            deck_id: "default-deck".to_owned(),
            now_ms,
            day_start_ms: now_ms - 12 * 60 * 60 * 1_000,
            day_end_ms: now_ms + 12 * 60 * 60 * 1_000,
        };

        let overview = service.get_today_overview(&request).unwrap();
        assert_eq!(overview.new_cards, 1);
        assert_eq!(overview.due_reviews, 0);
        assert_eq!(overview.queue[0].card_id, SAMPLE_CARD_ID);

        let storage = Storage::open(&path).unwrap();
        assert_eq!(storage.load_study_card(SAMPLE_CARD_ID).unwrap(), before);
        assert!(
            storage
                .active_review_events_for_deck("default-deck")
                .unwrap()
                .is_empty()
        );
        drop(storage);

        let restarted = ApplicationService::new(&path);
        assert_eq!(restarted.get_today_overview(&request).unwrap(), overview);
    }

    #[test]
    fn controller_recomputes_once_across_short_and_long_local_days() {
        const HOUR_MS: i64 = 60 * 60 * 1_000;
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        service.seed_test_collection(1_000).unwrap();

        let first = TodayRequest {
            deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
            now_ms: 100_000,
            day_start_ms: 0,
            day_end_ms: 25 * HOUR_MS,
        };
        service.get_today_overview(&first).unwrap();
        let first_profile = Storage::open(&path)
            .unwrap()
            .get_scheduler_profile(meiki_storage::DEFAULT_DECK_ID)
            .unwrap();
        assert_eq!(
            first_profile.controller_last_evaluated_day_start_ms,
            Some(0)
        );
        assert_eq!(first_profile.updated_at_ms, first.now_ms);

        let same_day = TodayRequest {
            now_ms: first.now_ms + 1,
            ..first.clone()
        };
        service.get_today_overview(&same_day).unwrap();
        let unchanged = Storage::open(&path)
            .unwrap()
            .get_scheduler_profile(meiki_storage::DEFAULT_DECK_ID)
            .unwrap();
        assert_eq!(unchanged.updated_at_ms, first.now_ms);

        let short_day_start = 23 * HOUR_MS;
        let short_day = TodayRequest {
            deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
            now_ms: short_day_start + 100_000,
            day_start_ms: short_day_start,
            day_end_ms: short_day_start + 25 * HOUR_MS,
        };
        service.get_today_overview(&short_day).unwrap();
        let after_short_day = Storage::open(&path)
            .unwrap()
            .get_scheduler_profile(meiki_storage::DEFAULT_DECK_ID)
            .unwrap();
        assert_eq!(
            after_short_day.controller_last_evaluated_day_start_ms,
            Some(short_day_start)
        );

        let long_day_start = short_day_start + 25 * HOUR_MS;
        let long_day = TodayRequest {
            deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
            now_ms: long_day_start + 100_000,
            day_start_ms: long_day_start,
            day_end_ms: long_day_start + 23 * HOUR_MS,
        };
        service.get_today_overview(&long_day).unwrap();
        let after_long_day = Storage::open(&path)
            .unwrap()
            .get_scheduler_profile(meiki_storage::DEFAULT_DECK_ID)
            .unwrap();
        assert_eq!(
            after_long_day.controller_last_evaluated_day_start_ms,
            Some(long_day_start)
        );
        assert_eq!(
            after_long_day.controller_target_retention_basis_points,
            first_profile.controller_target_retention_basis_points
        );
    }

    #[test]
    fn clean_collection_stays_empty_across_screens_and_restart_until_the_first_cloze() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        let request = request_at(100_000);

        let study = service.prepare_study(&request).unwrap();
        assert_eq!(study.availability, StudyAvailabilityDto::EmptyCollection);
        assert!(study.overview.queue.is_empty());
        assert_eq!(study.overview.next_due_at, None);
        assert!(
            service
                .get_library(&LibraryRequest {
                    query: String::new(),
                    deck_id: None,
                    tag_id: None,
                    due: LibraryDueFilterDto::All,
                    suspended: LibrarySuspendedFilterDto::All,
                    language_tag: None,
                    media: LibraryMediaFilterDto::All,
                    trash: LibraryTrashFilterDto::Active,
                    now_ms: request.now_ms,
                    offset: 0,
                    limit: 50,
                })
                .unwrap()
                .notes
                .is_empty()
        );
        service
            .get_scheduler_settings(meiki_storage::DEFAULT_DECK_ID)
            .unwrap();
        assert!(
            service
                .get_today_overview(&request)
                .unwrap()
                .queue
                .is_empty()
        );

        {
            let storage = Storage::open(&path).unwrap();
            assert!(storage.library_notes().unwrap().is_empty());
            assert!(
                storage
                    .study_cards_for_deck(meiki_storage::DEFAULT_DECK_ID)
                    .unwrap()
                    .is_empty()
            );
            assert!(
                storage
                    .active_review_events_for_deck(meiki_storage::DEFAULT_DECK_ID)
                    .unwrap()
                    .is_empty()
            );
        }

        let restarted = ApplicationService::new(&path);
        assert_eq!(
            restarted.prepare_study(&request).unwrap().availability,
            StudyAvailabilityDto::EmptyCollection
        );
        assert!(
            Storage::open(&path)
                .unwrap()
                .library_notes()
                .unwrap()
                .is_empty()
        );

        let mut draft = restarted.new_authoring_draft().unwrap();
        draft.segments[0].text = "Read widely".into();
        let segment_id = draft.segments[0].id.clone();
        let draft = restarted
            .make_cloze(MakeClozeRequest {
                draft,
                segment_id,
                selection_start_utf16: 5,
                selection_end_utf16: 11,
            })
            .unwrap();
        let saved = restarted.save_authoring_draft(&draft).unwrap();
        let study = restarted.prepare_study(&request).unwrap();
        assert_eq!(study.availability, StudyAvailabilityDto::Ready);
        assert_eq!(study.overview.queue.len(), 1);
        assert_eq!(study.overview.queue[0].card_id, saved.clozes[0].card_id);
        let library = restarted
            .get_library(&LibraryRequest {
                query: String::new(),
                deck_id: None,
                tag_id: None,
                due: LibraryDueFilterDto::All,
                suspended: LibrarySuspendedFilterDto::All,
                language_tag: None,
                media: LibraryMediaFilterDto::All,
                trash: LibraryTrashFilterDto::Active,
                now_ms: request.now_ms,
                offset: 0,
                limit: 50,
            })
            .unwrap();
        assert_eq!(library.notes.len(), 1);
        assert_eq!(library.notes[0].source_id, saved.source_id);
        assert_eq!(library.notes[0].cards[0].card_id, saved.clozes[0].card_id);
    }

    #[test]
    fn study_plan_preserves_legacy_content_and_reports_its_next_due_time() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        let card = service.seed_test_collection(100_000).unwrap();
        assert_eq!(
            service
                .prepare_study(&request_at(100_000))
                .unwrap()
                .availability,
            StudyAvailabilityDto::Ready
        );

        {
            let mut storage = Storage::open(&path).unwrap();
            let mut note = storage.get_source_note(SAMPLE_SOURCE_ID).unwrap();
            note.source_item.explanation = Some(LocalizedText {
                value: "Keep this user edit".into(),
                language_tag: Some("en".into()),
                direction: Direction::LeftToRight,
            });
            note.source_item.updated_at_ms = 100_001;
            storage.update_source_note(&note).unwrap();
            let mut stored_card = storage.get_card(SAMPLE_CARD_ID).unwrap();
            stored_card.content_version += 1;
            stored_card.updated_at_ms = 100_001;
            storage.update_card(&stored_card).unwrap();
        }
        let changed = service.get_study_card(SAMPLE_CARD_ID).unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "legacy-reviewed".into(),
                    card_id: card.card_id,
                    card_content_version: changed.card_content_version,
                    schedule_version: changed.schedule_version,
                    raw_response: "行きます".into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 1_000,
                },
                100_001,
            )
            .unwrap();
        let schedule = Storage::open(&path)
            .unwrap()
            .load_schedule(SAMPLE_CARD_ID)
            .unwrap();

        let study = service
            .prepare_study(&request_at(schedule.due_at_ms - 1))
            .unwrap();
        assert_eq!(study.availability, StudyAvailabilityDto::NothingDue);
        assert_eq!(
            study.overview.next_due_at,
            Some(crate::timestamp_string(schedule.due_at_ms).unwrap())
        );
        let storage = Storage::open(&path).unwrap();
        let note = storage.get_source_note(SAMPLE_SOURCE_ID).unwrap();
        assert_eq!(
            note.source_item.explanation.unwrap().value,
            "Keep this user edit"
        );
        assert_eq!(storage.get_card(SAMPLE_CARD_ID).unwrap().content_version, 1);
        assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 1);
    }

    #[test]
    fn invalid_local_day_is_rejected() {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        let error = service
            .get_today_overview(&TodayRequest {
                deck_id: "default-deck".to_owned(),
                now_ms: 1,
                day_start_ms: 1,
                day_end_ms: 1,
            })
            .unwrap_err();
        assert!(matches!(error, ApplicationError::InvalidToday(_)));
    }

    #[test]
    #[ignore = "release performance budget; run with scripts/performance"]
    fn release_budget_one_million_card_today_queue() {
        let candidates = (0..1_000_000)
            .map(|index| {
                let lifecycle = if index % 4 == 0 {
                    CardLifecycle::Unseen
                } else {
                    CardLifecycle::Introduced
                };
                candidate(
                    &format!("card-{index:07}"),
                    i64::from(index % 86_400_000),
                    lifecycle,
                )
            })
            .collect::<Vec<_>>();
        let started = std::time::Instant::now();
        let mut request = request();
        request.now_ms = request.day_end_ms - 1;
        let plan = plan_today(&candidates, &[], &request, 20, Some(60), false);
        let elapsed = started.elapsed();

        assert_eq!(plan.due_reviews, 750_000);
        assert!(
            elapsed <= std::time::Duration::from_secs(15),
            "one-million-card Today queue exceeded 15 s: {elapsed:?}"
        );
        eprintln!(
            "release-budget today_queue_one_million elapsed_ms={}",
            elapsed.as_millis()
        );
    }
}
