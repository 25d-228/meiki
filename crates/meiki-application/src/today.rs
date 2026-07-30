use meiki_domain::{CardLifecycle, ReviewEvent, StudySettings, StudySettingsOverride};
use meiki_storage::{DeckRepository, SchedulerProfileRepository};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ApplicationError, ApplicationService, timestamp_string};

const DEFAULT_RESPONSE_SECONDS: u64 = 20;
const MIN_RESPONSE_MILLISECONDS: u64 = 1_000;
const MAX_RESPONSE_MILLISECONDS: u64 = 10 * 60 * 1_000;
const MIN_ESTIMATE_SECONDS: u64 = 5;
const MAX_ESTIMATE_SECONDS: u64 = 120;

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
    pub next_due_at: Option<String>,
    pub queue: Vec<TodayQueueCardDto>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueueCandidate {
    card_id: String,
    deck_id: String,
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

impl ApplicationService {
    /// Builds a deterministic, read-only queue projection for one local day.
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
        let storage = self.open_storage()?;
        let deck = storage.get_deck(&request.deck_id)?;
        let profile = storage.get_scheduler_profile(&request.deck_id)?;
        let settings = StudySettings::resolve(
            &StudySettings::default(),
            &deck.settings,
            &StudySettingsOverride::default(),
        );
        let candidates = storage
            .study_cards_for_deck(&request.deck_id)?
            .into_iter()
            .filter(|stored| !stored.card.suspended)
            .map(|stored| QueueCandidate {
                card_id: stored.card.id,
                deck_id: stored.source_item.deck_id,
                due_at_ms: stored.schedule.due_at_ms,
                ideal_due_at_ms: stored.schedule.ideal_due_at_ms,
                lifecycle: stored.schedule.lifecycle,
                created_at_ms: stored.card.created_at_ms,
            })
            .collect::<Vec<_>>();
        let reviews = storage.active_review_events_for_deck(&request.deck_id)?;
        let plan = plan_today(
            &candidates,
            &reviews,
            request,
            settings.new_cards_per_day,
            profile.daily_time_budget_minutes,
        );
        let decks = storage
            .list_decks()?
            .into_iter()
            .map(|deck| TodayDeckDto {
                id: deck.id,
                name: deck.name,
            })
            .collect();

        Ok(TodayOverviewDto {
            deck_id: deck.id,
            deck_name: deck.name,
            decks,
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
            daily_time_budget_minutes: profile.daily_time_budget_minutes,
            next_due_at: plan.next_due_at_ms.map(timestamp_string).transpose()?,
            queue: plan
                .cards
                .into_iter()
                .map(|card| {
                    Ok(TodayQueueCardDto {
                        card_id: card.card_id,
                        deck_id: card.deck_id,
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
            card.lifecycle == CardLifecycle::Introduced && card.due_at_ms < request.day_end_ms
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
    new.truncate(selected_new);

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
    use chrono::Utc;
    use meiki_domain::{CardLifecycle, ComparisonResult, Grade, ReviewEventKind, ScheduleState};
    use meiki_storage::{SAMPLE_CARD_ID, Storage};
    use tempfile::tempdir;

    use super::{ApplicationError, ApplicationService, QueueCandidate, TodayRequest, plan_today};

    fn candidate(id: &str, due_at_ms: i64, lifecycle: CardLifecycle) -> QueueCandidate {
        QueueCandidate {
            card_id: id.to_owned(),
            deck_id: "deck".to_owned(),
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
        );
        assert!(plan.cards.is_empty());
        assert_eq!(plan.next_due_at_ms, Some(90_000_000));
    }

    #[test]
    fn queue_includes_reviews_due_later_in_the_local_day() {
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
        );
        assert_eq!(plan.due_reviews, 1);
        assert_eq!(plan.cards[0].card_id, "later-today");
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
        );

        assert_eq!(plan.due_reviews, 1);
        assert_eq!(plan.deferred_new_cards, 1);
        assert_eq!(plan.cards.len(), 1);
        assert_eq!(plan.cards[0].card_id, "lapsed");
        assert_eq!(plan.cards[0].lifecycle, CardLifecycle::Introduced);
    }

    #[test]
    fn overview_is_read_only_and_restart_safe() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        service.initialize_collection().unwrap();
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
        let plan = plan_today(&candidates, &[], &request(), 20, Some(60));
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
