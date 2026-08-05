# Data portability

## `.meiki` archive version 4

A `.meiki` file is a ZIP container with these exact entries:

- `manifest.json`: format name, schema version, the compatibility scope marker,
  counts, collection checksum, and expected media paths, sizes, and checksums.
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

## Bundle export and import behavior

The product uses `.meiki` files for clean language bundles. Export writes the
remaining installed decks associated with one language in stable order, their
active typed-cloze cards and annotations, and referenced local media. Review
events, learned schedules, the collection study-time setting, unrelated decks,
moved-out cards, and Trash are omitted. Every exported card has a version-zero
unseen schedule.

Import previews the ordered decks and re-reads and validates the file before
changing the collection. It adds only missing stages, preserves installed
stages and their study state, validates identities before writes, and commits
bundle associations transactionally. Equal media objects deduplicate. A
validation, media, or transaction failure leaves the live collection
unchanged.

The version-4 writer retains the published compatibility scope marker because
changing it would create a new archive format. Product behavior is determined
by the clean-bundle validation rules, not by offering a collection-replacement
workflow. Readers retain published-version compatibility required to install
valid bundles from earlier Meiki data.

## Internal recovery policy

Schema migrations and transactional bundle operations create managed recovery
points before durable changes. These files are maintained internally and are
not listed or restored through the user interface. The newest five files per
category are retained, with paired media snapshots where the operation can
change media references.
