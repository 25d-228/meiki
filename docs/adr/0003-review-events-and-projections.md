# ADR 0003: Review events and schedule projections

- Status: accepted
- Date: 2026-07-29

## Decision

A completed review appends an immutable `review_events` row and updates its
card's `schedule_states` projection and queue metadata in one SQLite
transaction. Each card also owns an immutable schedule baseline.

The command supplies the card content version and schedule version it observed.
Storage validates both inside the transaction. A stale command writes neither
the event nor the projection. Database triggers reject updates and deletes of
review events.

Each event records the raw and normalized response, comparison, suggested and
chosen grades, scheduler and optional parameter-set version, target retention,
timestamp, and complete before/after schedule values. Events replay in
schedule-version order from the baseline. The current schedule is therefore a
rebuildable projection rather than the source of review history.

Card lifecycle is part of those schedule snapshots. A card begins `unseen`;
any graded review makes it `introduced`, and a lapse cannot reverse that
transition. Undo restores the lifecycle from the compensated review's previous
snapshot, so only compensating the first and sole active review makes a card
unseen again. Scheduler repetition and memory fields never classify lifecycle.

## Consequences

Retries are safe when callers reload after a stale-version error. Projection
repair and future undo work can use immutable history without rewriting past
reviews. The initial production engine is the versioned `fsrs-7` implementation;
future formula changes require a new engine identifier.
