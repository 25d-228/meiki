# Contributing

Current release: **0.2.0**. Read the
[v0.2.0 release notes](docs/releases/v0.2.0.md), the accepted
[architecture decisions](docs/adr/), and the target issue before changing a
boundary. Keep the product scope, module direction, and test ownership defined
by those tracked documents.

Run all development commands through `./scripts/dev-env` so generated state
stays in the project-local virtual environment. Before handing off a change,
run:

```sh
./scripts/dev-env npm run verify
```

For a release-affecting change, also run:

```sh
./scripts/performance
./scripts/dev-env npm run release:check
```

The platform matrix, performance budgets, severity gate, signing posture, and
tag procedure are defined in [docs/release-quality.md](docs/release-quality.md).
Test ownership, deterministic seams, and the production-boundary failure matrix
are defined in [docs/testing.md](docs/testing.md).

Do not place SQL outside `meiki-storage`, scheduling rules outside
`meiki-scheduler`, text comparison outside `meiki-text`, or business rules in
Tauri commands and Svelte components. Generated files under
`apps/desktop/src/lib/generated` come from Rust DTOs and must not be edited by
hand.
