# ADR 0005: Central Unicode text engine

- Status: accepted
- Date: 2026-07-29

## Decision

`meiki-text` owns all language-neutral text derivation. Domain and storage
retain original UTF-8 values; callers request derived comparison values, search
keys, grapheme positions, differences, and bidi rendering contracts from the
text engine.

Default answer comparison performs NFC normalization and outer-whitespace
trimming only. Case, diacritics, punctuation, internal spacing, and width
remain significant. Explicit options can relax each dimension. Compatibility
folding is used by the separate search-key operation and never changes stored
content.

Editor coordinates enter the engine as browser UTF-16 offsets and must resolve
to extended grapheme boundaries before semantic segments change. Invalid
positions fail instead of rounding into a grapheme. Differences and edit
distance also operate on extended grapheme clusters.

Near matches are feedback, not correctness. They require a minimum answer
length and a bounded absolute and relative grapheme edit distance. Exact and
explicit accepted answers always take precedence.

Bidirectional content is rendered with an explicit `auto`, `ltr`, or `rtl`
contract and Unicode isolate controls when text is embedded in another text
run. Application controls keep their own direction.

IME composition is an explicit state machine. Composition updates remain
opaque, and submission is disallowed until both the engine state and browser
event report that composition has ended.

## Consequences

- Raw and normalized responses remain distinguishable in application DTOs and
  immutable review events.
- Reveal DTOs carry a grapheme-aware difference for later study UI work.
- Search can discover substrings in scripts without whitespace tokenization.
- Unicode normalization, segmentation, and category dependencies are rejected
  outside `meiki-text` by the boundary check.
- Model changes require text-engine tests rather than feature-local string
  rules.
