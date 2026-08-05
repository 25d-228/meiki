# 明記 · Meiki

Meiki is a personal, network-free desktop application for language-neutral typed-cloze
recall. Create structured clozes in any Unicode script, study entirely from the
keyboard, and keep immutable review history, adaptive schedules, and
checksum-addressed media on your device.

Current release: **0.2.0**. See the
[v0.2.0 release notes](docs/releases/v0.2.0.md), the accepted
[architecture decisions](docs/adr/), and the [user guide](docs/user-guide.md)
for the current product. [Release quality](docs/release-quality.md) defines the
supported test, interface-quality, performance, and packaging matrix.

## Verify a clean checkout

Install Python 3, Node.js 24, and Rust 1.85 or newer, then run one command:

```sh
./scripts/check
```

The script creates an ignored project-local `.venv`, installs locked npm
dependencies and Chromium there, then checks formatting, module boundaries,
release metadata, linting, generated Rust-to-TypeScript contracts, types,
builds, unit tests, accessibility, and browser-level release scenarios. Cargo
build output, npm cache, and browser binaries stay inside the virtual
environment.

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

Run the release performance budgets:

```sh
./scripts/performance
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

## Product scope

Meiki is deliberately one personal collection on one desktop. The collection
may contain flat decks, tags, typed clozes, and local media. Daily study,
authoring, deck search, portable language bundles, and manual expert
scheduling are the supported product.

Accounts, identity, cloud sync, mobile clients, marketplaces, plugins,
executable card templates, collaborative editing, shared ownership, automatic
content generation, social or competitive features, and network-dependent
study behavior are permanent non-goals. Production code and desktop
permissions must not require a network.

## Language bundles

Decks imports and exports versioned `.meiki` language bundles. A bundle contains
ordered decks, typed-cloze content, and referenced local media, but never the
user's review history, learned schedules, or collection study-time setting.
Imported cards start unseen and each deck remains independently schedulable.

Meiki validates bundle structure and media before applying changes. Migrations
and transactional bundle operations create internal recovery points when
needed; they are not a collection-management workflow in Settings.
