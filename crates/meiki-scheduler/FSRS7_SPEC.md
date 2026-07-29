# Pinned FSRS-7 model

Meiki's initial scheduling engine is identified as `fsrs-7`.

The equations and 35 bundled population parameters are pinned to the
Open Spaced Repetition `srs-benchmark` reference model at commit
[`70cc4387f573ff20b13ac9c106333a335c8a4cb8`](https://github.com/open-spaced-repetition/srs-benchmark/blob/70cc4387f573ff20b13ac9c106333a335c8a4cb8/models/fsrs_v7.py).

The implementation in this crate is independent Rust code. It retains the
reference model's:

- four initial-stability parameters;
- difficulty initialization, damping, and mean reversion;
- long-term and short-term stability updates;
- exponential transition between short- and long-term memory;
- two-component power-law forgetting curve; and
- 35-parameter ordering.

Meiki solves the target-retention interval at fractional-day precision and
stores the resulting timestamp in milliseconds. It deliberately does not add
random interval fuzzing, so identical inputs always produce identical
decisions. Queue placement initially equals ideal due time; the Today planner
may later place cards without changing the engine's ideal timestamp.

Any formula or default-vector change requires a new engine identifier and new
reference fixtures. Existing immutable review events keep their recorded
engine and parameter-set identifiers.

## Reference fixture

The `pinned_reference_vector_matches_fsrs7_model` test fixes the following
double-precision transitions from the pinned equations:

| Transition                   |   Stability (days) |        Difficulty |
| ---------------------------- | -----------------: | ----------------: |
| First `Good`                 |             4.1283 | 4.194588083372719 |
| `Good` after 0.25 day        | 7.2739410326214955 | 4.180821488255665 |
| `Again` after 7 days         | 1.3698820245809902 | 8.343267826257986 |
| `Hard` at the same timestamp | 1.3698820245809902 | 8.882483072294189 |
| `Easy` after 120 days        |  8.413020800712248 |  8.42085010328852 |

At 90% target retention, a 4.1283-day stability produces a
256,342,507-millisecond interval. The executable fixture also verifies this
fractional interval.

## Personalization

Local optimization starts after 64 useful reviews (reviews after a card's
first grade). Candidate parameters are selected on the chronological first 80%
of history and adopted only when they remain valid and reduce log loss on the
held-out final 20%. Adoption is prospective: it changes the active parameter
set without changing existing projections. A full replay is a separate,
backup-first operation.
