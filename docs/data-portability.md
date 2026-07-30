# Data portability

## `.meiki` archive version 2

A `.meiki` file is a ZIP container with these exact entries:

- `manifest.json`: format name, schema version, scope, counts, collection
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

Version 2 records each card's explicit `unseen` or `introduced` lifecycle in
the baseline, current projection, and immutable event snapshots. Lifecycle is
independent from the scheduler's resettable repetition counter.

Version 1 is the published v0.1 schema. Its import path deterministically
derives lifecycle from immutable review history and initialized memory fields
before validating the projection chain. Version-1 fixtures stay in the test
suite while the writer emits version 2.

## Export and import behavior

Exports support a full collection, selected decks, and selected notes. The
selection includes the owning decks, scheduler profiles, parameter sets used
by profiles or history, and all referenced media.

Import always re-reads and validates the archive after preview. Merge mode
derives a stable namespace from the collection checksum and applies it to
every database identity. Repeating the same merge is detected as a collision.
Media identities remain content hashes, so equal bytes deduplicate. Replace
mode accepts only a full collection and preserves its original identities.

Both modes populate a temporary SQLite database first. The live database is
unchanged if validation or staging fails, and the same archive can be retried.
Immediately before replacement, Meiki creates a managed recovery backup.
Media objects are immutable additions; checksum validation happens both before
and during import.

The Library JSON export is the lightweight interchange format. It is intended
for external tooling and does not claim to preserve review history, schedule
projections, or scheduler parameters.

## Rolling backup policy

Meiki creates managed database backups before schema migrations, full
scheduler rebuilds, archive imports, and restores. Application-level recovery
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
