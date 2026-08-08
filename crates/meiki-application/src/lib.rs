//! Use cases and versioned desktop data-transfer objects.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use meiki_domain::{
    Annotation, CollectionSchedulingSettings, ComparisonResult, Direction, Grade, LocalizedText,
    MatchingPolicy, MediaKind, MediaReference, MediaRole, ReviewEvent, ReviewEventKind,
    SchedulerParameterSet, SchedulerProfile, SchedulingMode, SegmentContent, SourceItem,
    StudySettings, StudySettingsOverride,
};
use meiki_media::{DetectedMediaKind, MediaError, MediaStore};
use meiki_scheduler::{
    AutomaticPolicyDecision, AutomaticPolicyInput, CONTROLLER_VERSION, ENGINE_VERSION,
    FORECAST_DAYS, Fsrs7Engine, SchedulerConfig, SchedulerEngine, SchedulerError, automatic_policy,
};
use meiki_storage::{
    CardRepository, DeckRepository, SchedulerParameterSetRepository, SchedulerProfileRepository,
    SchedulingWorkload, Storage, StorageError, StoredStudyCard,
};
use meiki_text::{
    CaseSensitivity, ComparisonOptions, DiacriticSensitivity, DiffKind, DiffSegment,
    PunctuationSensitivity, WhitespaceSensitivity, WidthSensitivity, compare_answer_with_options,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

mod authoring;
mod deck_cards;
mod decks;
mod portable;
mod today;

pub use authoring::{
    AnnotationDraftDto, AuthoringClozeDto, AuthoringDraftDto, AuthoringPreviewDto,
    AuthoringSegmentDto, AuthoringSegmentKindDto, MakeClozeRequest, MatchingPolicyDto,
    RemoveClozeRequest, ReorderSegmentsRequest,
};
pub use deck_cards::{
    DeckCardActionDto, DeckCardActionRequest, DeckCardActionResultDto, DeckCardDeckDto,
    DeckCardDto, DeckCardOverviewDto, DeckCardRequest, DeckCardStatusDto, DeckCardTrashDto,
};
pub use decks::{
    BundleRemovalPreviewDto, BundleRemovalProgressDto, BundleRemovalRequest,
    BundleRemovalResultDto, CreateDeckRequest, DeckDto, DeckSummaryDto, DeleteDeckRequest,
    DeleteDeckResultDto, RenameDeckRequest,
};
pub use portable::{
    BundleDeckInstallStatusDto, BundleDeckPreviewDto, BundleExportRequest, BundleImportProgressDto,
    BundleImportRequest, BundleImportResultDto, BundleImportStageDto, BundlePreviewDto,
    PortableExportResultDto,
};
pub use today::{
    ALL_DECKS_ID, StudyAvailabilityDto, StudyPlanDto, TodayDeckDto, TodayOverviewDto,
    TodayQueueCardDto, TodayRequest,
};

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("{0}")]
    Storage(#[from] StorageError),
    #[error("failed to prepare the collection directory: {0}")]
    CollectionDirectory(#[source] std::io::Error),
    #[error("the card changed; reload it before continuing")]
    StaleCard,
    #[error("the card is not due yet")]
    CardNotDue,
    #[error("stored timestamp is invalid: {0}")]
    InvalidTimestamp(i64),
    #[error("stored numeric value is too large for the desktop contract: {0}")]
    NumericRange(&'static str),
    #[error("invalid authoring draft: {0}")]
    InvalidAuthoring(String),
    #[error("invalid Today request: {0}")]
    InvalidToday(String),
    #[error("invalid deck request: {0}")]
    InvalidDeck(String),
    #[error("invalid deck card request: {0}")]
    InvalidDeckCard(String),
    #[error("invalid text selection: {0}")]
    TextBoundary(#[from] meiki_text::TextBoundaryError),
    #[error("scheduler operation failed: {0}")]
    Scheduler(#[from] SchedulerError),
    #[error("unsupported scheduler engine version: {0}")]
    UnsupportedScheduler(String),
    #[error("invalid scheduler parameter file: {0}")]
    InvalidSchedulerParameterFile(String),
    #[error("scheduler parameter file operation failed: {0}")]
    SchedulerParameterIo(#[source] std::io::Error),
    #[error("scheduler parameter file JSON is invalid: {0}")]
    SchedulerParameterJson(#[source] serde_json::Error),
    #[error("media operation failed: {0}")]
    Media(#[from] MediaError),
    #[error("bundle file operation failed: {0}")]
    Portable(#[from] meiki_portable::PortableError),
    #[error("invalid portability request: {0}")]
    InvalidPortable(String),
    #[error("This bundle conflicts with existing cards in your collection.")]
    BundleConflict,
    #[error("portable data filesystem operation failed: {0}")]
    PortableIo(#[source] std::io::Error),
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SchedulingModeDto {
    Automatic,
    Expert,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum BudgetSourceDto {
    CollectionBudget,
    DeckOverride,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum TextDiffKindDto {
    Equal,
    Delete,
    Insert,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum MediaKindDto {
    Audio,
    Image,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum MediaRoleDto {
    PromptAudio,
    AnswerAudio,
    RevealImage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum MediaAvailabilityDto {
    Ready,
    Missing,
    Corrupt,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ImportMediaRequest {
    pub path: String,
    pub role: MediaRoleDto,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LocalizedTextDto {
    pub value: String,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct StudyAnnotationDto {
    pub label: String,
    pub value: String,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct StudyMediaDto {
    pub id: String,
    pub content_hash: String,
    pub kind: MediaKindDto,
    pub role: MediaRoleDto,
    pub media_type: String,
    #[ts(type = "number")]
    pub byte_size: u64,
    pub original_file_name: Option<String>,
    pub alt_text: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
    pub asset_path: Option<String>,
    pub availability: MediaAvailabilityDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct TextDiffSegmentDto {
    pub kind: TextDiffKindDto,
    pub text: String,
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
    pub suspended: bool,
    pub hint: Option<LocalizedTextDto>,
    pub prompt_media: Vec<StudyMediaDto>,
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
    pub source_segments: Vec<RevealSegmentDto>,
    pub expected_answer: String,
    pub raw_response: String,
    pub normalized_response: String,
    pub comparison: ComparisonResultDto,
    pub difference: Vec<TextDiffSegmentDto>,
    pub suggested_grade: GradeDto,
    pub grade_previews: Vec<GradePreviewDto>,
    pub annotations: Vec<StudyAnnotationDto>,
    pub explanation: Option<LocalizedTextDto>,
    pub answer_media: Vec<StudyMediaDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct RevealSegmentDto {
    pub text: String,
    pub highlighted: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct GradePreviewDto {
    pub grade: GradeDto,
    pub due_at: String,
    pub interval_seconds: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct GradeReviewRequest {
    pub review_event_id: String,
    pub card_id: String,
    pub card_content_version: u32,
    pub schedule_version: u32,
    pub raw_response: String,
    pub chosen_grade: GradeDto,
    pub response_duration_ms: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct GradeReviewResultDto {
    pub review_event_id: String,
    pub schedule_version: u32,
    pub due_at: String,
    pub interval_seconds: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct StudyQueueEntryDto {
    pub card_id: String,
    pub card_content_version: u32,
    pub schedule_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ReconcileStudyQueueRequest {
    pub deck_id: String,
    #[ts(type = "number")]
    pub now_ms: i64,
    #[ts(type = "number")]
    pub day_start_ms: i64,
    #[ts(type = "number")]
    pub day_end_ms: i64,
    pub entries: Vec<StudyQueueEntryDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct SuspendCardRequest {
    pub card_id: String,
    pub card_content_version: u32,
    pub schedule_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct UndoReviewRequest {
    pub undo_event_id: String,
    pub card_id: String,
    pub card_content_version: u32,
    pub schedule_version: u32,
    pub review_event_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct UndoReviewResultDto {
    pub undo_event_id: String,
    pub schedule_version: u32,
    pub due_at: String,
    pub interval_seconds: u32,
    pub completed_reviews: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct SchedulerSettingsDto {
    pub deck_id: String,
    pub scheduling_mode: SchedulingModeDto,
    pub collection_daily_time_budget_minutes: u32,
    pub deck_daily_time_budget_minutes: Option<u32>,
    pub effective_daily_time_budget_minutes: u32,
    pub budget_source: BudgetSourceDto,
    pub target_retention_basis_points: u16,
    pub new_cards_per_day: u32,
    pub maximum_interval_days: u32,
    pub day_boundary_minutes: u16,
    pub controller_backlog_exceeds_budget: bool,
    pub controller_explanation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct UpdateSchedulerSettingsRequest {
    pub deck_id: String,
    pub scheduling_mode: SchedulingModeDto,
    pub collection_daily_time_budget_minutes: u32,
    pub deck_daily_time_budget_minutes: Option<u32>,
    pub target_retention_basis_points: u16,
    pub new_cards_per_day: u32,
    pub maximum_interval_days: u32,
    pub day_boundary_minutes: u16,
    #[ts(type = "number")]
    pub now_ms: i64,
    #[ts(type = "number")]
    pub day_start_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct SchedulerPolicyPreviewDto {
    pub effective_daily_time_budget_minutes: u32,
    pub budget_source: BudgetSourceDto,
    pub target_retention_basis_points: u16,
    pub new_cards_per_day: u32,
    pub backlog_exceeds_budget: bool,
    pub explanation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ImportSchedulerParametersRequest {
    pub deck_id: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct SchedulerParametersExportDto {
    pub path: String,
}

const SCHEDULER_PARAMETER_FORMAT: &str = "meiki-scheduler-parameters";
const SCHEDULER_PARAMETER_VERSION: u32 = 1;
const MINIMUM_CONTROLLER_RESPONSE_SAMPLES: u64 = 8;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SchedulerParameterFile {
    format: String,
    version: u32,
    parameter_set_id: String,
    engine_version: String,
    parameters: Vec<f64>,
}

/// Supplies wall-clock time to application use cases.
pub trait Clock: std::fmt::Debug + Send + Sync {
    /// Returns the current Unix timestamp in milliseconds.
    fn now_ms(&self) -> i64;
}

/// Supplies opaque aggregate identities to application use cases.
pub trait IdSource: std::fmt::Debug + Send + Sync {
    /// Returns a fresh identifier for the named purpose.
    fn next_id(&self, purpose: &'static str) -> String;
}

#[derive(Debug)]
struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        Utc::now().timestamp_millis()
    }
}

#[derive(Debug)]
struct RandomIdSource;

impl IdSource for RandomIdSource {
    fn next_id(&self, _purpose: &'static str) -> String {
        Uuid::new_v4().to_string()
    }
}

/// Narrow runtime inputs used to make application journeys deterministic.
#[derive(Clone, Debug)]
pub struct ApplicationRuntime {
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdSource>,
}

impl ApplicationRuntime {
    /// Creates runtime inputs from a clock and identity source.
    pub fn new(clock: impl Clock + 'static, ids: impl IdSource + 'static) -> Self {
        Self {
            clock: Arc::new(clock),
            ids: Arc::new(ids),
        }
    }
}

impl Default for ApplicationRuntime {
    fn default() -> Self {
        Self::new(SystemClock, RandomIdSource)
    }
}

#[derive(Clone, Debug)]
pub struct ApplicationService {
    collection_path: PathBuf,
    runtime: ApplicationRuntime,
}

impl ApplicationService {
    /// Creates a use-case service for one collection database path.
    pub fn new(collection_path: impl Into<PathBuf>) -> Self {
        Self::with_runtime(collection_path, ApplicationRuntime::default())
    }

    /// Creates a use-case service with explicit deterministic runtime inputs.
    pub fn with_runtime(collection_path: impl Into<PathBuf>, runtime: ApplicationRuntime) -> Self {
        Self {
            collection_path: collection_path.into(),
            runtime,
        }
    }

    pub(crate) fn now_ms(&self) -> i64 {
        self.runtime.clock.now_ms()
    }

    pub(crate) fn next_id(&self, purpose: &'static str) -> String {
        self.runtime.ids.next_id(purpose)
    }

    #[cfg(test)]
    fn seed_test_collection(&self, now_ms: i64) -> Result<StudyCardDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        storage.seed_walking_skeleton(now_ms)?;
        self.study_card_dto(&storage, meiki_storage::SAMPLE_CARD_ID)
    }

    /// Imports one local audio or image into the collection's content-addressed store.
    ///
    /// # Errors
    ///
    /// Returns an error when the path cannot be read, the file signature is
    /// unsupported, or the requested role does not match the detected media kind.
    pub fn import_media(
        &self,
        request: &ImportMediaRequest,
    ) -> Result<StudyMediaDto, ApplicationError> {
        let store = self.media_store();
        let imported = store.import_file(Path::new(&request.path))?;
        let kind = match imported.kind {
            DetectedMediaKind::Audio => MediaKind::Audio,
            DetectedMediaKind::Image => MediaKind::Image,
        };
        let role: MediaRole = request.role.into();
        if let Err(error) = ensure_role_matches_kind(role, kind) {
            if !imported.deduplicated {
                store.remove(&imported.content_hash)?;
            }
            return Err(error);
        }
        let media = MediaReference {
            id: self.next_id("media-reference"),
            content_hash: imported.content_hash,
            kind,
            role,
            media_type: imported.media_type,
            byte_size: imported.byte_size,
            original_file_name: Some(imported.original_file_name),
            alt_text: None,
            width: imported.width,
            height: imported.height,
            duration_ms: imported.duration_ms,
            language_tag: request.language_tag.clone(),
            direction: request.direction.into(),
            created_at_ms: self.now_ms(),
        };
        Ok(self.study_media_dto(&media))
    }

    /// Restores a study card from the collection.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when the card cannot be loaded.
    pub fn get_study_card(&self, card_id: &str) -> Result<StudyCardDto, ApplicationError> {
        let storage = self.open_storage()?;
        self.study_card_dto(&storage, card_id)
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
        let comparison = compare_answer_with_options(
            &stored.cloze.answer,
            &stored.cloze.accepted_answers,
            &request.raw_response,
            &answer_options(&storage, &stored)?,
        );
        let suggested_grade = suggested_grade(comparison.result);
        let previewed_at_ms = self.now_ms();
        let (engine, _, _) = scheduler_for_card(&storage, &stored)?;
        let grade_previews = [
            GradeDto::Again,
            GradeDto::Hard,
            GradeDto::Good,
            GradeDto::Easy,
        ]
        .into_iter()
        .map(|grade| {
            let decision = engine.review(&stored.schedule, grade.into(), previewed_at_ms)?;
            Ok(GradePreviewDto {
                grade,
                due_at: timestamp_string(decision.next_state.due_at_ms)?,
                interval_seconds: desktop_u32(
                    decision.next_state.interval_seconds,
                    "preview interval seconds",
                )?,
            })
        })
        .collect::<Result<Vec<_>, ApplicationError>>()?;

        Ok(RevealDto {
            card_id: stored.card.id,
            card_content_version: desktop_u32(stored.card.content_version, "card content version")?,
            schedule_version: desktop_u32(stored.schedule.version, "schedule version")?,
            full_source: render_source(&stored.source_item, None),
            source_segments: reveal_segments(&stored.source_item, &stored.cloze.id),
            expected_answer: stored.cloze.answer,
            raw_response: request.raw_response.clone(),
            normalized_response: comparison.normalized_response,
            comparison: comparison.result.into(),
            difference: comparison.difference.into_iter().map(Into::into).collect(),
            suggested_grade: suggested_grade.into(),
            grade_previews,
            annotations: study_annotations(
                &stored.source_item.annotations,
                &stored.cloze.annotations,
            ),
            explanation: stored
                .cloze
                .explanation
                .as_ref()
                .or(stored.source_item.explanation.as_ref())
                .map(localized_text_dto),
            answer_media: self.study_media(
                &stored
                    .source_item
                    .media
                    .iter()
                    .chain(&stored.cloze.media)
                    .filter(|media| {
                        matches!(media.role, MediaRole::AnswerAudio | MediaRole::RevealImage)
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
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
        self.grade_review_at(request, self.now_ms())
    }

    fn grade_review_at(
        &self,
        request: &GradeReviewRequest,
        reviewed_at_ms: i64,
    ) -> Result<GradeReviewResultDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        let stored = storage.load_study_card(&request.card_id)?;
        if let Some(existing) = storage
            .review_events(&request.card_id)?
            .into_iter()
            .find(|event| event.id == request.review_event_id)
        {
            if existing.kind != ReviewEventKind::Review
                || existing.card_content_version != u64::from(request.card_content_version)
                || existing.previous_schedule.version != u64::from(request.schedule_version)
                || existing.raw_response != request.raw_response
                || existing.chosen_grade != Grade::from(request.chosen_grade)
                || existing.response_duration_ms != u64::from(request.response_duration_ms)
            {
                return Err(ApplicationError::StaleCard);
            }
            return grade_review_result(&existing);
        }
        ensure_card_is_current(
            &stored,
            u64::from(request.card_content_version),
            u64::from(request.schedule_version),
        )?;
        if stored.schedule.lifecycle == meiki_domain::CardLifecycle::Introduced
            && stored.schedule.due_at_ms > reviewed_at_ms
        {
            return Err(ApplicationError::CardNotDue);
        }

        let comparison = compare_answer_with_options(
            &stored.cloze.answer,
            &stored.cloze.accepted_answers,
            &request.raw_response,
            &answer_options(&storage, &stored)?,
        );
        let suggested = suggested_grade(comparison.result);
        let (engine, profile, _) = scheduler_for_card(&storage, &stored)?;
        let decision = engine.review(
            &stored.schedule,
            request.chosen_grade.into(),
            reviewed_at_ms,
        )?;
        let mut next_schedule = decision.next_state;
        next_schedule.last_review_event_id = Some(request.review_event_id.clone());

        let event = ReviewEvent {
            id: request.review_event_id.clone(),
            card_id: stored.card.id,
            card_content_version: stored.card.content_version,
            kind: ReviewEventKind::Review,
            undoes_review_event_id: None,
            raw_response: request.raw_response.clone(),
            normalized_response: comparison.normalized_response,
            comparison: comparison.result,
            suggested_grade: suggested,
            chosen_grade: request.chosen_grade.into(),
            grade_overridden: request.chosen_grade != GradeDto::from(suggested),
            response_duration_ms: u64::from(request.response_duration_ms),
            reviewed_at_ms,
            scheduler_version: decision.scheduler_version.to_owned(),
            scheduler_parameter_set_id: Some(profile.active_parameter_set_id),
            target_retention_basis_points: decision.target_retention_basis_points,
            previous_schedule: stored.schedule,
            next_schedule,
        };
        let committed = storage.commit_review(&event)?;

        Ok(GradeReviewResultDto {
            review_event_id: request.review_event_id.clone(),
            schedule_version: desktop_u32(committed.version, "schedule version")?,
            due_at: timestamp_string(committed.due_at_ms)?,
            interval_seconds: desktop_u32(committed.interval_seconds, "interval seconds")?,
        })
    }

    /// Suspends a card after checking the versions observed by the study view.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::StaleCard`] when the card changed or
    /// another [`ApplicationError`] when persistence fails.
    pub fn suspend_card(
        &self,
        request: &SuspendCardRequest,
    ) -> Result<StudyCardDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        let stored = storage.load_study_card(&request.card_id)?;
        ensure_card_is_current(
            &stored,
            u64::from(request.card_content_version),
            u64::from(request.schedule_version),
        )?;
        let mut card = stored.card;
        card.suspended = true;
        card.updated_at_ms = self.now_ms();
        storage.update_card(&card)?;
        self.study_card_dto(&storage, &request.card_id)
    }

    /// Compensates the latest committed review and returns the restored queue
    /// projection.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::StaleCard`] when the observed versions no
    /// longer match or another [`ApplicationError`] when there is no matching
    /// latest review to undo.
    pub fn undo_review(
        &self,
        request: &UndoReviewRequest,
    ) -> Result<UndoReviewResultDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        let stored = storage.load_study_card(&request.card_id)?;
        if let Some(existing) = storage
            .review_events(&request.card_id)?
            .into_iter()
            .find(|event| event.id == request.undo_event_id)
        {
            if existing.kind != ReviewEventKind::Undo
                || existing.card_content_version != u64::from(request.card_content_version)
                || existing.previous_schedule.version != u64::from(request.schedule_version)
                || existing.undoes_review_event_id.as_deref()
                    != Some(request.review_event_id.as_str())
                || stored.schedule.last_review_event_id.as_deref()
                    != Some(request.undo_event_id.as_str())
            {
                return Err(ApplicationError::StaleCard);
            }
            return undo_review_result(
                &storage,
                &request.card_id,
                request.undo_event_id.clone(),
                &existing.next_schedule,
            );
        }
        ensure_card_is_current(
            &stored,
            u64::from(request.card_content_version),
            u64::from(request.schedule_version),
        )?;
        let restored = storage.undo_last_review(
            &request.card_id,
            &request.review_event_id,
            &request.undo_event_id,
            self.now_ms(),
        )?;
        undo_review_result(
            &storage,
            &request.card_id,
            request.undo_event_id.clone(),
            &restored,
        )
    }

    /// Loads basic and advanced scheduling controls for a deck.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when the deck or scheduler profile cannot
    /// be loaded.
    pub fn get_scheduler_settings(
        &self,
        deck_id: &str,
    ) -> Result<SchedulerSettingsDto, ApplicationError> {
        let storage = self.open_storage()?;
        scheduler_settings_dto(&storage, deck_id)
    }

    /// Previews the effective policy for proposed controls without persisting
    /// any settings or schedule state.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when the proposed controls or workload
    /// state are invalid.
    pub fn preview_scheduler_policy(
        &self,
        request: &UpdateSchedulerSettingsRequest,
    ) -> Result<SchedulerPolicyPreviewDto, ApplicationError> {
        validate_scheduler_settings_request(request)?;
        let storage = self.open_storage()?;
        let profile = storage.get_scheduler_profile(&request.deck_id)?;
        let (budget, source) = requested_budget(request);
        let (target, new_cards, backlog, explanation) = if request.scheduling_mode
            == SchedulingModeDto::Automatic
        {
            let (decision, _) = policy_decision(
                &storage,
                &request.deck_id,
                budget,
                profile.controller_target_retention_basis_points,
                profile.controller_target_retention_basis_points,
                request.now_ms,
            )?;
            (
                decision.target_retention_basis_points,
                decision.new_cards_per_day,
                decision.backlog_exceeds_budget,
                decision.explanation,
            )
        } else {
            (
                request.target_retention_basis_points,
                request.new_cards_per_day,
                false,
                format!(
                    "{budget} min/day\nTarget retention: {}%\nNew cards today: {}\nReason: Expert mode keeps these manual policy values.",
                    format_retention(request.target_retention_basis_points),
                    request.new_cards_per_day
                ),
            )
        };
        Ok(SchedulerPolicyPreviewDto {
            effective_daily_time_budget_minutes: budget,
            budget_source: source,
            target_retention_basis_points: target,
            new_cards_per_day: new_cards,
            backlog_exceeds_budget: backlog,
            explanation,
        })
    }

    /// Updates scheduling controls without rescheduling existing cards.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when controls are unsafe or persistence
    /// fails.
    pub fn update_scheduler_settings(
        &self,
        request: &UpdateSchedulerSettingsRequest,
    ) -> Result<SchedulerSettingsDto, ApplicationError> {
        validate_scheduler_settings_request(request)?;
        let mut storage = self.open_storage()?;
        storage.update_collection_scheduling_settings(&CollectionSchedulingSettings {
            daily_time_budget_minutes: request.collection_daily_time_budget_minutes,
            updated_at_ms: request.now_ms,
        })?;
        let mut deck = storage.get_deck(&request.deck_id)?;
        deck.settings = StudySettingsOverride {
            target_retention_basis_points: Some(request.target_retention_basis_points),
            new_cards_per_day: Some(request.new_cards_per_day),
            maximum_interval_days: Some(request.maximum_interval_days),
        };
        deck.updated_at_ms = request.now_ms;
        storage.update_deck(&deck)?;

        let mut profile = storage.get_scheduler_profile(&request.deck_id)?;
        profile.scheduling_mode = request.scheduling_mode.into();
        profile.deck_daily_time_budget_minutes = request.deck_daily_time_budget_minutes;
        profile.day_boundary_minutes = request.day_boundary_minutes;
        profile.updated_at_ms = request.now_ms;
        if profile.scheduling_mode == SchedulingMode::Expert {
            profile.controller_explanation =
                "Expert mode keeps manual scheduling-policy values.".into();
        }
        storage.update_scheduler_profile(&profile)?;
        if profile.scheduling_mode == SchedulingMode::Automatic {
            evaluate_and_store_policy(
                &mut storage,
                &request.deck_id,
                request.now_ms,
                request.day_start_ms,
                true,
            )?;
        }
        scheduler_settings_dto(&storage, &request.deck_id)
    }

    /// Imports and activates a strictly validated, versioned parameter file.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] unless the deck is in Expert mode or the
    /// file fails format, engine, or parameter validation.
    pub fn import_scheduler_parameters(
        &self,
        request: &ImportSchedulerParametersRequest,
    ) -> Result<SchedulerSettingsDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        let profile = storage.get_scheduler_profile(&request.deck_id)?;
        if profile.scheduling_mode != SchedulingMode::Expert {
            return Err(ApplicationError::InvalidSchedulerParameterFile(
                "parameter import is available only in Expert mode".into(),
            ));
        }
        let bytes = fs::read(&request.path).map_err(ApplicationError::SchedulerParameterIo)?;
        if bytes.len() > 64 * 1024 {
            return Err(ApplicationError::InvalidSchedulerParameterFile(
                "parameter file exceeds 64 KiB".into(),
            ));
        }
        let imported: SchedulerParameterFile =
            serde_json::from_slice(&bytes).map_err(ApplicationError::SchedulerParameterJson)?;
        validate_scheduler_parameter_file(&imported)?;
        let deck = storage.get_deck(&request.deck_id)?;
        let settings = expert_study_settings(&deck);
        Fsrs7Engine::from_parameters(
            SchedulerConfig {
                target_retention_basis_points: settings.target_retention_basis_points,
                maximum_interval_days: settings.maximum_interval_days,
            },
            &imported.parameters,
        )?;
        let now_ms = self.now_ms();
        let parameter_set = SchedulerParameterSet {
            id: format!(
                "fsrs7-import-{}-{}-{}",
                sanitize_parameter_id(&imported.parameter_set_id),
                now_ms,
                self.next_id("scheduler-parameter-set")
            ),
            engine_version: imported.engine_version,
            parameters: imported.parameters,
            created_at_ms: now_ms,
        };
        storage.adopt_scheduler_parameter_set(&request.deck_id, &parameter_set, now_ms)?;
        scheduler_settings_dto(&storage, &request.deck_id)
    }

    /// Exports the active memory-model parameter set as versioned JSON.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when the deck is not in Expert mode or the
    /// file cannot be serialized and written.
    pub fn export_scheduler_parameters(
        &self,
        deck_id: &str,
    ) -> Result<SchedulerParametersExportDto, ApplicationError> {
        let storage = self.open_storage()?;
        let profile = storage.get_scheduler_profile(deck_id)?;
        if profile.scheduling_mode != SchedulingMode::Expert {
            return Err(ApplicationError::InvalidSchedulerParameterFile(
                "parameter export is available only in Expert mode".into(),
            ));
        }
        let parameters = storage.get_scheduler_parameter_set(&profile.active_parameter_set_id)?;
        let file = SchedulerParameterFile {
            format: SCHEDULER_PARAMETER_FORMAT.into(),
            version: SCHEDULER_PARAMETER_VERSION,
            parameter_set_id: parameters.id,
            engine_version: parameters.engine_version,
            parameters: parameters.parameters,
        };
        let path = scheduler_parameters_path(
            &self.collection_path,
            self.now_ms(),
            &self.next_id("scheduler-parameter-export"),
        );
        let json =
            serde_json::to_vec_pretty(&file).map_err(ApplicationError::SchedulerParameterJson)?;
        fs::write(&path, json).map_err(ApplicationError::SchedulerParameterIo)?;
        Ok(SchedulerParametersExportDto {
            path: path.to_string_lossy().into_owned(),
        })
    }

    fn open_storage(&self) -> Result<Storage, ApplicationError> {
        if let Some(parent) = self.collection_path.parent() {
            fs::create_dir_all(parent).map_err(ApplicationError::CollectionDirectory)?;
        }
        Ok(Storage::open(&self.collection_path)?)
    }

    fn media_store(&self) -> MediaStore {
        MediaStore::new(self.collection_path.with_extension("media"))
    }

    fn study_card_dto(
        &self,
        storage: &Storage,
        card_id: &str,
    ) -> Result<StudyCardDto, ApplicationError> {
        let stored = storage.load_study_card(card_id)?;
        let deck = storage.get_deck(&stored.source_item.deck_id)?;
        let completed_reviews = storage.review_count(card_id)?;
        Ok(StudyCardDto {
            card_id: stored.card.id,
            card_content_version: desktop_u32(stored.card.content_version, "card content version")?,
            schedule_version: desktop_u32(stored.schedule.version, "schedule version")?,
            prompt: render_source(&stored.source_item, Some(&stored.cloze.id)),
            language_tag: stored
                .cloze
                .language_tag
                .or(stored.source_item.language_tag)
                .or(deck.language_tag),
            direction: resolve_direction(
                stored.cloze.direction,
                stored.source_item.direction,
                deck.direction,
            )
            .into(),
            due_at: timestamp_string(stored.schedule.due_at_ms)?,
            completed_reviews: desktop_u32(completed_reviews, "completed review count")?,
            suspended: stored.card.suspended,
            hint: stored.cloze.hint.as_ref().map(localized_text_dto),
            prompt_media: self.study_media(
                &stored
                    .source_item
                    .media
                    .iter()
                    .chain(&stored.cloze.media)
                    .filter(|media| media.role == MediaRole::PromptAudio)
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
        })
    }

    fn study_media(&self, media: &[MediaReference]) -> Vec<StudyMediaDto> {
        media
            .iter()
            .map(|media| self.study_media_dto(media))
            .collect()
    }

    fn study_media_dto(&self, media: &MediaReference) -> StudyMediaDto {
        let role_is_valid = ensure_role_matches_kind(media.role, media.kind).is_ok();
        let media_type_is_valid = media_type_matches_kind(&media.media_type, media.kind);
        let (asset_path, availability) = if !role_is_valid || !media_type_is_valid {
            (None, MediaAvailabilityDto::Unsupported)
        } else {
            match self.media_store().resolve(&media.content_hash) {
                Ok(path) => (
                    Some(path.to_string_lossy().into_owned()),
                    MediaAvailabilityDto::Ready,
                ),
                Err(MediaError::MissingObject(_)) => (None, MediaAvailabilityDto::Missing),
                Err(MediaError::InvalidHash(_) | MediaError::UnsupportedFormat) => {
                    (None, MediaAvailabilityDto::Unsupported)
                }
                Err(_) => (None, MediaAvailabilityDto::Corrupt),
            }
        };
        StudyMediaDto {
            id: media.id.clone(),
            content_hash: media.content_hash.clone(),
            kind: media.kind.into(),
            role: media.role.into(),
            media_type: media.media_type.clone(),
            byte_size: media.byte_size,
            original_file_name: media.original_file_name.clone(),
            alt_text: media.alt_text.clone(),
            width: media.width,
            height: media.height,
            duration_ms: media.duration_ms,
            language_tag: media.language_tag.clone(),
            direction: media.direction.into(),
            asset_path,
            availability,
        }
    }
}

fn scheduler_for_card(
    storage: &Storage,
    card: &StoredStudyCard,
) -> Result<(Fsrs7Engine, SchedulerProfile, StudySettings), ApplicationError> {
    let deck = storage.get_deck(&card.source_item.deck_id)?;
    let profile = storage.get_scheduler_profile(&deck.id)?;
    let settings = effective_study_settings(&deck, &profile);
    let engine = scheduler_from_profile(storage, &profile, &settings)?;
    Ok((engine, profile, settings))
}

fn scheduler_from_profile(
    storage: &Storage,
    profile: &SchedulerProfile,
    settings: &StudySettings,
) -> Result<Fsrs7Engine, ApplicationError> {
    if profile.engine_version != ENGINE_VERSION {
        return Err(ApplicationError::UnsupportedScheduler(
            profile.engine_version.clone(),
        ));
    }
    let parameter_set = storage.get_scheduler_parameter_set(&profile.active_parameter_set_id)?;
    if parameter_set.engine_version != profile.engine_version {
        return Err(ApplicationError::UnsupportedScheduler(
            parameter_set.engine_version,
        ));
    }
    Ok(Fsrs7Engine::from_parameters(
        SchedulerConfig {
            target_retention_basis_points: settings.target_retention_basis_points,
            maximum_interval_days: settings.maximum_interval_days,
        },
        &parameter_set.parameters,
    )?)
}

fn scheduler_settings_dto(
    storage: &Storage,
    deck_id: &str,
) -> Result<SchedulerSettingsDto, ApplicationError> {
    let deck = storage.get_deck(deck_id)?;
    let profile = storage.get_scheduler_profile(deck_id)?;
    let collection = storage.collection_scheduling_settings()?;
    let resolved = effective_study_settings(&deck, &profile);
    let (effective_budget, budget_source) = effective_budget(&collection, &profile);
    let controller_explanation = if profile.controller_explanation.trim().is_empty() {
        format!(
            "{effective_budget} min/day\nTarget retention: {}%\nNew cards today: {}\nReason: the automatic policy will refine this plan from local schedule state.",
            format_retention(resolved.target_retention_basis_points),
            resolved.new_cards_per_day
        )
    } else {
        profile.controller_explanation.clone()
    };
    Ok(SchedulerSettingsDto {
        deck_id: deck_id.to_owned(),
        scheduling_mode: profile.scheduling_mode.into(),
        collection_daily_time_budget_minutes: collection.daily_time_budget_minutes,
        deck_daily_time_budget_minutes: profile.deck_daily_time_budget_minutes,
        effective_daily_time_budget_minutes: effective_budget,
        budget_source,
        target_retention_basis_points: resolved.target_retention_basis_points,
        new_cards_per_day: resolved.new_cards_per_day,
        maximum_interval_days: resolved.maximum_interval_days,
        day_boundary_minutes: profile.day_boundary_minutes,
        controller_backlog_exceeds_budget: profile.controller_backlog_exceeds_budget,
        controller_explanation,
    })
}

pub(crate) fn effective_study_settings(
    deck: &meiki_domain::Deck,
    profile: &SchedulerProfile,
) -> StudySettings {
    match profile.scheduling_mode {
        SchedulingMode::Automatic => StudySettings {
            target_retention_basis_points: profile.controller_target_retention_basis_points,
            new_cards_per_day: profile.controller_new_cards_per_day,
            maximum_interval_days: StudySettings::default().maximum_interval_days,
        },
        SchedulingMode::Expert => expert_study_settings(deck),
    }
}

fn expert_study_settings(deck: &meiki_domain::Deck) -> StudySettings {
    StudySettings::resolve(&StudySettings::default(), &deck.settings)
}

pub(crate) fn effective_budget(
    collection: &CollectionSchedulingSettings,
    profile: &SchedulerProfile,
) -> (u32, BudgetSourceDto) {
    profile.deck_daily_time_budget_minutes.map_or(
        (
            collection.daily_time_budget_minutes,
            BudgetSourceDto::CollectionBudget,
        ),
        |budget| (budget, BudgetSourceDto::DeckOverride),
    )
}

fn requested_budget(request: &UpdateSchedulerSettingsRequest) -> (u32, BudgetSourceDto) {
    request.deck_daily_time_budget_minutes.map_or(
        (
            request.collection_daily_time_budget_minutes,
            BudgetSourceDto::CollectionBudget,
        ),
        |budget| (budget, BudgetSourceDto::DeckOverride),
    )
}

fn validate_scheduler_settings_request(
    request: &UpdateSchedulerSettingsRequest,
) -> Result<(), ApplicationError> {
    Fsrs7Engine::new(SchedulerConfig {
        target_retention_basis_points: request.target_retention_basis_points,
        maximum_interval_days: request.maximum_interval_days,
    })?;
    if request.deck_id.trim().is_empty()
        || !(1..=1_440).contains(&request.collection_daily_time_budget_minutes)
        || request
            .deck_daily_time_budget_minutes
            .is_some_and(|value| !(1..=1_440).contains(&value))
        || request.new_cards_per_day > 10_000
        || request.day_boundary_minutes >= 1_440
        || request.day_start_ms > request.now_ms
        || request.now_ms.saturating_sub(request.day_start_ms) > 28 * 60 * 60 * 1_000
    {
        return Err(ApplicationError::Scheduler(SchedulerError::InvalidState(
            "settings controls are outside safe bounds",
        )));
    }
    Ok(())
}

fn policy_decision(
    storage: &Storage,
    deck_id: &str,
    budget_minutes: u32,
    current_target: u16,
    previous_target: u16,
    now_ms: i64,
) -> Result<(AutomaticPolicyDecision, SchedulingWorkload), ApplicationError> {
    let horizon_ms = i64::from(FORECAST_DAYS)
        .checked_mul(86_400_000)
        .and_then(|duration| now_ms.checked_add(duration))
        .ok_or(ApplicationError::Scheduler(SchedulerError::InvalidState(
            "forecast horizon is outside supported time",
        )))?;
    let workload = storage.scheduling_workload(deck_id, now_ms, horizon_ms)?;
    let response_seconds = controller_response_seconds(&workload);
    Ok((
        automatic_policy(AutomaticPolicyInput {
            daily_budget_minutes: budget_minutes,
            due_cards_now: workload.due_cards_now,
            forecast_review_occurrences: workload.forecast_review_occurrences,
            response_seconds,
            unseen_cards: workload.unseen_cards,
            current_target_retention_basis_points: current_target,
            previous_target_retention_basis_points: previous_target,
        }),
        workload,
    ))
}

fn controller_response_seconds(workload: &SchedulingWorkload) -> u64 {
    if workload.response_duration_samples < MINIMUM_CONTROLLER_RESPONSE_SAMPLES {
        return 20;
    }
    workload
        .median_response_duration_ms
        .map_or(20, |milliseconds| {
            milliseconds.div_ceil(1_000).clamp(5, 120)
        })
}

pub(crate) fn evaluate_and_store_policy(
    storage: &mut Storage,
    deck_id: &str,
    now_ms: i64,
    day_start_ms: i64,
    force: bool,
) -> Result<(), ApplicationError> {
    let mut profile = storage.get_scheduler_profile(deck_id)?;
    if profile.scheduling_mode != SchedulingMode::Automatic {
        return Ok(());
    }
    let collection = storage.collection_scheduling_settings()?;
    let (budget, _) = effective_budget(&collection, &profile);
    let (decision, workload) = policy_decision(
        storage,
        deck_id,
        budget,
        profile.controller_target_retention_basis_points,
        profile.controller_target_retention_basis_points,
        now_ms,
    )?;
    let history_changed =
        workload.review_count >= profile.controller_review_count.saturating_add(32);
    let unseen_changed = workload.unseen_cards != profile.controller_unseen_count;
    if !force
        && profile.controller_last_evaluated_day_start_ms == Some(day_start_ms)
        && !history_changed
        && !unseen_changed
    {
        return Ok(());
    }
    profile.controller_version = CONTROLLER_VERSION.into();
    profile.controller_target_retention_basis_points = decision.target_retention_basis_points;
    profile.controller_new_cards_per_day = decision.new_cards_per_day;
    profile.controller_last_evaluated_day_start_ms = Some(day_start_ms);
    profile.controller_review_count = workload.review_count;
    profile.controller_unseen_count = workload.unseen_cards;
    profile.controller_forecast_review_seconds_per_day = decision.forecast_review_seconds_per_day;
    profile.controller_backlog_exceeds_budget = decision.backlog_exceeds_budget;
    profile.controller_explanation = decision.explanation;
    profile.updated_at_ms = now_ms;
    storage.update_scheduler_profile(&profile)?;
    Ok(())
}

fn validate_scheduler_parameter_file(
    file: &SchedulerParameterFile,
) -> Result<(), ApplicationError> {
    if file.format != SCHEDULER_PARAMETER_FORMAT
        || file.version != SCHEDULER_PARAMETER_VERSION
        || file.parameter_set_id.trim().is_empty()
        || file.parameter_set_id.len() > 128
        || !file
            .parameter_set_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        || file.engine_version != ENGINE_VERSION
    {
        return Err(ApplicationError::InvalidSchedulerParameterFile(
            "format, version, identifier, or engine is unsupported".into(),
        ));
    }
    Ok(())
}

fn sanitize_parameter_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(64)
        .collect::<String>();
    if sanitized.is_empty() {
        "imported".into()
    } else {
        sanitized
    }
}

fn scheduler_parameters_path(collection_path: &Path, now_ms: i64, suffix: &str) -> PathBuf {
    let name = collection_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("collection.db");
    collection_path.with_file_name(format!(
        "{name}.scheduler-parameters-{now_ms}-{suffix}.json"
    ))
}

fn format_retention(target: u16) -> String {
    if target % 100 == 0 {
        (target / 100).to_string()
    } else {
        format!("{}.{:02}", target / 100, target % 100)
    }
}

fn grade_review_result(event: &ReviewEvent) -> Result<GradeReviewResultDto, ApplicationError> {
    Ok(GradeReviewResultDto {
        review_event_id: event.id.clone(),
        schedule_version: desktop_u32(event.next_schedule.version, "schedule version")?,
        due_at: timestamp_string(event.next_schedule.due_at_ms)?,
        interval_seconds: desktop_u32(event.next_schedule.interval_seconds, "interval seconds")?,
    })
}

fn undo_review_result(
    storage: &Storage,
    card_id: &str,
    undo_event_id: String,
    schedule: &meiki_domain::ScheduleState,
) -> Result<UndoReviewResultDto, ApplicationError> {
    Ok(UndoReviewResultDto {
        undo_event_id,
        schedule_version: desktop_u32(schedule.version, "schedule version")?,
        due_at: timestamp_string(schedule.due_at_ms)?,
        interval_seconds: desktop_u32(schedule.interval_seconds, "interval seconds")?,
        completed_reviews: desktop_u32(storage.review_count(card_id)?, "completed review count")?,
    })
}

const fn resolve_direction(cloze: Direction, source: Direction, deck: Direction) -> Direction {
    if !matches!(cloze, Direction::Auto) {
        cloze
    } else if !matches!(source, Direction::Auto) {
        source
    } else {
        deck
    }
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

fn answer_options(
    storage: &Storage,
    stored: &StoredStudyCard,
) -> Result<ComparisonOptions, ApplicationError> {
    let deck = storage.get_deck(&stored.source_item.deck_id)?;
    let policy = stored.cloze.matching_policy.unwrap_or(deck.matching_policy);
    Ok(match policy {
        MatchingPolicy::Strict => ComparisonOptions::default(),
        MatchingPolicy::Forgiving => ComparisonOptions {
            case: CaseSensitivity::UnicodeLowercase,
            diacritics: DiacriticSensitivity::Ignore,
            punctuation: PunctuationSensitivity::Ignore,
            whitespace: WhitespaceSensitivity::Collapse,
            width: WidthSensitivity::FoldCompatibility,
            ..ComparisonOptions::default()
        },
    })
}

fn render_source(source: &SourceItem, hidden_cloze_id: Option<&str>) -> String {
    source
        .segments
        .iter()
        .map(|segment| match &segment.content {
            SegmentContent::Cloze { cloze_id, text: _ }
                if hidden_cloze_id == Some(cloze_id.as_str()) =>
            {
                "[…]"
            }
            SegmentContent::Text(text) | SegmentContent::Cloze { text, .. } => text.as_str(),
        })
        .collect()
}

fn reveal_segments(source: &SourceItem, active_cloze_id: &str) -> Vec<RevealSegmentDto> {
    source
        .segments
        .iter()
        .map(|segment| match &segment.content {
            SegmentContent::Text(text) => RevealSegmentDto {
                text: text.clone(),
                highlighted: false,
            },
            SegmentContent::Cloze { cloze_id, text } => RevealSegmentDto {
                text: text.clone(),
                highlighted: cloze_id == active_cloze_id,
            },
        })
        .collect()
}

fn localized_text_dto(text: &LocalizedText) -> LocalizedTextDto {
    LocalizedTextDto {
        value: text.value.clone(),
        language_tag: text.language_tag.clone(),
        direction: text.direction.into(),
    }
}

fn study_annotations(source: &[Annotation], cloze: &[Annotation]) -> Vec<StudyAnnotationDto> {
    source
        .iter()
        .chain(cloze)
        .map(|annotation| StudyAnnotationDto {
            label: annotation.label.clone(),
            value: annotation.value.clone(),
            language_tag: annotation.language_tag.clone(),
            direction: annotation.direction.into(),
        })
        .collect()
}

fn ensure_role_matches_kind(role: MediaRole, kind: MediaKind) -> Result<(), ApplicationError> {
    if matches!(
        (role, kind),
        (
            MediaRole::PromptAudio | MediaRole::AnswerAudio,
            MediaKind::Audio
        ) | (MediaRole::RevealImage, MediaKind::Image)
    ) {
        Ok(())
    } else {
        Err(ApplicationError::InvalidAuthoring(
            "the selected media file does not match its prompt, answer, or reveal role".to_owned(),
        ))
    }
}

fn media_type_matches_kind(media_type: &str, kind: MediaKind) -> bool {
    matches!(
        (kind, media_type),
        (
            MediaKind::Audio,
            "audio/mpeg" | "audio/mp4" | "audio/ogg" | "audio/flac" | "audio/wav" | "audio/aac"
        ) | (
            MediaKind::Image,
            "image/png" | "image/jpeg" | "image/gif" | "image/webp"
        )
    )
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

impl From<MediaKind> for MediaKindDto {
    fn from(value: MediaKind) -> Self {
        match value {
            MediaKind::Audio => Self::Audio,
            MediaKind::Image => Self::Image,
        }
    }
}

impl From<MediaKindDto> for MediaKind {
    fn from(value: MediaKindDto) -> Self {
        match value {
            MediaKindDto::Audio => Self::Audio,
            MediaKindDto::Image => Self::Image,
        }
    }
}

impl From<MediaRole> for MediaRoleDto {
    fn from(value: MediaRole) -> Self {
        match value {
            MediaRole::PromptAudio => Self::PromptAudio,
            MediaRole::AnswerAudio => Self::AnswerAudio,
            MediaRole::RevealImage => Self::RevealImage,
        }
    }
}

impl From<MediaRoleDto> for MediaRole {
    fn from(value: MediaRoleDto) -> Self {
        match value {
            MediaRoleDto::PromptAudio => Self::PromptAudio,
            MediaRoleDto::AnswerAudio => Self::AnswerAudio,
            MediaRoleDto::RevealImage => Self::RevealImage,
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

impl From<SchedulingMode> for SchedulingModeDto {
    fn from(value: SchedulingMode) -> Self {
        match value {
            SchedulingMode::Automatic => Self::Automatic,
            SchedulingMode::Expert => Self::Expert,
        }
    }
}

impl From<SchedulingModeDto> for SchedulingMode {
    fn from(value: SchedulingModeDto) -> Self {
        match value {
            SchedulingModeDto::Automatic => Self::Automatic,
            SchedulingModeDto::Expert => Self::Expert,
        }
    }
}

impl From<DiffKind> for TextDiffKindDto {
    fn from(value: DiffKind) -> Self {
        match value {
            DiffKind::Equal => Self::Equal,
            DiffKind::Delete => Self::Delete,
            DiffKind::Insert => Self::Insert,
        }
    }
}

impl From<DiffSegment> for TextDiffSegmentDto {
    fn from(value: DiffSegment) -> Self {
        Self {
            kind: value.kind.into(),
            text: value.text,
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
    SchedulingModeDto::export_all_to(output)?;
    BudgetSourceDto::export_all_to(output)?;
    TextDiffKindDto::export_all_to(output)?;
    MediaKindDto::export_all_to(output)?;
    MediaRoleDto::export_all_to(output)?;
    MediaAvailabilityDto::export_all_to(output)?;
    ImportMediaRequest::export_all_to(output)?;
    LocalizedTextDto::export_all_to(output)?;
    StudyAnnotationDto::export_all_to(output)?;
    StudyMediaDto::export_all_to(output)?;
    TextDiffSegmentDto::export_all_to(output)?;
    StudyCardDto::export_all_to(output)?;
    CheckAnswerRequest::export_all_to(output)?;
    RevealSegmentDto::export_all_to(output)?;
    GradePreviewDto::export_all_to(output)?;
    RevealDto::export_all_to(output)?;
    GradeReviewRequest::export_all_to(output)?;
    GradeReviewResultDto::export_all_to(output)?;
    StudyQueueEntryDto::export_all_to(output)?;
    ReconcileStudyQueueRequest::export_all_to(output)?;
    SuspendCardRequest::export_all_to(output)?;
    UndoReviewRequest::export_all_to(output)?;
    UndoReviewResultDto::export_all_to(output)?;
    SchedulerSettingsDto::export_all_to(output)?;
    UpdateSchedulerSettingsRequest::export_all_to(output)?;
    SchedulerPolicyPreviewDto::export_all_to(output)?;
    ImportSchedulerParametersRequest::export_all_to(output)?;
    SchedulerParametersExportDto::export_all_to(output)?;
    DeckDto::export_all_to(output)?;
    DeckSummaryDto::export_all_to(output)?;
    DeckCardTrashDto::export_all_to(output)?;
    DeckCardStatusDto::export_all_to(output)?;
    DeckCardRequest::export_all_to(output)?;
    DeckCardDto::export_all_to(output)?;
    DeckCardDeckDto::export_all_to(output)?;
    DeckCardOverviewDto::export_all_to(output)?;
    DeckCardActionDto::export_all_to(output)?;
    DeckCardActionRequest::export_all_to(output)?;
    DeckCardActionResultDto::export_all_to(output)?;
    CreateDeckRequest::export_all_to(output)?;
    RenameDeckRequest::export_all_to(output)?;
    DeleteDeckRequest::export_all_to(output)?;
    DeleteDeckResultDto::export_all_to(output)?;
    BundleRemovalPreviewDto::export_all_to(output)?;
    BundleRemovalRequest::export_all_to(output)?;
    BundleRemovalProgressDto::export_all_to(output)?;
    BundleRemovalResultDto::export_all_to(output)?;
    AnnotationDraftDto::export_all_to(output)?;
    AuthoringSegmentKindDto::export_all_to(output)?;
    MatchingPolicyDto::export_all_to(output)?;
    AuthoringSegmentDto::export_all_to(output)?;
    AuthoringClozeDto::export_all_to(output)?;
    AuthoringDraftDto::export_all_to(output)?;
    AuthoringPreviewDto::export_all_to(output)?;
    MakeClozeRequest::export_all_to(output)?;
    RemoveClozeRequest::export_all_to(output)?;
    ReorderSegmentsRequest::export_all_to(output)?;
    TodayRequest::export_all_to(output)?;
    StudyAvailabilityDto::export_all_to(output)?;
    StudyPlanDto::export_all_to(output)?;
    TodayDeckDto::export_all_to(output)?;
    TodayQueueCardDto::export_all_to(output)?;
    TodayOverviewDto::export_all_to(output)?;
    PortableExportResultDto::export_all_to(output)?;
    BundleDeckInstallStatusDto::export_all_to(output)?;
    BundleDeckPreviewDto::export_all_to(output)?;
    BundlePreviewDto::export_all_to(output)?;
    BundleImportRequest::export_all_to(output)?;
    BundleImportStageDto::export_all_to(output)?;
    BundleImportProgressDto::export_all_to(output)?;
    BundleImportResultDto::export_all_to(output)?;
    BundleExportRequest::export_all_to(output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use meiki_domain::{Deck, Direction, MatchingPolicy, StudySettingsOverride};
    use meiki_scheduler::DEFAULT_PARAMETERS;
    use meiki_storage::{
        DEFAULT_DECK_ID, DeckRepository, SAMPLE_CARD_ID, SchedulerParameterSetRepository,
        SchedulerProfileRepository, SchedulingWorkload, Storage,
    };
    use tempfile::tempdir;

    use super::{
        ApplicationError, ApplicationService, BudgetSourceDto, CheckAnswerRequest,
        ComparisonResultDto, GradeDto, GradeReviewRequest, ImportSchedulerParametersRequest,
        SchedulerError, SchedulingModeDto, SuspendCardRequest, UndoReviewRequest,
        UpdateSchedulerSettingsRequest, controller_response_seconds,
    };

    #[test]
    fn walking_skeleton_checks_grades_and_restores() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        let card = service.seed_test_collection(1_000).unwrap();
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
        assert_eq!(reveal.raw_response, " 行きます ");
        assert_eq!(reveal.normalized_response, "行きます");
        assert_eq!(reveal.difference.len(), 1);
        assert_eq!(reveal.source_segments.len(), 2);
        assert!(reveal.source_segments[1].highlighted);
        assert_eq!(reveal.grade_previews.len(), 4);
        assert_eq!(reveal.grade_previews[2].grade, GradeDto::Good);

        let grade_request = GradeReviewRequest {
            review_event_id: "review-retry-safe".into(),
            card_id: card.card_id.clone(),
            card_content_version: card.card_content_version,
            schedule_version: card.schedule_version,
            raw_response: reveal.raw_response,
            chosen_grade: GradeDto::Easy,
            response_duration_ms: 1_250,
        };
        let result = service.grade_review(&grade_request).unwrap();
        assert_eq!(result.schedule_version, 1);
        assert_eq!(service.grade_review(&grade_request).unwrap(), result);
        let storage = Storage::open(&path).unwrap();
        let events = storage.review_events(SAMPLE_CARD_ID).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scheduler_version, "fsrs-7");
        assert_eq!(
            events[0].scheduler_parameter_set_id.as_deref(),
            Some("fsrs7-default-v1")
        );
        assert_eq!(events[0].target_retention_basis_points, 9_000);
        assert_eq!(events[0].response_duration_ms, 1_250);
        assert!(events[0].grade_overridden);
        assert_eq!(
            events[0].next_schedule.due_at_ms,
            events[0].next_schedule.ideal_due_at_ms
        );
        assert!(events[0].next_schedule.interval_milliseconds > 0);

        drop(storage);

        let undo_request = UndoReviewRequest {
            undo_event_id: "undo-retry-safe".into(),
            card_id: card.card_id.clone(),
            card_content_version: card.card_content_version,
            schedule_version: result.schedule_version,
            review_event_id: result.review_event_id.clone(),
        };
        let undo = service.undo_review(&undo_request).unwrap();
        assert_eq!(undo.schedule_version, 2);
        assert_eq!(undo.completed_reviews, 0);
        assert_eq!(service.undo_review(&undo_request).unwrap(), undo);
        assert_eq!(service.grade_review(&grade_request).unwrap(), result);
        assert_eq!(
            Storage::open(&path)
                .unwrap()
                .review_events(SAMPLE_CARD_ID)
                .unwrap()
                .len(),
            2
        );

        let suspended = service
            .suspend_card(&SuspendCardRequest {
                card_id: card.card_id,
                card_content_version: card.card_content_version,
                schedule_version: undo.schedule_version,
            })
            .unwrap();
        assert!(suspended.suspended);

        let restarted_service = ApplicationService::new(&path);
        let restored = restarted_service.get_study_card("sample-card").unwrap();
        assert_eq!(restored.schedule_version, 2);
        assert_eq!(restored.completed_reviews, 0);
        assert!(restored.suspended);
    }

    #[test]
    fn controller_response_estimate_uses_fallback_until_history_is_robust() {
        let sparse = SchedulingWorkload {
            unseen_cards: 0,
            due_cards_now: 0,
            forecast_review_occurrences: 0,
            response_duration_samples: 7,
            median_response_duration_ms: Some(1_500),
            review_count: 7,
        };
        assert_eq!(controller_response_seconds(&sparse), 20);
        assert_eq!(
            controller_response_seconds(&SchedulingWorkload {
                response_duration_samples: 8,
                review_count: 8,
                ..sparse
            }),
            5
        );
    }

    #[test]
    fn introduced_cards_require_the_exact_due_timestamp() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let mut storage = Storage::open(&path).unwrap();
        storage.seed_walking_skeleton(100_000).unwrap();
        drop(storage);
        let service = ApplicationService::new(&path);
        let first = GradeReviewRequest {
            review_event_id: "introduce-card".into(),
            card_id: SAMPLE_CARD_ID.into(),
            card_content_version: 0,
            schedule_version: 0,
            raw_response: "行きます".into(),
            chosen_grade: GradeDto::Good,
            response_duration_ms: 1_000,
        };
        service.grade_review_at(&first, 100_000).unwrap();
        let scheduled = Storage::open(&path)
            .unwrap()
            .load_schedule(SAMPLE_CARD_ID)
            .unwrap();
        assert!(scheduled.due_at_ms > 100_000);

        let second = GradeReviewRequest {
            review_event_id: "review-at-due".into(),
            card_id: SAMPLE_CARD_ID.into(),
            card_content_version: 0,
            schedule_version: 1,
            raw_response: "行きます".into(),
            chosen_grade: GradeDto::Good,
            response_duration_ms: 1_000,
        };
        assert!(matches!(
            service.grade_review_at(&second, scheduled.due_at_ms - 1),
            Err(ApplicationError::CardNotDue)
        ));
        assert_eq!(
            Storage::open(&path)
                .unwrap()
                .review_events(SAMPLE_CARD_ID)
                .unwrap()
                .len(),
            1
        );

        service
            .grade_review_at(&second, scheduled.due_at_ms)
            .unwrap();
        assert_eq!(
            Storage::open(&path)
                .unwrap()
                .review_events(SAMPLE_CARD_ID)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn automatic_and_expert_policy_changes_leave_existing_projections_unchanged() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        let card = service.seed_test_collection(1_000).unwrap();
        let defaults = service.get_scheduler_settings(DEFAULT_DECK_ID).unwrap();
        assert_eq!(defaults.scheduling_mode, SchedulingModeDto::Automatic);
        assert_eq!(defaults.collection_daily_time_budget_minutes, 30);
        assert_eq!(defaults.budget_source, BudgetSourceDto::CollectionBudget);
        assert_eq!(defaults.target_retention_basis_points, 9_000);
        assert!(matches!(
            service.export_scheduler_parameters(DEFAULT_DECK_ID),
            Err(ApplicationError::InvalidSchedulerParameterFile(_))
        ));
        let default_profile = Storage::open(&path)
            .unwrap()
            .get_scheduler_profile(DEFAULT_DECK_ID)
            .unwrap();
        let stored_parameters = Storage::open(&path)
            .unwrap()
            .get_scheduler_parameter_set(&default_profile.active_parameter_set_id)
            .unwrap()
            .parameters;
        assert!(
            stored_parameters
                .iter()
                .zip(DEFAULT_PARAMETERS)
                .all(|(stored, expected)| stored.to_bits() == expected.to_bits())
        );

        let proposed = UpdateSchedulerSettingsRequest {
            deck_id: DEFAULT_DECK_ID.into(),
            scheduling_mode: SchedulingModeDto::Automatic,
            collection_daily_time_budget_minutes: 15,
            deck_daily_time_budget_minutes: None,
            target_retention_basis_points: 8_750,
            new_cards_per_day: 12,
            maximum_interval_days: 2_000,
            day_boundary_minutes: 180,
            now_ms: 2_000,
            day_start_ms: 0,
        };
        let preview = service.preview_scheduler_policy(&proposed).unwrap();
        assert_eq!(preview.effective_daily_time_budget_minutes, 15);
        assert_eq!(preview.budget_source, BudgetSourceDto::CollectionBudget);
        assert_eq!(preview.target_retention_basis_points, 9_000);
        assert_eq!(preview.new_cards_per_day, 1);
        assert!(preview.explanation.contains("15 min/day"));
        let updated = service.update_scheduler_settings(&proposed).unwrap();
        assert_eq!(updated.scheduling_mode, SchedulingModeDto::Automatic);
        assert_eq!(updated.target_retention_basis_points, 9_000);
        assert_eq!(updated.new_cards_per_day, 1);
        assert_eq!(updated.effective_daily_time_budget_minutes, 15);

        let expert = service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                scheduling_mode: SchedulingModeDto::Expert,
                deck_daily_time_budget_minutes: Some(25),
                now_ms: 3_000,
                ..proposed.clone()
            })
            .unwrap();
        assert_eq!(expert.scheduling_mode, SchedulingModeDto::Expert);
        assert_eq!(expert.budget_source, BudgetSourceDto::DeckOverride);
        assert_eq!(expert.effective_daily_time_budget_minutes, 25);
        assert_eq!(expert.target_retention_basis_points, 8_750);
        assert_eq!(expert.new_cards_per_day, 12);

        let exported_parameters = service
            .export_scheduler_parameters(DEFAULT_DECK_ID)
            .unwrap();
        service
            .import_scheduler_parameters(&ImportSchedulerParametersRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                path: exported_parameters.path,
            })
            .unwrap();
        let imported_profile = Storage::open(&path)
            .unwrap()
            .get_scheduler_profile(DEFAULT_DECK_ID)
            .unwrap();
        assert_ne!(
            imported_profile.active_parameter_set_id,
            default_profile.active_parameter_set_id
        );

        let invalid_path = directory.path().join("invalid-parameters.json");
        std::fs::write(
            &invalid_path,
            r#"{"format":"meiki-scheduler-parameters","version":1,"parameter_set_id":"bad","engine_version":"fsrs-7","parameters":[1.0]}"#,
        )
        .unwrap();
        assert!(matches!(
            service.import_scheduler_parameters(&ImportSchedulerParametersRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                path: invalid_path.to_string_lossy().into_owned(),
            }),
            Err(ApplicationError::Scheduler(
                SchedulerError::InvalidParameterCount { .. }
            ))
        ));

        let reveal = service
            .check_answer(&CheckAnswerRequest {
                card_id: card.card_id.clone(),
                card_content_version: card.card_content_version,
                schedule_version: card.schedule_version,
                raw_response: "行きます".into(),
            })
            .unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "policy-review".into(),
                    card_id: card.card_id,
                    card_content_version: card.card_content_version,
                    schedule_version: card.schedule_version,
                    raw_response: reveal.raw_response,
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 900,
                },
                4_000,
            )
            .unwrap();

        let schedule_before = Storage::open(&path)
            .unwrap()
            .load_schedule(SAMPLE_CARD_ID)
            .unwrap();
        let history_before = Storage::open(&path)
            .unwrap()
            .review_events(SAMPLE_CARD_ID)
            .unwrap();
        service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                scheduling_mode: SchedulingModeDto::Automatic,
                collection_daily_time_budget_minutes: 90,
                deck_daily_time_budget_minutes: None,
                target_retention_basis_points: 9_300,
                new_cards_per_day: 40,
                maximum_interval_days: 5_000,
                day_boundary_minutes: 240,
                now_ms: 5_000,
                day_start_ms: 0,
            })
            .unwrap();
        let storage = Storage::open(&path).unwrap();
        assert_eq!(
            storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
            schedule_before
        );
        assert_eq!(
            storage.review_events(SAMPLE_CARD_ID).unwrap(),
            history_before
        );
        assert!(
            storage
                .check_collection_schedule_integrity()
                .unwrap()
                .is_valid()
        );
    }

    #[test]
    fn deck_budget_inheritance_and_override_sources_are_visible() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        service.get_scheduler_settings(DEFAULT_DECK_ID).unwrap();
        let mut storage = Storage::open(&path).unwrap();
        storage
            .create_deck(&Deck {
                id: "second-deck".into(),
                name: "Second".into(),
                description: None,
                language_tag: None,
                direction: Direction::Auto,
                matching_policy: MatchingPolicy::Strict,
                settings: StudySettingsOverride::default(),
                created_at_ms: 1_000,
                updated_at_ms: 1_000,
            })
            .unwrap();
        drop(storage);

        service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                scheduling_mode: SchedulingModeDto::Automatic,
                collection_daily_time_budget_minutes: 75,
                deck_daily_time_budget_minutes: None,
                target_retention_basis_points: 9_000,
                new_cards_per_day: 20,
                maximum_interval_days: 36_500,
                day_boundary_minutes: 240,
                now_ms: 2_000,
                day_start_ms: 0,
            })
            .unwrap();
        let inherited = service.get_scheduler_settings("second-deck").unwrap();
        assert_eq!(inherited.effective_daily_time_budget_minutes, 75);
        assert_eq!(inherited.budget_source, BudgetSourceDto::CollectionBudget);

        let overridden = service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                deck_id: "second-deck".into(),
                deck_daily_time_budget_minutes: Some(25),
                now_ms: 3_000,
                ..UpdateSchedulerSettingsRequest {
                    deck_id: DEFAULT_DECK_ID.into(),
                    scheduling_mode: SchedulingModeDto::Automatic,
                    collection_daily_time_budget_minutes: 75,
                    deck_daily_time_budget_minutes: None,
                    target_retention_basis_points: 9_000,
                    new_cards_per_day: 20,
                    maximum_interval_days: 36_500,
                    day_boundary_minutes: 240,
                    now_ms: 2_000,
                    day_start_ms: 0,
                }
            })
            .unwrap();
        assert_eq!(overridden.effective_daily_time_budget_minutes, 25);
        assert_eq!(overridden.budget_source, BudgetSourceDto::DeckOverride);
        assert_eq!(
            service
                .get_scheduler_settings(DEFAULT_DECK_ID)
                .unwrap()
                .effective_daily_time_budget_minutes,
            75
        );

        let cleared = service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                deck_daily_time_budget_minutes: None,
                now_ms: 4_000,
                ..UpdateSchedulerSettingsRequest {
                    deck_id: "second-deck".into(),
                    scheduling_mode: SchedulingModeDto::Automatic,
                    collection_daily_time_budget_minutes: 75,
                    deck_daily_time_budget_minutes: Some(25),
                    target_retention_basis_points: 9_000,
                    new_cards_per_day: 20,
                    maximum_interval_days: 36_500,
                    day_boundary_minutes: 240,
                    now_ms: 3_000,
                    day_start_ms: 0,
                }
            })
            .unwrap();
        assert_eq!(cleared.deck_daily_time_budget_minutes, None);
        assert_eq!(cleared.effective_daily_time_budget_minutes, 75);
        assert_eq!(cleared.budget_source, BudgetSourceDto::CollectionBudget);
    }
}
