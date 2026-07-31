# Data portability

## `.meiki` archive version 4

A `.meiki` file is a ZIP container with these exact entries:

- `manifest.json`: format name, schema version, full-collection scope, counts, collection
  checksum, and the expected media paths, sizes, and checksums.
- `collection.json`: canonical compact UTF-8 JSON containing decks, source
  notes, clozes, cards, immutable review events, current and baseline schedule
  projections, scheduler profiles, and every referenced parameter set.
- `media/sha256/<first two hex digits>/<remaining hex digits>`: one object for
  each referenced SHA-256 checksum.

SQLite is an implementation detail and is not stored in the archive. Readers
reject unknown versions, missing or additional entries, duplicate paths,
oversized data, invalid relationships, inconsistent review projections,
non-canonical checksums, and corrupt media. Archive paths are never used as
filesystem extraction paths.

Version 4 is the published v0.2 archive format. It removes obsolete card-level
scheduling overrides. Version 3 adds the collection-wide scheduling budget and
preserves each deck's optional budget override and automatic-controller state.
Version 2 records each card's explicit `unseen` or `introduced` lifecycle in
the baseline, current projection, and immutable event snapshots. Lifecycle is
independent from the scheduler's resettable repetition counter.

Version 1 is the published v0.1 archive format. Its import path deterministically
derives lifecycle from immutable review history and initialized memory fields
before validating the projection chain. Version-1 and version-2 fixtures stay
in the test suite while the writer emits version 4; version-3 archives remain
readable through the same bounded reader.

## Export and import behavior

Export always includes the complete collection: every deck, note, scheduler
profile, parameter set used by profiles or history, and referenced media.

Import always re-reads and validates the archive after preview. A full
collection archive has two possible actions:

- **Add deck** is available only when the archive contains exactly one
  non-trashed deck, no review events, and only untouched version-zero unseen
  cards. It preserves all existing decks, history, schedules, backups, and
  collection settings. The new deck uses Automatic scheduling, the current
  default scheduler parameters, the current collection budget, and the import
  time for its initial timestamps. Archive scheduler policy and controller
  diagnostics are ignored.
- **Replace collection** populates a temporary SQLite database and then
  replaces the live collection after the user types `REPLACE`.

Both actions create a managed recovery backup immediately before durable
changes. Adding a deck preflights every imported identity and commits its
database rows in one transaction. A repeated deck identity reports **Deck
already installed**. The live database is unchanged if validation, media
staging, or the database transaction fails, so the archive can be retried.
Media identities remain content hashes, equal bytes deduplicate, and checksum
validation happens both before and during import.

## Rolling backup policy

Meiki creates managed database backups before schema migrations, pristine deck
imports, collection replacement imports, and restores. Application-level recovery
points pair the SQLite backup with a checksum-verified `.media` directory;
media objects are merged back before database restore and are never
overwritten. Migration backups need no media copy because migrations do not
change the immutable media store.

Backups live in the collection's sibling `backups` directory. Each category
retains its newest five database files, and orphaned media companions are
pruned. Names contain a millisecond timestamp and a fixed-width sequence;
lexical pruning therefore matches creation order.

Restore first validates SQLite integrity, creates a `pre-restore` recovery
backup, and then uses SQLite's backup API to replace the collection. The UI
only restores files from the managed directory and requires the exact filename
as confirmation.
