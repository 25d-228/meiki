# 明記 · Meiki

Meiki is a local-first desktop application for typed-cloze recall. The current
foundation implements one complete offline path: open the bundled Japanese
sample, type the hidden text, reveal and grade the answer, then restore the
persisted review and next due time after restart.

This repository is implementing [the v0.1 epic](https://github.com/25d-228/meiki/issues/1).
The foundation and core persistence milestones are tracked in
[issue #2](https://github.com/25d-228/meiki/issues/2) and
[issue #3](https://github.com/25d-228/meiki/issues/3).

## Verify a clean checkout

Install Python 3, Node.js 24, and Rust 1.85 or newer, then run one command:

```sh
./scripts/check
```

The script creates an ignored project-local `.venv`, installs locked npm
dependencies and Chromium there, then checks formatting, module boundaries,
linting, generated Rust-to-TypeScript contracts, types, builds, unit tests, and
browser tests. Cargo build output, npm cache, and browser binaries stay inside
the virtual environment.

## Develop

Prepare dependencies:

```sh
./scripts/dev-env npm ci
```

Launch the Tauri desktop application:

```sh
./scripts/dev-env npm run tauri --workspace @meiki/desktop -- dev
```

Run an individual command inside the same isolated environment:

```sh
./scripts/dev-env cargo test -p meiki-storage
./scripts/dev-env npm run typecheck
```

Generate TypeScript contracts after changing a desktop DTO:

```sh
./scripts/dev-env npm run bindings
```

## Workspace

```text
apps/desktop/             Svelte 5 UI and thin Tauri 2 adapter
crates/meiki-application/ Use cases and versioned desktop DTOs
crates/meiki-domain/      Framework-free, language-neutral entities
crates/meiki-text/        Centralized text comparison
crates/meiki-scheduler/   Pure scheduling boundary
crates/meiki-storage/     Versioned SQLite migrations, repositories, and backups
crates/meiki-media/       Content-addressed media ownership boundary
crates/meiki-portable/    Versioned, validated .meiki archive boundary
docs/adr/                 Architecture decisions
```

SQLite is created in the operating system's application-data directory. The
application needs no account or network connection.

## Data portability and recovery

Settings can export a full collection or one deck as a `.meiki` archive, and
Library can export selected notes. The versioned archive contains canonical
UTF-8 structured data, immutable review history and current projections,
scheduler metadata, a manifest, and checksum-addressed media. It does not
contain SQLite, and imports validate the complete archive before staging any
database change.

Merge import deterministically namespaces incoming identities and reuses media
with identical SHA-256 checksums. Replace import is available only for a full
collection. Both modes show a preview and require explicit typed confirmation.
The existing Library JSON export is a lightweight interoperability format and
does not promise to preserve scheduling state.

Meiki keeps the newest five backups for each automatic backup category.
Migrations, full schedule rebuilds, imports, and restores create a backup
before changing durable collection state. Settings lists these backups and
requires the exact filename before restore. Pruning is lexical by the
timestamp-and-sequence filename, so it remains deterministic even when
multiple backups are created within one millisecond. Application recovery
points include a checksum-verified media-store companion.
