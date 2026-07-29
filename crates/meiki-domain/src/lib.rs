//! Language-neutral entities for Meiki.
//!
//! This crate intentionally has no framework, database, UI, or locale
//! dependency. Text is stored losslessly; language metadata is optional, and
//! cloze identity is represented by semantic segments rather than offsets.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Direction {
    #[default]
    Auto,
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MatchingPolicy {
    #[default]
    Strict,
    Forgiving,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizedText {
    pub value: String,
    pub language_tag: Option<String>,
    pub direction: Direction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StudySettings {
    /// Desired recall probability in basis points (for example, 9000 = 90%).
    pub target_retention_basis_points: u16,
    pub new_cards_per_day: u32,
    pub maximum_interval_days: u32,
}

impl Default for StudySettings {
    fn default() -> Self {
        Self {
            target_retention_basis_points: 9_000,
            new_cards_per_day: 20,
            maximum_interval_days: 36_500,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StudySettingsOverride {
    pub target_retention_basis_points: Option<u16>,
    pub new_cards_per_day: Option<u32>,
    pub maximum_interval_days: Option<u32>,
}

impl StudySettings {
    /// Resolves collection defaults, deck overrides, and card overrides in
    /// increasing order of specificity.
    pub fn resolve(
        defaults: &Self,
        deck: &StudySettingsOverride,
        card: &StudySettingsOverride,
    ) -> Self {
        Self {
            target_retention_basis_points: card
                .target_retention_basis_points
                .or(deck.target_retention_basis_points)
                .unwrap_or(defaults.target_retention_basis_points),
            new_cards_per_day: card
                .new_cards_per_day
                .or(deck.new_cards_per_day)
                .unwrap_or(defaults.new_cards_per_day),
            maximum_interval_days: card
                .maximum_interval_days
                .or(deck.maximum_interval_days)
                .unwrap_or(defaults.maximum_interval_days),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deck {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub language_tag: Option<String>,
    pub direction: Direction,
    pub matching_policy: MatchingPolicy,
    pub settings: StudySettingsOverride,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Annotation {
    pub id: String,
    pub label: String,
    pub value: String,
    pub language_tag: Option<String>,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaKind {
    Audio,
    Image,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaReference {
    pub id: String,
    pub content_hash: String,
    pub kind: MediaKind,
    pub media_type: String,
    pub original_file_name: Option<String>,
    pub alt_text: Option<String>,
    pub language_tag: Option<String>,
    pub direction: Direction,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceItem {
    pub id: String,
    pub deck_id: String,
    pub segments: Vec<SemanticSegment>,
    pub language_tag: Option<String>,
    pub direction: Direction,
    pub tags: Vec<Tag>,
    pub annotations: Vec<Annotation>,
    pub explanation: Option<LocalizedText>,
    pub media: Vec<MediaReference>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Product-language alias for a persisted source item.
pub type SourceNote = SourceItem;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSegment {
    pub id: String,
    pub ordinal: u32,
    pub content: SegmentContent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SegmentContent {
    Text(String),
    Cloze { cloze_id: String, text: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cloze {
    pub id: String,
    pub source_item_id: String,
    pub answer: String,
    pub accepted_answers: Vec<String>,
    pub hint: Option<LocalizedText>,
    pub language_tag: Option<String>,
    pub direction: Direction,
    pub matching_policy: Option<MatchingPolicy>,
    pub annotations: Vec<Annotation>,
    pub explanation: Option<LocalizedText>,
    pub media: Vec<MediaReference>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Card {
    pub id: String,
    pub cloze_id: String,
    pub content_version: u64,
    pub settings: StudySettingsOverride,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchedulerParameterSet {
    pub id: String,
    pub engine_version: String,
    pub parameters: Vec<f64>,
    pub created_at_ms: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StudyIntensity {
    Light,
    #[default]
    Balanced,
    Intensive,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OptimizerStatus {
    #[default]
    NeverRun,
    InsufficientData,
    Adopted,
    Rejected,
    Failed,
    RolledBack,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerProfile {
    pub deck_id: String,
    pub engine_version: String,
    pub active_parameter_set_id: String,
    pub previous_parameter_set_id: Option<String>,
    pub intensity: StudyIntensity,
    pub daily_time_budget_minutes: Option<u32>,
    pub day_boundary_minutes: u16,
    pub optimizer_status: OptimizerStatus,
    /// Deterministic diagnostic JSON that never includes learning content.
    pub optimizer_diagnostics: Option<String>,
    pub updated_at_ms: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonResult {
    Exact,
    AcceptedVariant,
    NearMatch,
    Incorrect,
    Empty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Grade {
    Again,
    Hard,
    Good,
    Easy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleState {
    pub card_id: String,
    pub version: u64,
    pub due_at_ms: i64,
    pub ideal_due_at_ms: i64,
    pub interval_milliseconds: u64,
    pub interval_seconds: u64,
    pub repetitions: u32,
    /// Stability as fixed-point milliseconds to preserve exact projections.
    pub stability_milliseconds: u64,
    /// Difficulty in the inclusive range 1,000–10,000.
    pub difficulty_millipoints: u32,
    pub last_reviewed_at_ms: Option<i64>,
    pub last_review_event_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewEvent {
    pub id: String,
    pub card_id: String,
    pub card_content_version: u64,
    pub raw_response: String,
    pub normalized_response: String,
    pub comparison: ComparisonResult,
    pub suggested_grade: Grade,
    pub chosen_grade: Grade,
    pub reviewed_at_ms: i64,
    pub scheduler_version: String,
    pub scheduler_parameter_set_id: Option<String>,
    pub target_retention_basis_points: u16,
    pub previous_schedule: ScheduleState,
    pub next_schedule: ScheduleState,
}

#[cfg(test)]
mod tests {
    use super::{
        Direction, SegmentContent, SemanticSegment, SourceItem, StudySettings,
        StudySettingsOverride,
    };

    #[test]
    fn source_segments_preserve_order_and_cloze_identity() {
        let item = SourceItem {
            id: "source-1".into(),
            deck_id: "deck-1".into(),
            segments: vec![
                SemanticSegment {
                    id: "segment-1".into(),
                    ordinal: 0,
                    content: SegmentContent::Text("日曜日は図書館に".into()),
                },
                SemanticSegment {
                    id: "segment-2".into(),
                    ordinal: 1,
                    content: SegmentContent::Cloze {
                        cloze_id: "cloze-1".into(),
                        text: "行きます".into(),
                    },
                },
            ],
            language_tag: Some("ja".into()),
            direction: Direction::Auto,
            tags: Vec::new(),
            annotations: Vec::new(),
            explanation: None,
            media: Vec::new(),
            created_at_ms: 1_000,
            updated_at_ms: 1_000,
        };

        assert!(matches!(
            item.segments[1].content,
            SegmentContent::Cloze {
                ref cloze_id,
                ..
            } if cloze_id == "cloze-1"
        ));
    }

    #[test]
    fn card_settings_override_deck_settings() {
        let defaults = StudySettings::default();
        let deck = StudySettingsOverride {
            new_cards_per_day: Some(12),
            maximum_interval_days: Some(2_000),
            ..StudySettingsOverride::default()
        };
        let card = StudySettingsOverride {
            new_cards_per_day: Some(5),
            ..StudySettingsOverride::default()
        };

        let resolved = StudySettings::resolve(&defaults, &deck, &card);
        assert_eq!(resolved.target_retention_basis_points, 9_000);
        assert_eq!(resolved.new_cards_per_day, 5);
        assert_eq!(resolved.maximum_interval_days, 2_000);
    }
}
