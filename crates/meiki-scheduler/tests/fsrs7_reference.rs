use std::collections::HashMap;

use meiki_domain::Grade;
use meiki_scheduler::{Fsrs7Engine, SchedulerConfig, SchedulerEngine};
use serde::Deserialize;

const FIXTURE: &str = include_str!("../fixtures/fsrs7-reference.json");
const EXPECTED_REFERENCE_COMMIT: &str = "70cc4387f573ff20b13ac9c106333a335c8a4cb8";
const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Deserialize)]
struct ReferenceFixture {
    reference_commit: String,
    recall_tolerance: f64,
    parameter_sets: HashMap<String, Vec<f64>>,
    cases: Vec<ReferenceCase>,
}

#[derive(Debug, Deserialize)]
struct ReferenceCase {
    name: String,
    parameters: String,
    target_retention_basis_points: u16,
    steps: Vec<ReferenceStep>,
}

#[derive(Debug, Deserialize)]
struct ReferenceStep {
    grade: String,
    elapsed_ms: i64,
    reviewed_at_ms: i64,
    stability_milliseconds: u64,
    difficulty_millipoints: u32,
    interval_milliseconds: u64,
    due_at_ms: i64,
    repetitions: u32,
    recall_after_30_days: f64,
}

fn grade(value: &str) -> Grade {
    match value {
        "again" => Grade::Again,
        "hard" => Grade::Hard,
        "good" => Grade::Good,
        "easy" => Grade::Easy,
        other => panic!("unknown fixture grade {other}"),
    }
}

#[test]
fn committed_reference_matrix_matches_the_pinned_fsrs7_equations() {
    let fixture: ReferenceFixture =
        serde_json::from_str(FIXTURE).expect("the committed reference fixture is valid JSON");
    assert_eq!(fixture.reference_commit, EXPECTED_REFERENCE_COMMIT);
    assert!(
        fixture.cases.len() >= 100,
        "the differential fixture must remain broad"
    );

    for case in fixture.cases {
        let parameters = fixture
            .parameter_sets
            .get(&case.parameters)
            .unwrap_or_else(|| panic!("missing parameter set for {}", case.name));
        let engine = Fsrs7Engine::from_parameters(
            SchedulerConfig {
                target_retention_basis_points: case.target_retention_basis_points,
                maximum_interval_days: 36_500,
            },
            parameters,
        )
        .unwrap_or_else(|error| panic!("invalid fixture {}: {error}", case.name));
        let mut state = engine.initial_schedule(&case.name, 0);
        let mut elapsed_total = 0_i64;
        for (index, expected) in case.steps.iter().enumerate() {
            elapsed_total = elapsed_total
                .checked_add(expected.elapsed_ms)
                .expect("fixture elapsed time remains supported");
            assert_eq!(elapsed_total, expected.reviewed_at_ms, "{}", case.name);
            state = engine
                .review(&state, grade(&expected.grade), expected.reviewed_at_ms)
                .unwrap_or_else(|error| {
                    panic!("fixture {} step {index} failed: {error}", case.name)
                })
                .next_state;
            assert_eq!(
                state.stability_milliseconds, expected.stability_milliseconds,
                "{} step {index} stability",
                case.name
            );
            assert_eq!(
                state.difficulty_millipoints, expected.difficulty_millipoints,
                "{} step {index} difficulty",
                case.name
            );
            assert_eq!(
                state.interval_milliseconds, expected.interval_milliseconds,
                "{} step {index} interval",
                case.name
            );
            assert_eq!(
                state.due_at_ms, expected.due_at_ms,
                "{} step {index} due timestamp",
                case.name
            );
            assert_eq!(
                state.repetitions, expected.repetitions,
                "{} step {index} repetitions",
                case.name
            );
            let recall = engine
                .recall_probability(&state, expected.reviewed_at_ms + 30 * DAY_MS)
                .unwrap();
            assert!(
                (recall - expected.recall_after_30_days).abs() <= fixture.recall_tolerance,
                "{} step {index} recall: expected {:.15}, found {recall:.15}",
                case.name,
                expected.recall_after_30_days
            );
        }
    }
}

