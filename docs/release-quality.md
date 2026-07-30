# Release quality

This document defines the v0.1 release gate. A tag is a release candidate only
after the normal verification suite, the performance suite, and the package
workflow pass for the tagged commit.

## Supported matrix

| Boundary                                                       | Required coverage                                                                                                            |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Rust domain, text, scheduler, storage, media, and archive code | Unit, property, fixture, transaction, and integration tests on Linux, macOS, and Windows                                     |
| Desktop frontend                                               | Strict TypeScript check and production build on Linux, macOS, and Windows                                                    |
| Browser behavior                                               | Chromium end-to-end suite on Linux, including keyboard, IME, bidi, accessibility, recovery, and the personal release journey |
| Packages                                                       | Tauri bundle smoke build for Linux, macOS, and Windows from a version tag or manual release run                              |

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

The browser suite covers Japanese IME, Arabic/Persian RTL, Devanagari
combining text, Latin diacritics, CJK without spaces, mixed direction, mixed
script and punctuation, and multi-code-point emoji. Storage and archive tests
cover atomic review commits, compensating undo events, migrations, rolling
backups, checksum failures, and restore.

An open defect blocks release when it can cause data loss, an incorrect or
duplicated review commit, accidental IME submission, unreadable bidi content,
or a scheduler invariant failure. These are P0/P1 defects regardless of their
UI severity. Confirm that no such issue is open before creating a public
release.

## Accessibility gate

Every primary screen is keyboard operable and is audited against automated
WCAG 2.0 A/AA and WCAG 2.1 AA rules. The suite also checks:

- skip navigation and intentional focus transfer;
- labelled controls and dialog names;
- polite status announcements and assertive error announcements;
- visible focus and 4.5:1 normal-text contrast in light and dark themes;
- reduced motion;
- isolated RTL learning content without reversing interface controls.

Automated checks do not replace a screen-reader pass. Before a public release,
manually complete the release journey with VoiceOver on macOS or NVDA on
Windows.

## Performance budgets

Run `./scripts/performance` on a release build. The budgets are deliberately
generous cross-platform regression limits, not product claims.

| Scenario                      |                          Fixture |    Budget |
| ----------------------------- | -------------------------------: | --------: |
| Today queue construction      |                  1,000,000 cards |      15 s |
| Cross-script substring search |                  250,000 records |       5 s |
| Time-budget policy aggregate  |                  1,000,000 cards |       1 s |
| Media integrity scan          |                   10,000 objects |      30 s |
| Desktop shell startup         | Primary action ready in Chromium |       2 s |
| New database migration        |              current v0.1 schema |       2 s |
| Warm startup database open    |                         50 opens | 5 s total |

The CI performance job runs serially and prints measurements in its log. A
budget failure blocks merge. Investigate host noise once, but do not increase a
budget without documenting the regression and its user impact here.

## Version and recovery policy

`npm run release:check` enforces one version across Cargo, npm, and Tauri,
contiguous database migrations, the published archive version, bundle
metadata, icons, and release documentation. v0.1 publishes database schema 7
and `.meiki` archive version 1. Current development uses database schema 10 and
archive version 4 while retaining the released schema-7 migration fixture and
version-1 and version-2 archive import coverage. Future releases must keep
migration fixtures from every released database schema and import fixtures
from every published archive version.

Database migrations, imports, and restores create recovery points before
durable state changes. A corrupt database backup or media companion must fail
validation before replacement. Never repair immutable review events in place.

## Release procedure

1. Run `./scripts/check`.
2. Run `./scripts/performance`.
3. Confirm no release-blocking GitHub issue is open.
4. Update all four version declarations and release notes in one commit.
5. Push a `vMAJOR.MINOR.PATCH` tag, or run the Package workflow manually for an
   unsigned release candidate.
6. Confirm all three package jobs and provenance attestations pass.
7. Verify each artifact against `SHA256SUMS`, install it on a clean host, and
   complete the personal release journey.
