# Contributing

Read the [v0.1 epic](https://github.com/25d-228/meiki/issues/1) and the target
issue before changing a boundary. Work should follow the dependency order in the
epic.

Run all development commands through `./scripts/dev-env` so generated state
stays in the project-local virtual environment. Before handing off a change,
run:

```sh
./scripts/dev-env npm run verify
```

Do not place SQL outside `meiki-storage`, scheduling rules outside
`meiki-scheduler`, text comparison outside `meiki-text`, or business rules in
Tauri commands and Svelte components. Generated files under
`apps/desktop/src/lib/generated` come from Rust DTOs and must not be edited by
hand.
