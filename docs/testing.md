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
| Restart finds a pending review request                                    | Playwright seeds one serialized UI queue fixture and verifies that the identical command is replayed; exactly-once persistence remains a Rust invariant |

All filesystem faults are confined to temporary directories. The only explicit
fault API is compiled for storage tests and fixture builds; it cannot be
enabled through production runtime state.

## Continuous integration

Linux quality CI runs the full real journey, all crate tests, desktop command
contracts, frontend checks, and Chromium UI tests. macOS and Windows run the
core Rust journeys, desktop command contracts, generated-contract check, and
frontend production build. The package workflow builds each platform bundle
and launches its executable; Linux uses Xvfb. Failed browser runs upload
Playwright traces and test results, while failed package smokes upload the
built bundle for inspection.
