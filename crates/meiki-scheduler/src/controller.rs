//! Deterministic time-budget scheduling policy.
//!
//! The controller changes policy inputs only. It never fits or invents
//! memory-model parameters and never rewrites an existing schedule.

use std::collections::BTreeMap;

pub const CONTROLLER_VERSION: &str = "time-budget-v1";
pub const FORECAST_DAYS: u32 = 28;
pub const MINIMUM_TARGET_RETENTION_BASIS_POINTS: u16 = 8_000;
pub const BASELINE_TARGET_RETENTION_BASIS_POINTS: u16 = 9_000;
pub const MAXIMUM_TARGET_RETENTION_BASIS_POINTS: u16 = 9_500;
const TARGET_STEP_BASIS_POINTS: u16 = 100;
const MAXIMUM_NEW_CARDS_PER_DAY: u32 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutomaticPolicyInput {
    pub daily_budget_minutes: u32,
    pub due_cards_now: u64,
    /// Projected review occurrences during the bounded rolling horizon.
    pub forecast_review_occurrences: u64,
    pub response_seconds: u64,
    pub unseen_cards: u64,
    pub current_target_retention_basis_points: u16,
    pub previous_target_retention_basis_points: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutomaticPolicyDecision {
    pub controller_version: &'static str,
    pub target_retention_basis_points: u16,
    pub new_cards_per_day: u32,
    pub due_work_seconds: u64,
    pub forecast_review_seconds_per_day: u64,
    pub backlog_exceeds_budget: bool,
    pub explanation: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckIntakeCandidate {
    pub deck_id: String,
    pub unseen_cards: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckIntakeAllocation {
    pub deck_id: String,
    pub new_cards: u32,
}

/// Chooses a bounded, explainable scheduling policy from aggregate workload.
///
/// Target candidates are deliberately discrete. The fixed workload factors
/// approximate the relative review frequency needed at each recall target;
/// they are policy constants, not fitted learner-memory weights.
#[must_use]
pub fn automatic_policy(input: AutomaticPolicyInput) -> AutomaticPolicyDecision {
    let budget_seconds = u64::from(input.daily_budget_minutes).saturating_mul(60);
    let response_seconds = input.response_seconds.max(1);
    let due_work_seconds = input.due_cards_now.saturating_mul(response_seconds);
    let current_target = input.current_target_retention_basis_points.clamp(
        MINIMUM_TARGET_RETENTION_BASIS_POINTS,
        MAXIMUM_TARGET_RETENTION_BASIS_POINTS,
    );
    let previous_target = input.previous_target_retention_basis_points.clamp(
        MINIMUM_TARGET_RETENTION_BASIS_POINTS,
        MAXIMUM_TARGET_RETENTION_BASIS_POINTS,
    );

    let baseline_work = forecast_seconds_per_day(
        input.forecast_review_occurrences,
        response_seconds,
        BASELINE_TARGET_RETENTION_BASIS_POINTS,
        current_target,
    );
    let desired_target = desired_target(
        &input,
        baseline_work,
        budget_seconds,
        response_seconds,
        current_target,
    );
    let target_retention_basis_points = desired_target.clamp(
        previous_target.saturating_sub(TARGET_STEP_BASIS_POINTS),
        previous_target
            .saturating_add(TARGET_STEP_BASIS_POINTS)
            .min(MAXIMUM_TARGET_RETENTION_BASIS_POINTS),
    );
    let forecast_review_seconds_per_day = forecast_seconds_per_day(
        input.forecast_review_occurrences,
        response_seconds,
        target_retention_basis_points,
        current_target,
    );
    let remaining_seconds = budget_seconds.saturating_sub(forecast_review_seconds_per_day);
    let new_card_cost = new_card_daily_cost(response_seconds, target_retention_basis_points);
    let allowed_by_budget = remaining_seconds / new_card_cost.max(1);
    let new_cards_per_day = if due_work_seconds >= budget_seconds || baseline_work > budget_seconds
    {
        // Intake reaches zero before the controller lowers retention. This
        // remains true while smoothing moves toward a lower target.
        0
    } else {
        u32::try_from(input.unseen_cards.min(allowed_by_budget))
            .unwrap_or(u32::MAX)
            .min(MAXIMUM_NEW_CARDS_PER_DAY)
    };
    let backlog_exceeds_budget = due_work_seconds > budget_seconds;
    let forecast_minutes = forecast_review_seconds_per_day.div_ceil(60);
    let reason = if backlog_exceeds_budget {
        format!(
            "the current due backlog needs about {} min; all due reviews remain available",
            due_work_seconds.div_ceil(60)
        )
    } else if forecast_review_seconds_per_day > budget_seconds {
        format!(
            "forecast review work remains above budget at the safe retention limit ({forecast_minutes} min/day)"
        )
    } else if new_cards_per_day > 0 {
        format!(
            "due and forecast review work is about {forecast_minutes} min/day; remaining time introduces new cards"
        )
    } else if input.unseen_cards == 0
        && target_retention_basis_points > BASELINE_TARGET_RETENTION_BASIS_POINTS
    {
        format!(
            "there are no unseen cards, so spare capacity raises retention; forecast review work is {forecast_minutes} min/day"
        )
    } else {
        format!(
            "due and forecast review work is {forecast_minutes} min/day within the selected budget"
        )
    };
    AutomaticPolicyDecision {
        controller_version: CONTROLLER_VERSION,
        target_retention_basis_points,
        new_cards_per_day,
        due_work_seconds,
        forecast_review_seconds_per_day,
        backlog_exceeds_budget,
        explanation: format!(
            "{} min/day\nTarget retention: {}%\nNew cards today: {}\nReason: {reason}",
            input.daily_budget_minutes,
            format_retention(target_retention_basis_points),
            new_cards_per_day,
        ),
    }
}

fn desired_target(
    input: &AutomaticPolicyInput,
    baseline_work: u64,
    budget_seconds: u64,
    response_seconds: u64,
    current_target: u16,
) -> u16 {
    if baseline_work <= budget_seconds {
        if input.unseen_cards == 0 {
            retention_candidates()
                .rev()
                .find(|candidate| {
                    forecast_seconds_per_day(
                        input.forecast_review_occurrences,
                        response_seconds,
                        *candidate,
                        current_target,
                    ) <= budget_seconds
                })
                .unwrap_or(BASELINE_TARGET_RETENTION_BASIS_POINTS)
        } else {
            BASELINE_TARGET_RETENTION_BASIS_POINTS
        }
    } else {
        retention_candidates()
            .rev()
            .filter(|candidate| *candidate <= BASELINE_TARGET_RETENTION_BASIS_POINTS)
            .find(|candidate| {
                forecast_seconds_per_day(
                    input.forecast_review_occurrences,
                    response_seconds,
                    *candidate,
                    current_target,
                ) <= budget_seconds
            })
            .unwrap_or(MINIMUM_TARGET_RETENTION_BASIS_POINTS)
    }
}

/// Splits a shared unseen-card allowance by stable deck-id round robin.
///
/// The rule is independent of caller ordering, never exceeds the collection
/// allowance, and cannot allocate more cards than a deck has available.
#[must_use]
pub fn allocate_unseen_round_robin(
    candidates: &[DeckIntakeCandidate],
    collection_allowance: u32,
) -> Vec<DeckIntakeAllocation> {
    let mut remaining_by_deck = BTreeMap::<String, u64>::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.unseen_cards > 0)
    {
        let unseen = remaining_by_deck
            .entry(candidate.deck_id.clone())
            .or_default();
        *unseen = unseen.saturating_add(candidate.unseen_cards);
    }
    let mut remaining = remaining_by_deck.into_iter().collect::<Vec<_>>();
    let mut allocations = remaining
        .iter()
        .map(|(deck_id, _)| DeckIntakeAllocation {
            deck_id: deck_id.clone(),
            new_cards: 0,
        })
        .collect::<Vec<_>>();
    let mut allowance = collection_allowance.min(MAXIMUM_NEW_CARDS_PER_DAY);
    while allowance > 0 {
        let mut allocated_this_round = false;
        for (index, (_, unseen_cards)) in remaining.iter_mut().enumerate() {
            if *unseen_cards == 0 || allowance == 0 {
                continue;
            }
            *unseen_cards -= 1;
            allocations[index].new_cards += 1;
            allowance -= 1;
            allocated_this_round = true;
        }
        if !allocated_this_round {
            break;
        }
    }
    allocations
}

fn retention_candidates() -> impl DoubleEndedIterator<Item = u16> {
    (MINIMUM_TARGET_RETENTION_BASIS_POINTS..=MAXIMUM_TARGET_RETENTION_BASIS_POINTS)
        .step_by(usize::from(TARGET_STEP_BASIS_POINTS))
}

fn forecast_seconds_per_day(
    occurrences: u64,
    response_seconds: u64,
    candidate_target: u16,
    current_target: u16,
) -> u64 {
    let scaled_occurrences = multiply_ratio_ceil(
        occurrences,
        u64::from(workload_factor(candidate_target)),
        u64::from(workload_factor(current_target)),
    );
    multiply_ratio_ceil(
        scaled_occurrences,
        response_seconds,
        u64::from(FORECAST_DAYS),
    )
}

fn new_card_daily_cost(response_seconds: u64, target: u16) -> u64 {
    let first_review = response_seconds.saturating_mul(3).div_ceil(2);
    let follow_up_reviews = multiply_ratio_ceil(
        response_seconds.saturating_mul(4),
        u64::from(workload_factor(target)),
        100,
    );
    first_review.saturating_add(follow_up_reviews).max(1)
}

fn multiply_ratio_ceil(value: u64, numerator: u64, denominator: u64) -> u64 {
    let result = u128::from(value)
        .saturating_mul(u128::from(numerator))
        .div_ceil(u128::from(denominator.max(1)));
    u64::try_from(result).unwrap_or(u64::MAX)
}

fn workload_factor(target: u16) -> u16 {
    match (target.saturating_add(50) / 100).clamp(80, 95) {
        80 => 50,
        81 => 53,
        82 => 56,
        83 => 60,
        84 => 64,
        85 => 68,
        86 => 73,
        87 => 79,
        88 => 85,
        89 => 92,
        90 => 100,
        91 => 110,
        92 => 122,
        93 => 138,
        94 => 160,
        _ => 190,
    }
}

fn format_retention(target: u16) -> String {
    if target % 100 == 0 {
        (target / 100).to_string()
    } else {
        format!("{}.{:02}", target / 100, target % 100)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AutomaticPolicyInput, BASELINE_TARGET_RETENTION_BASIS_POINTS, DeckIntakeCandidate,
        MAXIMUM_TARGET_RETENTION_BASIS_POINTS, MINIMUM_TARGET_RETENTION_BASIS_POINTS,
        allocate_unseen_round_robin, automatic_policy,
    };

    fn input(budget: u32, unseen: u64) -> AutomaticPolicyInput {
        AutomaticPolicyInput {
            daily_budget_minutes: budget,
            due_cards_now: 4,
            forecast_review_occurrences: 560,
            response_seconds: 20,
            unseen_cards: unseen,
            current_target_retention_basis_points: 9_000,
            previous_target_retention_basis_points: 9_000,
        }
    }

    #[test]
    fn budget_monotonically_increases_intake_and_retention() {
        let mut previous = automatic_policy(input(1, 1_000));
        for budget in 2..=240 {
            let next = automatic_policy(input(budget, 1_000));
            assert!(next.new_cards_per_day >= previous.new_cards_per_day);
            assert!(next.target_retention_basis_points >= previous.target_retention_basis_points);
            previous = next;
        }
    }

    #[test]
    fn budget_increase_and_reversal_are_deterministic() {
        let constrained = automatic_policy(input(15, 1_000));
        let expanded = automatic_policy(input(120, 1_000));
        let constrained_again = automatic_policy(input(15, 1_000));
        assert_eq!(constrained_again, constrained);
        assert!(expanded.new_cards_per_day >= constrained.new_cards_per_day);
        assert!(
            expanded.target_retention_basis_points >= constrained.target_retention_basis_points
        );
    }

    #[test]
    fn target_is_safe_deterministic_and_intake_falls_first() {
        let generous = automatic_policy(input(60, 1_000));
        let constrained = automatic_policy(input(8, 1_000));
        assert_eq!(generous, automatic_policy(input(60, 1_000)));
        assert_eq!(
            generous.target_retention_basis_points,
            BASELINE_TARGET_RETENTION_BASIS_POINTS
        );
        assert!(generous.new_cards_per_day > 0);
        assert_eq!(constrained.new_cards_per_day, 0);
        assert!(
            (MINIMUM_TARGET_RETENTION_BASIS_POINTS..=MAXIMUM_TARGET_RETENTION_BASIS_POINTS)
                .contains(&constrained.target_retention_basis_points)
        );
    }

    #[test]
    fn due_backlog_is_reported_and_never_subtracted_from_card_counts() {
        let decision = automatic_policy(AutomaticPolicyInput {
            daily_budget_minutes: 1,
            due_cards_now: 100,
            forecast_review_occurrences: 100,
            response_seconds: 20,
            unseen_cards: 100,
            current_target_retention_basis_points: 9_000,
            previous_target_retention_basis_points: 9_000,
        });
        assert!(decision.backlog_exceeds_budget);
        assert_eq!(decision.due_work_seconds, 2_000);
        assert_eq!(decision.new_cards_per_day, 0);
        assert!(
            decision
                .explanation
                .contains("all due reviews remain available")
        );
    }

    #[test]
    fn due_work_consumes_capacity_before_any_unseen_intake() {
        let decision = automatic_policy(AutomaticPolicyInput {
            daily_budget_minutes: 1,
            due_cards_now: 61,
            forecast_review_occurrences: 61,
            response_seconds: 1,
            unseen_cards: 100,
            current_target_retention_basis_points: 9_000,
            previous_target_retention_basis_points: 9_000,
        });
        assert!(decision.backlog_exceeds_budget);
        assert_eq!(decision.new_cards_per_day, 0);
    }

    #[test]
    fn spare_capacity_raises_retention_only_without_unseen_cards() {
        let with_unseen = automatic_policy(input(240, 1));
        let without_unseen = automatic_policy(input(240, 0));
        assert_eq!(
            with_unseen.target_retention_basis_points,
            BASELINE_TARGET_RETENTION_BASIS_POINTS
        );
        assert!(
            without_unseen.target_retention_basis_points > BASELINE_TARGET_RETENTION_BASIS_POINTS
        );
    }

    #[test]
    fn smoothing_moves_at_most_one_percentage_point_per_evaluation() {
        let lowered = automatic_policy(AutomaticPolicyInput {
            previous_target_retention_basis_points: 9_500,
            ..input(1, 1_000)
        });
        assert_eq!(lowered.target_retention_basis_points, 9_400);
        let raised = automatic_policy(AutomaticPolicyInput {
            previous_target_retention_basis_points: 8_000,
            ..input(240, 0)
        });
        assert_eq!(raised.target_retention_basis_points, 8_100);
    }

    #[test]
    fn empty_collection_is_stable_and_explainable() {
        let decision = automatic_policy(AutomaticPolicyInput {
            due_cards_now: 0,
            forecast_review_occurrences: 0,
            unseen_cards: 0,
            ..input(30, 0)
        });
        assert_eq!(decision.new_cards_per_day, 0);
        assert!(!decision.explanation.is_empty());
    }

    #[test]
    fn shared_intake_is_stable_fair_and_bounded() {
        let forward = vec![
            DeckIntakeCandidate {
                deck_id: "deck-b".into(),
                unseen_cards: 10,
            },
            DeckIntakeCandidate {
                deck_id: "deck-a".into(),
                unseen_cards: 2,
            },
        ];
        let reverse = forward.iter().cloned().rev().collect::<Vec<_>>();
        let expected = allocate_unseen_round_robin(&forward, 7);
        assert_eq!(expected, allocate_unseen_round_robin(&reverse, 7));
        assert_eq!(expected[0].deck_id, "deck-a");
        assert_eq!(expected[0].new_cards, 2);
        assert_eq!(expected[1].new_cards, 5);
        assert_eq!(expected.iter().map(|item| item.new_cards).sum::<u32>(), 7);
    }

    #[test]
    fn million_card_aggregate_has_bounded_controller_work() {
        let decision = automatic_policy(AutomaticPolicyInput {
            due_cards_now: 1_000_000,
            forecast_review_occurrences: 28_000_000,
            unseen_cards: 1_000_000,
            ..input(30, 0)
        });
        assert!(decision.backlog_exceeds_budget);
        assert_eq!(decision.new_cards_per_day, 0);
        assert!(
            (MINIMUM_TARGET_RETENTION_BASIS_POINTS..=MAXIMUM_TARGET_RETENTION_BASIS_POINTS)
                .contains(&decision.target_retention_basis_points)
        );
    }

    #[test]
    fn fixed_adversarial_workload_matrix_preserves_controller_invariants() {
        let budgets = [0, 1, 5, 30, 240, 1_440, u32::MAX];
        let counts = [0, 1, 10, 10_000, 1_000_000, u64::MAX];
        let response_seconds = [0, 1, 20, 600, u64::MAX];
        let targets = [0, 8_000, 9_000, 9_500, u16::MAX];

        for budget in budgets {
            for count in counts {
                for response in response_seconds {
                    for target in targets {
                        let input = AutomaticPolicyInput {
                            daily_budget_minutes: budget,
                            due_cards_now: count,
                            forecast_review_occurrences: count,
                            response_seconds: response,
                            unseen_cards: count,
                            current_target_retention_basis_points: target,
                            previous_target_retention_basis_points: target,
                        };
                        let decision = automatic_policy(input);
                        assert_eq!(decision, automatic_policy(input));
                        assert!(
                            (MINIMUM_TARGET_RETENTION_BASIS_POINTS
                                ..=MAXIMUM_TARGET_RETENTION_BASIS_POINTS)
                                .contains(&decision.target_retention_basis_points)
                        );
                        assert!(u64::from(decision.new_cards_per_day) <= count);
                        assert!(decision.new_cards_per_day <= 10_000);
                        assert_eq!(
                            decision.backlog_exceeds_budget,
                            decision.due_work_seconds > u64::from(budget).saturating_mul(60)
                        );
                        if decision.backlog_exceeds_budget {
                            assert_eq!(decision.new_cards_per_day, 0);
                        }
                        assert!(!decision.explanation.is_empty());
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "release performance budget; run with scripts/performance"]
    fn release_budget_one_million_card_aggregate_policy() {
        let started = std::time::Instant::now();
        let decision = automatic_policy(AutomaticPolicyInput {
            due_cards_now: 1_000_000,
            forecast_review_occurrences: 28_000_000,
            unseen_cards: 1_000_000,
            ..input(30, 0)
        });
        let elapsed = started.elapsed();
        assert!(decision.backlog_exceeds_budget);
        assert!(
            elapsed <= std::time::Duration::from_secs(1),
            "one-million-card aggregate policy exceeded 1 s: {elapsed:?}"
        );
        eprintln!(
            "release-budget time_budget_policy_one_million elapsed_us={}",
            elapsed.as_micros()
        );
    }
}
