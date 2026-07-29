//! Use cases and versioned desktop data-transfer objects.

use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use meiki_domain::{ComparisonResult, Direction, Grade, ReviewEvent, SegmentContent, SourceItem};
use meiki_scheduler::schedule_review;
use meiki_storage::{SAMPLE_CARD_ID, Storage, StorageError, StoredStudyCard};
use meiki_text::compare_answer;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("{0}")]
    Storage(#[from] StorageError),
    #[error("failed to prepare the collection directory: {0}")]
    CollectionDirectory(#[source] std::io::Error),
    #[error("the card changed; reload it before continuing")]
    StaleCard,
    #[error("stored timestamp is invalid: {0}")]
    InvalidTimestamp(i64),
    #[error("stored numeric value is too large for the desktop contract: {0}")]
    NumericRange(&'static str),
}

#[derive(Debug, Error)]
pub enum ContractExportError {
    #[error("failed to prepare the TypeScript output directory: {0}")]
    OutputDirectory(#[from] std::io::Error),
    #[error("failed to generate a TypeScript contract: {0}")]
    TypeScript(#[from] ts_rs::ExportError),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum DirectionDto {
    Auto,
    Ltr,
    Rtl,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ComparisonResultDto {
    Exact,
    AcceptedVariant,
    NearMatch,
    Incorrect,
    Empty,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum GradeDto {
    Again,
    Hard,
    Good,
    Easy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct StudyCardDto {
    pub card_id: String,
    pub card_content_version: u32,
    pub schedule_version: u32,
    pub prompt: String,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
    pub due_at: String,
    pub completed_reviews: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct CheckAnswerRequest {
    pub card_id: String,
    pub card_content_version: u32,
    pub schedule_version: u32,
    pub raw_response: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct RevealDto {
    pub card_id: String,
    pub card_content_version: u32,
    pub schedule_version: u32,
    pub full_source: String,
    pub expected_answer: String,
    pub raw_response: String,
    pub comparison: ComparisonResultDto,
    pub suggested_grade: GradeDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct GradeReviewRequest {
    pub card_id: String,
    pub card_content_version: u32,
    pub schedule_version: u32,
    pub raw_response: String,
    pub chosen_grade: GradeDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct GradeReviewResultDto {
    pub review_event_id: String,
    pub schedule_version: u32,
    pub due_at: String,
    pub interval_seconds: u32,
}

#[derive(Clone, Debug)]
pub struct ApplicationService {
    collection_path: PathBuf,
}

impl ApplicationService {
    /// Creates a use-case service for one collection database path.
    pub fn new(collection_path: impl Into<PathBuf>) -> Self {
        Self {
            collection_path: collection_path.into(),
        }
    }

    /// Ensures the walking-skeleton collection exists and returns its card.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when the collection directory or database
    /// cannot be prepared.
    pub fn initialize_collection(&self) -> Result<StudyCardDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        storage.seed_walking_skeleton(Utc::now().timestamp_millis())?;
        study_card_dto(&storage, SAMPLE_CARD_ID)
    }

    /// Restores a study card from the collection.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when the card cannot be loaded.
    pub fn get_study_card(&self, card_id: &str) -> Result<StudyCardDto, ApplicationError> {
        let storage = self.open_storage()?;
        study_card_dto(&storage, card_id)
    }

    /// Compares a raw answer and returns the reveal without mutating history.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::StaleCard`] when the observed versions no
    /// longer match, or another [`ApplicationError`] when loading fails.
    pub fn check_answer(
        &self,
        request: &CheckAnswerRequest,
    ) -> Result<RevealDto, ApplicationError> {
        let storage = self.open_storage()?;
        let stored = storage.load_study_card(&request.card_id)?;
        ensure_card_is_current(
            &stored,
            u64::from(request.card_content_version),
            u64::from(request.schedule_version),
        )?;
        let comparison = compare_answer(
            &stored.cloze.answer,
            &stored.cloze.accepted_answers,
            &request.raw_response,
        );
        let suggested_grade = suggested_grade(comparison.result);

        Ok(RevealDto {
            card_id: stored.card.id,
            card_content_version: desktop_u32(stored.card.content_version, "card content version")?,
            schedule_version: desktop_u32(stored.schedule.version, "schedule version")?,
            full_source: render_source(&stored.source_item, None),
            expected_answer: stored.cloze.answer,
            raw_response: request.raw_response.clone(),
            comparison: comparison.result.into(),
            suggested_grade: suggested_grade.into(),
        })
    }

    /// Revalidates, schedules, and commits one completed review.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::StaleCard`] when the observed versions no
    /// longer match, or another [`ApplicationError`] when scheduling data cannot
    /// be loaded or committed.
    pub fn grade_review(
        &self,
        request: &GradeReviewRequest,
    ) -> Result<GradeReviewResultDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        let stored = storage.load_study_card(&request.card_id)?;
        ensure_card_is_current(
            &stored,
            u64::from(request.card_content_version),
            u64::from(request.schedule_version),
        )?;

        let comparison = compare_answer(
            &stored.cloze.answer,
            &stored.cloze.accepted_answers,
            &request.raw_response,
        );
        let suggested = suggested_grade(comparison.result);
        let reviewed_at_ms = Utc::now().timestamp_millis();
        let decision = schedule_review(
            &stored.schedule,
            request.chosen_grade.into(),
            reviewed_at_ms,
        );
        let review_event_id = Uuid::new_v4().to_string();
        let mut next_schedule = decision.next_state;
        next_schedule.last_review_event_id = Some(review_event_id.clone());

        let event = ReviewEvent {
            id: review_event_id.clone(),
            card_id: stored.card.id,
            card_content_version: stored.card.content_version,
            raw_response: request.raw_response.clone(),
            normalized_response: comparison.normalized_response,
            comparison: comparison.result,
            suggested_grade: suggested,
            chosen_grade: request.chosen_grade.into(),
            reviewed_at_ms,
            scheduler_version: decision.scheduler_version.to_owned(),
            previous_schedule: stored.schedule,
            next_schedule,
        };
        let committed = storage.commit_review(&event)?;

        Ok(GradeReviewResultDto {
            review_event_id,
            schedule_version: desktop_u32(committed.version, "schedule version")?,
            due_at: timestamp_string(committed.due_at_ms)?,
            interval_seconds: desktop_u32(committed.interval_seconds, "interval seconds")?,
        })
    }

    fn open_storage(&self) -> Result<Storage, ApplicationError> {
        if let Some(parent) = self.collection_path.parent() {
            fs::create_dir_all(parent).map_err(ApplicationError::CollectionDirectory)?;
        }
        Ok(Storage::open(&self.collection_path)?)
    }
}

fn study_card_dto(storage: &Storage, card_id: &str) -> Result<StudyCardDto, ApplicationError> {
    let stored = storage.load_study_card(card_id)?;
    let completed_reviews = storage.review_count(card_id)?;
    Ok(StudyCardDto {
        card_id: stored.card.id,
        card_content_version: desktop_u32(stored.card.content_version, "card content version")?,
        schedule_version: desktop_u32(stored.schedule.version, "schedule version")?,
        prompt: render_source(&stored.source_item, Some(&stored.cloze.id)),
        language_tag: stored.source_item.language_tag,
        direction: stored.source_item.direction.into(),
        due_at: timestamp_string(stored.schedule.due_at_ms)?,
        completed_reviews: desktop_u32(completed_reviews, "completed review count")?,
    })
}

fn ensure_card_is_current(
    stored: &StoredStudyCard,
    card_content_version: u64,
    schedule_version: u64,
) -> Result<(), ApplicationError> {
    if stored.card.content_version != card_content_version
        || stored.schedule.version != schedule_version
    {
        return Err(ApplicationError::StaleCard);
    }
    Ok(())
}

fn render_source(source: &SourceItem, hidden_cloze_id: Option<&str>) -> String {
    source
        .segments
        .iter()
        .map(|segment| match &segment.content {
            SegmentContent::Cloze { cloze_id, text }
                if hidden_cloze_id == Some(cloze_id.as_str()) =>
            {
                "[…]"
            }
            SegmentContent::Text(text) | SegmentContent::Cloze { text, .. } => text.as_str(),
        })
        .collect()
}

const fn suggested_grade(comparison: ComparisonResult) -> Grade {
    match comparison {
        ComparisonResult::Exact | ComparisonResult::AcceptedVariant => Grade::Good,
        ComparisonResult::NearMatch => Grade::Hard,
        ComparisonResult::Incorrect | ComparisonResult::Empty => Grade::Again,
    }
}

fn timestamp_string(timestamp_ms: i64) -> Result<String, ApplicationError> {
    DateTime::<Utc>::from_timestamp_millis(timestamp_ms)
        .map(|timestamp| timestamp.to_rfc3339())
        .ok_or(ApplicationError::InvalidTimestamp(timestamp_ms))
}

fn desktop_u32(value: u64, field: &'static str) -> Result<u32, ApplicationError> {
    u32::try_from(value).map_err(|_| ApplicationError::NumericRange(field))
}

impl From<Direction> for DirectionDto {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Auto => Self::Auto,
            Direction::LeftToRight => Self::Ltr,
            Direction::RightToLeft => Self::Rtl,
        }
    }
}

impl From<ComparisonResult> for ComparisonResultDto {
    fn from(value: ComparisonResult) -> Self {
        match value {
            ComparisonResult::Exact => Self::Exact,
            ComparisonResult::AcceptedVariant => Self::AcceptedVariant,
            ComparisonResult::NearMatch => Self::NearMatch,
            ComparisonResult::Incorrect => Self::Incorrect,
            ComparisonResult::Empty => Self::Empty,
        }
    }
}

impl From<Grade> for GradeDto {
    fn from(value: Grade) -> Self {
        match value {
            Grade::Again => Self::Again,
            Grade::Hard => Self::Hard,
            Grade::Good => Self::Good,
            Grade::Easy => Self::Easy,
        }
    }
}

impl From<GradeDto> for Grade {
    fn from(value: GradeDto) -> Self {
        match value {
            GradeDto::Again => Self::Again,
            GradeDto::Hard => Self::Hard,
            GradeDto::Good => Self::Good,
            GradeDto::Easy => Self::Easy,
        }
    }
}

/// Generates TypeScript files for every desktop DTO.
///
/// # Errors
///
/// Returns [`ContractExportError`] when the output directory or a generated
/// file cannot be written.
pub fn export_typescript_contracts(output: &Path) -> Result<(), ContractExportError> {
    fs::create_dir_all(output)?;
    DirectionDto::export_all_to(output)?;
    ComparisonResultDto::export_all_to(output)?;
    GradeDto::export_all_to(output)?;
    StudyCardDto::export_all_to(output)?;
    CheckAnswerRequest::export_all_to(output)?;
    RevealDto::export_all_to(output)?;
    GradeReviewRequest::export_all_to(output)?;
    GradeReviewResultDto::export_all_to(output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        ApplicationService, CheckAnswerRequest, ComparisonResultDto, GradeDto, GradeReviewRequest,
    };

    #[test]
    fn walking_skeleton_checks_grades_and_restores() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        let card = service.initialize_collection().unwrap();
        assert_eq!(card.prompt, "日曜日は図書館に[…]");

        let reveal = service
            .check_answer(&CheckAnswerRequest {
                card_id: card.card_id.clone(),
                card_content_version: card.card_content_version,
                schedule_version: card.schedule_version,
                raw_response: " 行きます ".into(),
            })
            .unwrap();
        assert_eq!(reveal.comparison, ComparisonResultDto::Exact);
        assert_eq!(reveal.suggested_grade, GradeDto::Good);

        let result = service
            .grade_review(&GradeReviewRequest {
                card_id: card.card_id,
                card_content_version: card.card_content_version,
                schedule_version: card.schedule_version,
                raw_response: reveal.raw_response,
                chosen_grade: reveal.suggested_grade,
            })
            .unwrap();
        assert_eq!(result.schedule_version, 1);

        let restarted_service = ApplicationService::new(&path);
        let restored = restarted_service.get_study_card("sample-card").unwrap();
        assert_eq!(restored.schedule_version, 1);
        assert_eq!(restored.completed_reviews, 1);
        assert_eq!(restored.due_at, result.due_at);
    }
}