#[test]
fn scheduler_properties_hold_across_fixed_generated_states() {
    let retentions = [7_000, 8_000, 8_500, 9_000, 9_500, 9_900];
    let elapsed = [
        0,
        60_000,
        60 * 60 * 1_000,
        12 * 60 * 60 * 1_000,
        DAY_MS,
        30 * DAY_MS,
        365 * DAY_MS,
        20_000 * DAY_MS,
    ];
    let grades = [Grade::Again, Grade::Hard, Grade::Good, Grade::Easy];

    for first_grade in grades {
        let default = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        let first = default
            .review(
                &default.initial_schedule("property-card", 0),
                first_grade,
                0,
            )
            .unwrap()
            .next_state;

        let mut prior_recall = 1.0;
        for at_ms in elapsed {
            let recall = default.recall_probability(&first, at_ms).unwrap();
            assert!(recall.is_finite() && (0.0..=1.0).contains(&recall));
            assert!(recall <= prior_recall);
            prior_recall = recall;
        }

        let serialized = default.serialize_state(&first);
        assert_eq!(default.deserialize_state(&serialized).unwrap(), first);

        for elapsed_ms in elapsed {
            let at_ms = elapsed_ms;
            let decisions = grades.map(|next_grade| {
                default
                    .review(&first, next_grade, at_ms)
                    .unwrap()
                    .next_state
            });
            assert!(
                decisions[1].interval_milliseconds <= decisions[2].interval_milliseconds,
                "Hard scheduled after Good for {first_grade:?} at {elapsed_ms}"
            );
            assert!(
                decisions[2].interval_milliseconds <= decisions[3].interval_milliseconds,
                "Good scheduled after Easy for {first_grade:?} at {elapsed_ms}"
            );
        }

        let intervals = retentions.map(|target| {
            Fsrs7Engine::new(SchedulerConfig {
                target_retention_basis_points: target,
                maximum_interval_days: 36_500,
            })
            .unwrap()
            .review(
                &Fsrs7Engine::new(SchedulerConfig {
                    target_retention_basis_points: target,
                    maximum_interval_days: 36_500,
                })
                .unwrap()
                .initial_schedule("retention-card", 0),
                first_grade,
                0,
            )
            .unwrap()
            .next_state
            .interval_milliseconds
        });
        assert!(
            intervals.windows(2).all(|pair| pair[1] <= pair[0]),
            "higher target retention lengthened an interval: {intervals:?}"
        );
    }
}

#[test]
fn malformed_parameters_and_extreme_timestamps_fail_safely() {
    let config = SchedulerConfig::default();
    let default = meiki_scheduler::DEFAULT_PARAMETERS;
    for invalid in [Vec::new(), default[..default.len() - 1].to_vec(), {
        let mut oversized = default.to_vec();
        oversized.push(1.0);
        oversized
    }] {
        assert!(Fsrs7Engine::from_parameters(config, &invalid).is_err());
    }
    for invalid_value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut invalid = default;
        invalid[7] = invalid_value;
        assert!(Fsrs7Engine::from_parameters(config, &invalid).is_err());
    }
    let mut unordered = default;
    unordered[1] = unordered[0] / 2.0;
    assert!(Fsrs7Engine::from_parameters(config, &unordered).is_err());

    let engine = Fsrs7Engine::new(config).unwrap();
    let first = engine
        .review(
            &engine.initial_schedule("extreme-card", i64::MIN),
            Grade::Good,
            i64::MIN,
        )
        .unwrap()
        .next_state;
    assert!(first.due_at_ms > i64::MIN);
    assert!(engine.review(&first, Grade::Good, i64::MAX).is_err());
    let near_max = engine
        .review(
            &engine.initial_schedule("saturating-card", i64::MAX - 1),
            Grade::Easy,
            i64::MAX - 1,
        )
        .unwrap()
        .next_state;
    assert_eq!(near_max.due_at_ms, i64::MAX);
}
