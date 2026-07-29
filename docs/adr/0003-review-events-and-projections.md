# ADR 0003: Review events and schedule projections

- Status: accepted
- Date: 2026-07-29

## Decision

A completed review appends an immutable `review_events` row and updates its
card's `schedule_states` projection in one SQLite transaction.

The command supplies the card content version and schedule version it observed.
Storage validates both inside the transaction. A stale command writes neither
the event nor the projection. Database triggers reject updates and deletes of
review events.

Each event records the raw and normalized response, comparison, suggested and
chosen grades, scheduler version, timestamp, and complete before/after schedule
values. The current schedule is therefore a replaceable projection rather than
the source of review history.

## Consequences

Retries are safe when callers reload after a stale-version error. Future
projection rebuild and undo work can use the immutable history without rewriting
past reviews. The `foundation-v1` interval policy is only the vertical-skeleton
engine; issue #7 replaces it through the existing pure scheduler boundary.
