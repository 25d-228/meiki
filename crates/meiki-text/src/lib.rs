//! Language-neutral Unicode behavior for authoring, review, and search.
//!
//! Raw user text stays outside normalization. This crate derives comparison
//! and search values, works in extended grapheme clusters, and exposes
//! direction and composition contracts without depending on a UI framework.

mod bidi;
mod comparison;
mod composition;
mod diff;
mod grapheme;
mod search;

pub use bidi::{BidiRenderContract, direction_attribute, isolate_for_display};
pub use comparison::{
    CaseSensitivity, Comparison, ComparisonOptions, DiacriticSensitivity, NearMatchOptions,
    PunctuationSensitivity, WhitespaceSensitivity, WidthSensitivity, compare_answer,
    compare_answer_with_options, normalize_for_comparison, normalize_for_default_comparison,
};
pub use composition::{CompositionEvent, CompositionPhase, CompositionState};
pub use diff::{DiffKind, DiffSegment, grapheme_diff, grapheme_distance};
pub use grapheme::{
    GraphemeIndex, GraphemeRange, SemanticPosition, TextBoundaryError, TextSplit,
    semantic_position_from_utf16,
};
pub use search::{normalize_for_search, search_contains};
