# Test architecture

Meiki tests the production boundaries where behavior lives. Run development
commands through `./scripts/dev-env`; the complete local gate is:

```sh
./scripts/dev-env npm run verify
./scripts/dev-env npm run performance
```

## Layers

| Layer                                      | Responsibility                                                                                                        |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| Crate unit and property tests              | Text normalization and graphemes, FSRS projections, storage transactions, media integrity, and archive validation     |
| `meiki-application/tests/real_journeys.rs` | Public `ApplicationService` journeys through real SQLite, scheduler, text, media, backup, and archive boundaries      |
| `meiki-desktop` library tests              | Plain command argument mapping, DTO serialization, display-error mapping, and Tauri handler registration completeness |
| Playwright                                 | UI request wiring, rendering, keyboard/IME behavior, bidi, accessibility, responsive layout, and visual snapshots     |
| `scripts/package-launch-smoke.py`          | The built platform executable opens its declared main window and initializes a clean local collection on Today        |

The application journeys use only three narrow runtime inputs: the collection
path, a clock, and an ID source. Production defaults remain the operating-system
app-data path, wall-clock time, and random UUIDs. Tests inject fixed time,
sequential IDs, and a temporary collection path. There is no service locator,
dependency-injection container, or repository mock.

The package smoke sets `MEIKI_DATA_DIR` to a temporary directory before launch.
Normal launches leave it unset and use Tauri's operating-system app-data path.

The Tauri macros call plain Rust command functions, and those functions call
`ApplicationService`. Command tests use a real temporary collection. The
browser adapter in `e2e/support/mock-api.ts` records command arguments and
returns predefined DTO or error scenarios from `scenario-dtos.ts`. It must not
schedule cards, reconcile queues, compare answers, mutate review history,
search notes, build archives, manage backups, or emulate application
controllers.

## Deterministic adversarial matrix

The scheduler differential suite consumes
`crates/meiki-scheduler/fixtures/fsrs7-reference.json`. The fixture is generated
from the pinned FSRS-7 reference commit recorded in
[FSRS7_SPEC.md](../crates/meiki-scheduler/FSRS7_SPEC.md); normal CI does not run
Python or access the network. Regenerate it only with the documented command and
treat any changed vector as an explicit scheduler engine-version decision.

The bounded merge-blocking suite also includes:

- a review/lifecycle command model that checks immutable event history,
  idempotency, undo, suspension, trash, restart, and projection repair after
  every step;
- UTC, Tokyo, New York DST, half-hour, and 45-minute local-calendar browser
  contexts with a fixed clock;
- the released schema-7 fixture, legacy archive versions, WAL termination
  recovery, concurrent stale writes, and database constraint failures;
- hostile Unicode boundaries, comparison policies, IME event ordering, media
  signatures, raw ZIP names, aggregate decompression limits, and archive
  checksums;
- axe on every primary screen and every core loading, empty, error, stale,
  destructive, and success state in light and dark themes;
- visual baselines for those core states, required scripts, responsive layouts,
  and both themes.

Property tests use fixed strategies and bounded inputs. Release performance
fixtures are ignored by normal `cargo test` and run serially through
`./scripts/performance`.

## Failure matrix

| Failure boundary                                                          | Executable evidence                                                                                                                                     |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Review transaction fails after writes but before commit                   | `injected_failure_before_review_commit_rolls_back_every_write` uses the storage crate's single bounded, test-only fault                                 |
| Response is lost after a committed review                                 | `response_loss_retry_is_exactly_once_and_undo_is_compensating` discards the first result and retries the same event ID                                  |
| Stale content or schedule version                                         | `corrupt_archive_and_stale_review_fail_without_partial_state` proves no second event is appended                                                        |
| Filesystem media write fails after recovery backup but before replacement | `media_write_failure_after_backup_does_not_replace_the_database` blocks the temporary target object directory                                           |
| Recovery backup cannot be created                                         | `backup_failure_leaves_the_live_collection_unchanged` blocks the temporary backup directory                                                             |
| Media object is missing or corrupt                                        | `missing_and_corrupt_media_are_reported_without_blocking_the_card` uses the real content-addressed object                                               |
| Archive bytes or media checksums are corrupt                              | Application and `meiki-portable` tests reject them before live replacement                                                                              |
| ZIP duplicate names, unsafe paths, invalid UTF-8, or decompression bombs  | Raw central-directory validation and aggregate/ratio limits reject the archive before collection parsing or media extraction                            |
| Restart finds a pending review request                                    | Playwright seeds one serialized UI queue fixture and verifies that the identical command is replayed; exactly-once persistence remains a Rust invariant |

All filesystem faults are confined to temporary directories. The only explicit
fault API is compiled for storage tests and fixture builds; it cannot be
enabled through production runtime state.

## Native input checklist

Before a release, record this small native pass in the release pull request
(issue #43 owns the v0.2 execution):

- [ ] Record the operating system, input method, language, and date.
- [ ] With a Japanese, Chinese, or Korean IME, exercise composition
      start/update/end in Study and Add / Edit.
- [ ] Press Enter while composing and confirm that no answer or form submits;
      press Enter after composition ends and confirm one submission.
- [ ] Edit combining text and an emoji ZWJ sequence without splitting a visible
      character.
- [ ] With an Arabic, Hebrew, or Persian input method, enter mixed RTL text,
      numbers, and punctuation; confirm that learning content is isolated while
      navigation and action order remain LTR.
- [ ] Link every defect found to a regression test at the lowest meaningful
      boundary.

Do not mark this checklist complete from synthetic composition events or
browser automation.

## Continuous integration

| Lane                      | Coverage                                                                                                                                                                                                |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pull request              | Formatting, lint, architecture boundaries, generated contracts, frontend build, all bounded Rust/property/differential/migration/integration tests, and Chromium interaction/accessibility/visual tests |
| macOS and Windows         | Core Rust integration journeys, desktop contracts, generated contracts, and frontend production build                                                                                                   |
| Main                      | The pull-request lanes plus serial storage-backed release budgets                                                                                                                                       |
| Tag or manual package run | Platform bundles, full Rust validation, archive/restore journey, and packaged executable launch smoke                                                                                                   |

The storage-backed release fixture contains 15,000 cards and mixed-script notes
across two real decks. It times current-schema open, exact-due all-decks Today
construction, automatic controller evaluation, and Library search through
`ApplicationService`. A separate 10,000-card schema-8 fixture times backup and
migration, and a 5,000-note `.meiki` fixture times export and validation.
Fixture construction is outside the measured interval; every result prints
fixture bytes and elapsed time.

Failed browser runs upload Playwright traces and test results, while failed
package smokes upload the built bundle for inspection. The packaged launch
smoke prints the elapsed time to creation of the clean collection.
