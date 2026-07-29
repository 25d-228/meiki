//! Language-neutral entities for Meiki.
//!
//! This crate intentionally has no framework, database, UI, or locale dependency.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Auto,
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceItem {
    pub id: String,
    pub segments: Vec<SemanticSegment>,
    pub language_tag: Option<String>,
    pub direction: Direction,
}

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Card {
    pub id: String,
    pub cloze_id: String,
    pub content_version: u64,
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
    pub interval_seconds: u64,
    pub repetitions: u32,
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
    pub previous_schedule: ScheduleState,
    pub next_schedule: ScheduleState,
}

#[cfg(test)]
mod tests {
    use super::{SegmentContent, SemanticSegment, SourceItem};

    #[test]
    fn source_segments_preserve_order_and_cloze_identity() {
        let item = SourceItem {
            id: "source-1".into(),
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
            direction: super::Direction::Auto,
        };

        assert!(matches!(
            item.segments[1].content,
            SegmentContent::Cloze {
                ref cloze_id,
                ..
            } if cloze_id == "cloze-1"
        ));
    }
}
