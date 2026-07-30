# Release quality

This document defines the maintained gate for every Meiki release. Current
release: **0.2.0**. A tag is a release candidate only after the normal
verification suite, the performance suite, and the package workflow pass for
the tagged commit.

## Supported matrix

| Boundary                                                       | Required coverage                                                                                                                 |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Rust domain, text, scheduler, storage, media, and archive code | Unit, property, fixture, transaction, and integration tests on Linux, macOS, and Windows                                          |
| Desktop frontend                                               | Strict TypeScript check and production build on Linux, macOS, and Windows                                                         |
| Browser behavior                                               | Chromium UI-contract suite on Linux, including request mapping, keyboard, IME, bidi, accessibility, responsive, and visual checks |
| Packages                                                       | Tauri bundle build and executable launch smoke on Linux, macOS, and Windows from a version tag or manual release run              |

Rust 1.85, Node.js 24, `Cargo.lock`, and `package-lock.json` are pinned inputs.
The package workflow produces SHA-256 sums and GitHub build provenance. Native
Apple and Windows code signing is required when signing credentials are
available to the repository; otherwise the workflow output is an explicitly
unsigned release candidate and must not be presented as a trusted public
installer.

## Release-blocking scenarios

The release journey must work without an account or content network:

1. Start a clean local collection.
2. Create multilingual content and a grapheme-safe cloze.
3. Study with a keyboard or IME.
4. Reveal, grade, undo, and resume after restart.
5. Export a complete `.meiki` archive.
6. Validate and restore it into a clean collection.
7. Continue review when media is missing, unsupported, or corrupt.

The real `ApplicationService` journeys execute this release path through
SQLite, text, scheduler, media, backup, and archive production code. The
browser suite covers UI request mapping and rendering for Japanese IME,
Arabic/Persian RTL, Devanagari combining text, Latin diacritics, CJK without
spaces, mixed direction, mixed script and punctuation, and multi-code-point
emoji. It uses static DTO scenarios and does not reimplement business rules.
The detailed layer and failure matrix is in
[test architecture](testing.md).

An open defect blocks release when it can cause data loss, an incorrect or
duplicated review commit, accidental IME submission, unreadable bidi content,
or a scheduler invariant failure. These are P0/P1 defects regardless of their
UI severity. Confirm that no such issue is open before creating a public
release.

## Automated interface-quality gate

Every primary screen is keyboard operable and is audited against automated
WCAG 2.0 A/AA and WCAG 2.1 AA rules. The suite also checks:

- skip navigation and intentional focus transfer;
- labelled controls and dialog names;
- explicit status and error text;
- visible focus and 4.5:1 normal-text contrast in light and dark themes;
- reduced motion;
- Dialog, AlertDialog, and Sheet focus trapping and restoration;
- IME-safe Enter handling;
- isolated RTL learning content without reversing interface controls;
- 200% zoom-equivalent reflow and the 640-pixel minimum layout.

The [test architecture](testing.md) owns these automated checks. Its optional
native-input smoke is exploratory and does not block a release.

## Performance budgets

Run `./scripts/performance` on a release build. The budgets are deliberately
generous cross-platform regression limits, not product claims.

| Scenario                                   |                                     Fixture |    Budget |
| ------------------------------------------ | ------------------------------------------: | --------: |
| Storage-backed Today queue and controller  | 15,000 cards across two SQLite-backed decks |      60 s |
| Storage-backed mixed-script Library search |                  15,000 SQLite-backed notes |      60 s |
| Released-shape large migration             |      10,000-card schema-8 SQLite collection |      60 s |
| Representative archive export              |                5,000 notes plus local media |      30 s |
| Representative archive validation          |                5,000 notes plus local media |      30 s |
| Today queue construction                   |                   1,000,000 in-memory cards |      15 s |
| Cross-script substring search              |                   250,000 in-memory records |       5 s |
| Time-budget policy aggregate               |                   1,000,000 aggregate cards |       1 s |
| Media integrity scan                       |                   10,000 filesystem objects |      30 s |
| Browser shell startup                      |            Primary action ready in Chromium |       2 s |
| Packaged shell startup                     |                Clean collection initialized |      10 s |
| New database migration                     |                              Current schema |       2 s |
| Warm startup database open                 |                                    50 opens | 5 s total |

The main-branch CI performance job runs serially and prints fixture bytes and
measurements in its log. Run it locally before merging performance-sensitive
changes. A budget failure blocks release promotion. Investigate host noise once,
but do not increase a budget without documenting the regression and its user
impact here.

## Version and recovery policy

`npm run release:check` enforces one version across Cargo, npm, Tauri, and their
lockfiles, contiguous database migrations, published archive versions, bundle
metadata, icons, and current release documentation. Meiki v0.2.0 publishes
database schema **10** and `.meiki` archive version **4**. The historical v0.1
release published schema 7 and archive version 1. The schema-7 migration fixture
and archive-version 1 through 3 import coverage remain release inputs. Future
releases must keep migration fixtures from every released database schema and
import fixtures from every published archive version.

Database migrations, imports, and restores create recovery points before
durable state changes. A corrupt database backup or media companion must fail
validation before replacement. Never repair immutable review events in place.

## Release procedure

1. Run `./scripts/check`.
2. Run `./scripts/performance`.
3. Confirm no release-blocking GitHub issue is open.
4. Update all four version declarations, generated lockfiles, and release notes
   in one change.
5. Push a `vMAJOR.MINOR.PATCH` tag, or run the Package workflow manually for an
   unsigned release candidate.
6. Confirm all three package jobs and provenance attestations pass.
7. Verify each artifact against `SHA256SUMS`, install it on a clean host, and
   complete the personal release journey.
