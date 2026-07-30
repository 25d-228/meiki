//! Language-neutral entities for Meiki.
//!
//! This crate intentionally has no framework, database, UI, or locale
//! dependency. Text is stored losslessly; language metadata is optional, and
//! cloze identity is represented by semantic segments rather than offsets.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Auto,
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchingPolicy {
    #[default]
    Strict,
    Forgiving,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LocalizedText {
    pub value: String,
    pub language_tag: Option<String>,
    pub direction: Direction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct StudySettingsOverride {
    pub target_retention_basis_points: Option<u16>,
    pub new_cards_per_day: Option<u32>,
    pub maximum_interval_days: Option<u32>,
}

impl StudySettings {
    /// Resolves collection defaults and an optional deck override.
    pub fn resolve(defaults: &Self, deck: &StudySettingsOverride) -> Self {
        Self {
            target_retention_basis_points: deck
                .target_retention_basis_points
                .unwrap_or(defaults.target_retention_basis_points),
            new_cards_per_day: deck.new_cards_per_day.unwrap_or(defaults.new_cards_per_day),
            maximum_interval_days: deck
                .maximum_interval_days
                .unwrap_or(defaults.maximum_interval_days),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Annotation {
    pub id: String,
    pub label: String,
    pub value: String,
    pub language_tag: Option<String>,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Audio,
    Image,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaRole {
    PromptAudio,
    AnswerAudio,
    RevealImage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MediaReference {
    pub id: String,
    pub content_hash: String,
    pub kind: MediaKind,
    pub role: MediaRole,
    pub media_type: String,
    pub byte_size: u64,
    pub original_file_name: Option<String>,
    pub alt_text: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub language_tag: Option<String>,
    pub direction: Direction,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SemanticSegment {
    pub id: String,
    pub ordinal: u32,
    pub content: SegmentContent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentContent {
    Text(String),
    Cloze { cloze_id: String, text: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Card {
    pub id: String,
    pub cloze_id: String,
    pub content_version: u64,
    pub suspended: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SchedulerParameterSet {
    pub id: String,
    pub engine_version: String,
    pub parameters: Vec<f64>,
    pub created_at_ms: i64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulingMode {
    #[default]
    Automatic,
    Expert,
}

/// Reader-only compatibility for scheduler profiles in archive versions 1–2.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyStudyIntensity {
    Light,
    #[default]
    Balanced,
    Intensive,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CollectionSchedulingSettings {
    pub daily_time_budget_minutes: u32,
    pub updated_at_ms: i64,
}

impl Default for CollectionSchedulingSettings {
    fn default() -> Self {
        Self {
            daily_time_budget_minutes: 30,
            updated_at_ms: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SchedulerProfile {
    pub deck_id: String,
    pub engine_version: String,
    pub active_parameter_set_id: String,
    #[serde(default)]
    pub scheduling_mode: SchedulingMode,
    #[serde(default, alias = "daily_time_budget_minutes")]
    pub deck_daily_time_budget_minutes: Option<u32>,
    #[serde(default = "default_controller_version")]
    pub controller_version: String,
    #[serde(default = "default_controller_target")]
    pub controller_target_retention_basis_points: u16,
    #[serde(default = "default_controller_new_cards")]
    pub controller_new_cards_per_day: u32,
    #[serde(default)]
    pub controller_last_evaluated_day_start_ms: Option<i64>,
    #[serde(default)]
    pub controller_review_count: u64,
    #[serde(default)]
    pub controller_unseen_count: u64,
    #[serde(default)]
    pub controller_forecast_review_seconds_per_day: u64,
    #[serde(default)]
    pub controller_backlog_exceeds_budget: bool,
    #[serde(default)]
    pub controller_explanation: String,
    #[serde(default, rename = "intensity", skip_serializing)]
    pub legacy_intensity: LegacyStudyIntensity,
    pub day_boundary_minutes: u16,
    pub updated_at_ms: i64,
}

fn default_controller_version() -> String {
    "time-budget-v1".into()
}

const fn default_controller_target() -> u16 {
    9_000
}

const fn default_controller_new_cards() -> u32 {
    20
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonResult {
    Exact,
    AcceptedVariant,
    NearMatch,
    Incorrect,
    Empty,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Again,
    Hard,
    Good,
    Easy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewEventKind {
    Review,
    Undo,
}

/// Whether a card has ever been introduced by an active graded review.
///
/// This lifecycle is independent from scheduler memory and success counters:
/// a lapse never makes an introduced card unseen. Compensating the first and
/// only active review restores the unseen baseline.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardLifecycle {
    #[default]
    Unseen,
    Introduced,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScheduleState {
    pub card_id: String,
    pub version: u64,
    #[serde(default)]
    pub lifecycle: CardLifecycle,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewEvent {
    pub id: String,
    pub card_id: String,
    pub card_content_version: u64,
    pub kind: ReviewEventKind,
    pub undoes_review_event_id: Option<String>,
    pub raw_response: String,
    pub normalized_response: String,
    pub comparison: ComparisonResult,
    pub suggested_grade: Grade,
    pub chosen_grade: Grade,
    pub grade_overridden: bool,
    pub response_duration_ms: u64,
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
    fn deck_settings_override_collection_settings() {
        let defaults = StudySettings::default();
        let deck = StudySettingsOverride {
            new_cards_per_day: Some(12),
            maximum_interval_days: Some(2_000),
            ..StudySettingsOverride::default()
        };

        let resolved = StudySettings::resolve(&defaults, &deck);
        assert_eq!(resolved.target_retention_basis_points, 9_000);
        assert_eq!(resolved.new_cards_per_day, 12);
        assert_eq!(resolved.maximum_interval_days, 2_000);
    }
}
