//! Pure scheduling boundary for the foundation walking skeleton.

use meiki_domain::{Grade, ScheduleState};

pub const ENGINE_VERSION: &str = "foundation-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleDecision {
    pub scheduler_version: &'static str,
    pub next_state: ScheduleState,
}

/// Calculates the next schedule state without I/O or mutable shared state.
pub fn schedule_review(
    current: &ScheduleState,
    grade: Grade,
    reviewed_at_ms: i64,
) -> ScheduleDecision {
    let (interval_seconds, repetitions) = match grade {
        Grade::Again => (60, 0),
        Grade::Hard => (
            86_400 * u64::from(current.repetitions.max(1)),
            current.repetitions + 1,
        ),
        Grade::Good => (
            259_200 * u64::from(current.repetitions.max(1)),
            current.repetitions + 1,
        ),
        Grade::Easy => (
            604_800 * u64::from(current.repetitions.max(1)),
            current.repetitions + 1,
        ),
    };
    let interval_ms = i64::try_from(interval_seconds)
        .unwrap_or(i64::MAX)
        .saturating_mul(1_000);

    ScheduleDecision {
        scheduler_version: ENGINE_VERSION,
        next_state: ScheduleState {
            card_id: current.card_id.clone(),
            version: current.version + 1,
            due_at_ms: reviewed_at_ms.saturating_add(interval_ms),
            interval_seconds,
            repetitions,
            last_review_event_id: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use meiki_domain::{Grade, ScheduleState};

    use super::{ENGINE_VERSION, schedule_review};

    fn initial_state() -> ScheduleState {
        ScheduleState {
            card_id: "card-1".into(),
            version: 0,
            due_at_ms: 1_000,
            interval_seconds: 0,
            repetitions: 0,
            last_review_event_id: None,
        }
    }

    #[test]
    fn identical_inputs_produce_identical_decisions() {
        let first = schedule_review(&initial_state(), Grade::Good, 10_000);
        let second = schedule_review(&initial_state(), Grade::Good, 10_000);
        assert_eq!(first, second);
        assert_eq!(first.scheduler_version, ENGINE_VERSION);
    }

    #[test]
    fn again_schedules_a_retry_and_resets_repetitions() {
        let decision = schedule_review(&initial_state(), Grade::Again, 10_000);
        assert_eq!(decision.next_state.due_at_ms, 70_000);
        assert_eq!(decision.next_state.repetitions, 0);
    }
}
