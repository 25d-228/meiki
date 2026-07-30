use std::collections::{HashMap, HashSet};

use meiki_domain::Grade;

use crate::{Fsrs7Engine, PARAMETER_COUNT, fsrs7::forgetting_curve};

/// Minimum number of reviews after a card's first grade before training starts.
pub const MINIMUM_OPTIMIZATION_REVIEWS: usize = 64;
const HOLDOUT_PERCENT: usize = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewHistoryEntry {
    pub card_id: String,
    pub reviewed_at_ms: i64,
    pub grade: Grade,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OptimizationDiagnostics {
    pub reviews: usize,
    pub training_reviews: usize,
    pub holdout_reviews: usize,
    pub current_holdout_loss: f64,
    pub candidate_holdout_loss: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OptimizationResult {
    InsufficientData {
        reviews: usize,
        minimum: usize,
    },
    Adopted {
        parameters: Box<[f64; PARAMETER_COUNT]>,
        diagnostics: OptimizationDiagnostics,
    },
    Rejected {
        diagnostics: OptimizationDiagnostics,
    },
    Failed {
        reason: &'static str,
    },
}

#[derive(Clone, Copy, Debug)]
struct SimulatedMemory {
    stability_days: f64,
    difficulty: f64,
    reviewed_at_ms: i64,
}

pub(crate) fn optimize(engine: &Fsrs7Engine, history: &[ReviewHistoryEntry]) -> OptimizationResult {
    let mut chronological = history.to_vec();
    chronological.sort_by(|left, right| {
        left.reviewed_at_ms
            .cmp(&right.reviewed_at_ms)
            .then_with(|| left.card_id.cmp(&right.card_id))
    });
    let mut seen_cards = HashSet::new();
    let useful_reviews = chronological
        .iter()
        .filter(|review| !seen_cards.insert(review.card_id.as_str()))
        .count();
    if useful_reviews < MINIMUM_OPTIMIZATION_REVIEWS {
        return OptimizationResult::InsufficientData {
            reviews: useful_reviews,
            minimum: MINIMUM_OPTIMIZATION_REVIEWS,
        };
    }
    let holdout_count = chronological.len() * HOLDOUT_PERCENT / 100;
    let training_count = chronological.len().saturating_sub(holdout_count);
    if holdout_count == 0 || training_count == 0 {
        return OptimizationResult::Failed {
            reason: "chronological split produced an empty partition",
        };
    }

    let candidates = candidates(engine.parameters());
    let Some((_, best_parameters)) = candidates
        .iter()
        .filter(|parameters| {
            Fsrs7Engine::from_parameters(engine.config(), parameters.as_slice()).is_ok()
        })
        .filter_map(|parameters| {
            score(parameters, &chronological, 0, training_count).map(|loss| (loss, *parameters))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
    else {
        return OptimizationResult::Failed {
            reason: "training history produced no predictable reviews",
        };
    };
    let Some(current_holdout_loss) = score(
        engine.parameters(),
        &chronological,
        training_count,
        chronological.len(),
    ) else {
        return OptimizationResult::Failed {
            reason: "holdout history produced no predictable reviews",
        };
    };
    let Some(candidate_holdout_loss) = score(
        &best_parameters,
        &chronological,
        training_count,
        chronological.len(),
    ) else {
        return OptimizationResult::Failed {
            reason: "candidate produced no holdout predictions",
        };
    };
    let diagnostics = OptimizationDiagnostics {
        reviews: chronological.len(),
        training_reviews: training_count,
        holdout_reviews: holdout_count,
        current_holdout_loss,
        candidate_holdout_loss,
    };
    if !parameters_equal(&best_parameters, engine.parameters())
        && candidate_holdout_loss + 1e-6 < current_holdout_loss
        && best_parameters.iter().all(|value| value.is_finite())
    {
        OptimizationResult::Adopted {
            parameters: Box::new(best_parameters),
            diagnostics,
        }
    } else {
        OptimizationResult::Rejected { diagnostics }
    }
}

fn candidates(current: &[f64; PARAMETER_COUNT]) -> Vec<[f64; PARAMETER_COUNT]> {
    let mut candidates = vec![*current];
    for initial_scale in [0.85, 1.15] {
        let mut candidate = *current;
        for value in &mut candidate[0..4] {
            *value = (*value * initial_scale).clamp(0.0001, 36_500.0);
        }
        candidates.push(candidate);
    }
    for (index, delta) in [(7, -0.15), (7, 0.15), (16, -0.15), (16, 0.15)] {
        let mut candidate = *current;
        candidate[index] = (candidate[index] + delta).max(0.0);
        candidates.push(candidate);
    }
    candidates
}

fn score(
    parameters: &[f64; PARAMETER_COUNT],
    history: &[ReviewHistoryEntry],
    score_start: usize,
    score_end: usize,
) -> Option<f64> {
    let mut memory_by_card = HashMap::<&str, SimulatedMemory>::new();
    let mut loss = 0.0;
    let mut predictions = 0usize;
    for (index, review) in history.iter().enumerate() {
        let previous = memory_by_card.get(review.card_id.as_str()).copied();
        if let Some(previous) = previous {
            let elapsed_days = elapsed_days(review.reviewed_at_ms, previous.reviewed_at_ms);
            let probability = forgetting_curve(parameters, elapsed_days, previous.stability_days)
                .clamp(0.0001, 0.9999);
            if (score_start..score_end).contains(&index) {
                let recalled = review.grade != Grade::Again;
                loss -= if recalled {
                    probability.ln()
                } else {
                    (1.0 - probability).ln()
                };
                predictions += 1;
            }
        }
        memory_by_card.insert(
            review.card_id.as_str(),
            update_memory(parameters, previous, review),
        );
    }
    let predictions = u32::try_from(predictions).ok()?;
    (predictions > 0).then_some(loss / f64::from(predictions))
}

fn update_memory(
    parameters: &[f64; PARAMETER_COUNT],
    previous: Option<SimulatedMemory>,
    review: &ReviewHistoryEntry,
) -> SimulatedMemory {
    let rating: u8 = match review.grade {
        Grade::Again => 1,
        Grade::Hard => 2,
        Grade::Good => 3,
        Grade::Easy => 4,
    };
    let Some(previous) = previous else {
        return SimulatedMemory {
            stability_days: parameters[usize::from(rating - 1)],
            difficulty: initial_difficulty(parameters, rating),
            reviewed_at_ms: review.reviewed_at_ms,
        };
    };
    let elapsed_days = elapsed_days(review.reviewed_at_ms, previous.reviewed_at_ms);
    let retrievability = forgetting_curve(parameters, elapsed_days, previous.stability_days);
    let long_term = stability_after_review(parameters, previous, retrievability, rating, 7);
    let short_term = stability_after_review(parameters, previous, retrievability, rating, 16);
    let transition =
        (1.0 - parameters[26] * (-parameters[25] * elapsed_days).exp()).clamp(0.0, 1.0);
    let delta = -parameters[6] * (f64::from(rating) - 3.0);
    let damped = delta * (10.0 - previous.difficulty) / 9.0;
    let difficulty = (0.01 * initial_difficulty(parameters, 4)
        + 0.99 * (previous.difficulty + damped))
        .clamp(1.0, 10.0);
    SimulatedMemory {
        stability_days: (transition * long_term + (1.0 - transition) * short_term)
            .clamp(0.0001, 36_500.0),
        difficulty,
        reviewed_at_ms: review.reviewed_at_ms,
    }
}

fn initial_difficulty(parameters: &[f64; PARAMETER_COUNT], rating: u8) -> f64 {
    (parameters[4] - (parameters[5] * (f64::from(rating) - 1.0)).exp() + 1.0).clamp(1.0, 10.0)
}

fn stability_after_review(
    parameters: &[f64; PARAMETER_COUNT],
    previous: SimulatedMemory,
    retrievability: f64,
    rating: u8,
    base: usize,
) -> f64 {
    let failed = parameters[base + 3]
        * previous.difficulty.powf(-parameters[base + 4])
        * ((previous.stability_days + 1.0).powf(parameters[base + 5]) - 1.0)
        * ((1.0 - retrievability) * parameters[base + 6]).exp();
    let post_lapse = previous.stability_days.min(failed);
    if rating == 1 {
        return post_lapse;
    }
    let hard = if rating == 2 {
        parameters[base + 7]
    } else {
        1.0
    };
    let easy = if rating == 4 {
        parameters[base + 8]
    } else {
        1.0
    };
    let increase = 1.0
        + (parameters[base] - 1.5).exp()
            * (11.0 - previous.difficulty)
            * previous.stability_days.powf(-parameters[base + 1])
            * (((1.0 - retrievability) * parameters[base + 2]).exp() - 1.0)
            * hard
            * easy;
    post_lapse.max(previous.stability_days * increase)
}

fn elapsed_days(later_ms: i64, earlier_ms: i64) -> f64 {
    let milliseconds = later_ms.saturating_sub(earlier_ms).max(0);
    let milliseconds = u64::try_from(milliseconds).unwrap_or(u64::MAX);
    std::time::Duration::from_millis(milliseconds).as_secs_f64() / 86_400.0
}

fn parameters_equal(left: &[f64; PARAMETER_COUNT], right: &[f64; PARAMETER_COUNT]) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| left.to_bits() == right.to_bits())
}

#[cfg(test)]
mod tests {
    use meiki_domain::Grade;

    use crate::{DEFAULT_PARAMETERS, Fsrs7Engine, SchedulerConfig, SchedulerEngine};

    use super::{MINIMUM_OPTIMIZATION_REVIEWS, OptimizationResult, ReviewHistoryEntry};

    #[test]
    fn insufficient_history_never_changes_parameters() {
        let engine = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        assert_eq!(
            engine.optimize(&[]),
            OptimizationResult::InsufficientData {
                reviews: 0,
                minimum: MINIMUM_OPTIMIZATION_REVIEWS,
            }
        );
    }

    #[test]
    fn optimization_is_deterministic_and_holdout_validated() {
        let engine = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        let mut history = Vec::new();
        for card in 0..8 {
            let mut reviewed_at_ms = 1_000;
            for review in 0..12 {
                reviewed_at_ms += if review < 2 {
                    600_000
                } else {
                    86_400_000 * i64::from(review)
                };
                history.push(ReviewHistoryEntry {
                    card_id: format!("card-{card}"),
                    reviewed_at_ms,
                    grade: if review % 5 == 4 {
                        Grade::Again
                    } else {
                        Grade::Good
                    },
                });
            }
        }
        let first = engine.optimize(&history);
        let second = engine.optimize(&history);
        assert_eq!(first, second);
        assert!(matches!(
            first,
            OptimizationResult::Adopted { .. } | OptimizationResult::Rejected { .. }
        ));
    }

    #[test]
    fn chronological_holdout_adopts_a_known_improvement() {
        let mut inflated = DEFAULT_PARAMETERS;
        for value in &mut inflated[0..4] {
            *value *= 1.15;
        }
        let engine = Fsrs7Engine::from_parameters(SchedulerConfig::default(), &inflated).unwrap();
        let mut history = Vec::new();
        for card in 0..100 {
            history.push(ReviewHistoryEntry {
                card_id: format!("card-{card:03}"),
                reviewed_at_ms: 0,
                grade: Grade::Good,
            });
        }
        for card in 0..100 {
            history.push(ReviewHistoryEntry {
                card_id: format!("card-{card:03}"),
                reviewed_at_ms: 256_342_507,
                grade: if card % 10 == 0 {
                    Grade::Again
                } else {
                    Grade::Good
                },
            });
        }

        let OptimizationResult::Adopted {
            parameters,
            diagnostics,
        } = engine.optimize(&history)
        else {
            panic!("the holdout-validated population fixture should be adopted");
        };
        assert!(diagnostics.candidate_holdout_loss < diagnostics.current_holdout_loss);
        assert!((parameters[0] - inflated[0] * 0.85).abs() < f64::EPSILON);
        assert_eq!(diagnostics.training_reviews, 160);
        assert_eq!(diagnostics.holdout_reviews, 40);
    }

    #[test]
    #[ignore = "release performance budget; run with scripts/performance"]
    fn release_budget_multi_year_optimizer_history() {
        let engine = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        let mut history = Vec::with_capacity(50_000);
        for review in 0..500 {
            for card in 0..100 {
                history.push(ReviewHistoryEntry {
                    card_id: format!("card-{card:03}"),
                    reviewed_at_ms: i64::from(review) * 3 * 86_400_000 + i64::from(card),
                    grade: if (review + card) % 13 == 0 {
                        Grade::Again
                    } else {
                        Grade::Good
                    },
                });
            }
        }

        let started = std::time::Instant::now();
        let result = engine.optimize(&history);
        let elapsed = started.elapsed();

        assert!(matches!(
            result,
            OptimizationResult::Adopted { .. } | OptimizationResult::Rejected { .. }
        ));
        assert!(
            elapsed <= std::time::Duration::from_secs(10),
            "50,000-review optimizer run exceeded 10 s: {elapsed:?}"
        );
        eprintln!(
            "release-budget optimizer_50000_reviews elapsed_ms={}",
            elapsed.as_millis()
        );
    }
}
