# ADR 0005: Personal collection and network-free scope

- Status: accepted
- Date: 2026-07-30

## Decision

Meiki is one person's local collection on one desktop. The supported model is
a collection with optional flat decks, tags, language-neutral typed clozes,
local media, daily study, Library search and editing, complete collection
archives, rolling recovery backups, and an expert manual scheduling policy.

Accounts and identity, cloud sync, mobile clients, marketplaces, plugins and
extensions, executable card templates, collaborative editing and shared
ownership, automatic content generation, social or competitive features, and
network-dependent study behavior are permanent non-goals.

Production modules and desktop capabilities must not introduce network
clients or network permissions. The interface describes local behavior
directly; it does not display online or offline state.

## Rationale

The product promise is reliable personal learning and recovery. Keeping the
collection, scheduling, media, and recovery model local makes that promise
testable and avoids incomplete abstractions for systems the product does not
intend to build.

## Consequences

Settings have collection defaults and optional flat-deck overrides. Cards do
not carry scheduling overrides. Deck workflows stay intentionally small.
Portable archives represent complete collection replacement, while readers
may retain compatibility with older published archives. Engine identifiers,
maintenance operations, and other implementation details are not normal
product controls.

The boundary check rejects network dependencies and Tauri network
permissions. A request for one of the permanent non-goals requires replacing
this decision rather than adding an architectural seam in anticipation.
