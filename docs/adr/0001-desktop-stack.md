# ADR 0001: Desktop stack

- Status: accepted
- Date: 2026-07-29

## Decision

Use Tauri 2 as the desktop shell, Svelte 5 with strict TypeScript for the UI,
Rust 2024 for application and core code, and SQLite with explicit migrations for
local persistence.

Rust DTOs derive TypeScript definitions with `ts-rs`. The generated files are
committed so the frontend and CI compile against the same versioned boundary.

## Rationale

The stack provides a small network-free desktop package, native filesystem access,
strong domain and transaction tests, and a direct typed boundary without placing
SQL or scheduling behavior in the UI.

## Consequences

Contributors need Rust, Node.js, and platform prerequisites for Tauri. Tauri
macros are one-call adapters over plain Rust command functions. Browser tests
use static DTO scenarios only; real `ApplicationService` journeys cover
SQLite, scheduling, text, media, bundle, and internal-recovery behavior. See the
[test architecture](../testing.md).
