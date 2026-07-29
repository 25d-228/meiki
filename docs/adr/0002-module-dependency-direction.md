# ADR 0002: Module dependency direction

- Status: accepted
- Date: 2026-07-29

## Decision

Internal dependencies point inward:

```text
Svelte UI → Tauri adapter → meiki-application
                              ├── meiki-domain
                              ├── meiki-media
                              ├── meiki-text → meiki-domain
                              ├── meiki-scheduler → meiki-domain
                              └── meiki-storage → meiki-domain

meiki-media remains an independent filesystem ownership boundary. The
application coordinates it with domain and storage references. meiki-portable
remains independent until its issue introduces concrete dependencies.
```

`meiki-domain` has no framework dependency. Only `meiki-storage` may depend on
SQLite. Tauri commands create an application service and delegate one use case;
they contain no rules. The managed Tauri state holds only the collection path,
not a global database connection or scheduler.

## Enforcement

`scripts/check-boundaries.py` validates direct internal Cargo dependencies and
rejects `rusqlite` use outside storage. Cargo and frontend linting enforce the
remaining compile-time boundaries.
