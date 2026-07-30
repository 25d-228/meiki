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

## Reference fixtures

`fixtures/fsrs7-reference.json` contains more than one hundred deterministic
vectors generated from the scalar equations in the pinned reference file. The
matrix covers every first and subsequent grade, same-minute, same-hour,
same-day, normal, overdue, and very long elapsed times, six target retentions,
the default parameters, a valid non-default vector, repeated lapses, and mixed
long histories.

Regenerate it only when making an explicit engine-version decision:

```sh
./scripts/dev-env python crates/meiki-scheduler/fixtures/generate_fsrs7_reference.py
```

The generator records reference commit
`70cc4387f573ff20b13ac9c106333a335c8a4cb8` in the fixture. It evaluates the
reference equations with IEEE-754 binary64 scalar arithmetic, rounds persisted
integer fields to the nearest unit, and records recall probabilities with an
accepted absolute tolerance of `1e-12`. Normal builds and CI consume the
committed JSON and require neither Python nor network access.

The smaller `pinned_reference_vector_matches_fsrs7_model` test also fixes the
following transitions for readable review:

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

## Memory model and workload policy

FSRS-7 parameters describe memory. The `time-budget-v1` controller is a
separate scheduling policy and never fits or invents those parameters.
Automatic mode evaluates aggregate schedule state over a rolling 28-day
horizon. It uses actual due timestamps, current intervals, a bounded median
response time, and an explicit first-review cost.

The controller starts from 90% target retention within a safe 80–95% range.
When projected work exceeds the daily budget, it first reduces new intake to
zero and only then lowers retention in one-percentage-point steps. Spare time
adds unseen cards before raising retention above 90%; it raises retention only
when no unseen cards remain. Existing due cards are always visible, including
when their estimated work exceeds the budget.

Results are deterministic and recomputed at a local-day transition, after
material history or unseen-card changes, or when settings change. Multiple
decks sharing an allowance receive unseen cards in stable deck-ID round-robin
order. The aggregate controller has constant memory use; it does not load card
content or construct a million-card candidate vector.

Expert mode allows manual target retention, new-card maximum, and maximum
interval. It also supports strict versioned import/export of memory parameter
sets. Parameter adoption is prospective and never changes existing schedule
projections or immutable review history.
