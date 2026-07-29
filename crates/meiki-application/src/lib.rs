//! Use cases and versioned desktop data-transfer objects.

use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use meiki_domain::{
    Annotation, ComparisonResult, Direction, Grade, LocalizedText, MatchingPolicy, MediaKind,
    MediaReference, OptimizerStatus, ReviewEvent, ReviewEventKind, SchedulerParameterSet,
    SchedulerProfile, SegmentContent, SourceItem, StudyIntensity, StudySettings,
    StudySettingsOverride,
};
use meiki_scheduler::{
    ENGINE_VERSION, Fsrs7Engine, MINIMUM_OPTIMIZATION_REVIEWS, OptimizationDiagnostics,
    OptimizationResult, ReviewHistoryEntry, SchedulerConfig, SchedulerEngine, SchedulerError,
};
use meiki_storage::{
    CardRepository, DeckRepository, SAMPLE_CARD_ID, SchedulerParameterSetRepository,
    SchedulerProfileRepository, Storage, StorageError, StoredStudyCard,
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

pub use authoring::{
    AnnotationDraftDto, AuthoringClozeDto, AuthoringDraftDto, AuthoringPreviewDto,
    AuthoringSegmentDto, AuthoringSegmentKindDto, MakeClozeRequest, MatchingPolicyDto,
    RemoveClozeRequest, ReorderSegmentsRequest,
};

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
    #[error("invalid authoring draft: {0}")]
    InvalidAuthoring(String),
    #[error("invalid text selection: {0}")]
    TextBoundary(#[from] meiki_text::TextBoundaryError),
    #[error("scheduler operation failed: {0}")]
    Scheduler(#[from] SchedulerError),
    #[error("unsupported scheduler engine version: {0}")]
    UnsupportedScheduler(String),
    #[error("failed to export scheduler diagnostics: {0}")]
    DiagnosticExport(#[source] std::io::Error),
    #[error("failed to serialize scheduler diagnostics: {0}")]
    DiagnosticSerialization(#[source] serde_json::Error),
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
pub enum StudyIntensityDto {
    Light,
    Balanced,
    Intensive,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum OptimizerStatusDto {
    NeverRun,
    InsufficientData,
    Adopted,
    Rejected,
    Failed,
    RolledBack,
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
    pub kind: MediaKindDto,
    pub media_type: String,
    pub original_file_name: Option<String>,
    pub alt_text: Option<String>,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
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
    pub intensity: StudyIntensityDto,
    pub target_retention_basis_points: u16,
    pub new_cards_per_day: u32,
    pub daily_time_budget_minutes: Option<u32>,
    pub maximum_interval_days: u32,
    pub day_boundary_minutes: u16,
    pub engine_version: String,
    pub active_parameter_set_id: String,
    pub previous_parameter_set_id: Option<String>,
    pub optimizer_status: OptimizerStatusDto,
    pub optimizer_diagnostics: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct UpdateSchedulerSettingsRequest {
    pub deck_id: String,
    pub intensity: StudyIntensityDto,
    pub target_retention_basis_points: u16,
    pub new_cards_per_day: u32,
    pub daily_time_budget_minutes: Option<u32>,
    pub maximum_interval_days: u32,
    pub day_boundary_minutes: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct RebuildSchedulerResultDto {
    pub backup_path: String,
    pub rebuilt_cards: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct SchedulerDiagnosticsExportDto {
    pub path: String,
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
        let comparison = compare_answer_with_options(
            &stored.cloze.answer,
            &stored.cloze.accepted_answers,
            &request.raw_response,
            &answer_options(&storage, &stored)?,
        );
        let suggested_grade = suggested_grade(comparison.result);
        let previewed_at_ms = Utc::now().timestamp_millis();
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
            answer_media: study_media(&stored.cloze.media),
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
                || stored.schedule.last_review_event_id.as_deref()
                    != Some(request.review_event_id.as_str())
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

        let comparison = compare_answer_with_options(
            &stored.cloze.answer,
            &stored.cloze.accepted_answers,
            &request.raw_response,
            &answer_options(&storage, &stored)?,
        );
        let suggested = suggested_grade(comparison.result);
        let reviewed_at_ms = Utc::now().timestamp_millis();
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
        let _ = maybe_run_automatic_optimizer(
            &mut storage,
            &stored.source_item.deck_id,
            reviewed_at_ms,
        );

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
        card.updated_at_ms = Utc::now().timestamp_millis();
        storage.update_card(&card)?;
        study_card_dto(&storage, &request.card_id)
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
            Utc::now().timestamp_millis(),
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
        let mut storage = self.open_storage()?;
        Fsrs7Engine::new(SchedulerConfig {
            target_retention_basis_points: request.target_retention_basis_points,
            maximum_interval_days: request.maximum_interval_days,
        })?;
        if request.day_boundary_minutes >= 1_440 || request.daily_time_budget_minutes == Some(0) {
            return Err(ApplicationError::Scheduler(SchedulerError::InvalidState(
                "settings controls are outside safe bounds",
            )));
        }

        let now_ms = Utc::now().timestamp_millis();
        let mut deck = storage.get_deck(&request.deck_id)?;
        deck.settings = StudySettingsOverride {
            target_retention_basis_points: Some(request.target_retention_basis_points),
            new_cards_per_day: Some(request.new_cards_per_day),
            maximum_interval_days: Some(request.maximum_interval_days),
        };
        deck.updated_at_ms = now_ms;
        storage.update_deck(&deck)?;

        let mut profile = storage.get_scheduler_profile(&request.deck_id)?;
        profile.intensity = request.intensity.into();
        profile.daily_time_budget_minutes = request.daily_time_budget_minutes;
        profile.day_boundary_minutes = request.day_boundary_minutes;
        profile.updated_at_ms = now_ms;
        storage.update_scheduler_profile(&profile)?;
        scheduler_settings_dto(&storage, &request.deck_id)
    }

    /// Runs deterministic local personalization and prospectively adopts only
    /// a holdout-validated improvement.
    ///
    /// Failed or rejected optimization leaves the known-good parameters active.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when required scheduler state cannot be
    /// loaded or persisted.
    pub fn optimize_scheduler(
        &self,
        deck_id: &str,
    ) -> Result<SchedulerSettingsDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        run_optimizer(&mut storage, deck_id, Utc::now().timestamp_millis())?;
        scheduler_settings_dto(&storage, deck_id)
    }

    /// Restores the previous known-good parameter set prospectively.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when no rollback target exists or
    /// persistence fails.
    pub fn rollback_scheduler(
        &self,
        deck_id: &str,
    ) -> Result<SchedulerSettingsDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        storage.rollback_scheduler_parameter_set(deck_id, Utc::now().timestamp_millis())?;
        scheduler_settings_dto(&storage, deck_id)
    }

    /// Explicitly rebuilds all schedule projections in a deck from immutable
    /// review events after creating a recovery backup.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when backup, replay, or atomic replacement
    /// fails.
    pub fn rebuild_scheduler(
        &self,
        deck_id: &str,
    ) -> Result<RebuildSchedulerResultDto, ApplicationError> {
        let mut storage = self.open_storage()?;
        let backup_path = scheduler_backup_path(&self.collection_path);
        storage.backup_to(&backup_path)?;

        let cards = storage.study_cards_for_deck(deck_id)?;
        let mut rebuilt = Vec::with_capacity(cards.len());
        for card in cards {
            let (engine, _, _) = scheduler_for_card(&storage, &card)?;
            let mut state = engine.initial_schedule(&card.card.id, card.card.created_at_ms);
            for event in storage.active_review_events(&card.card.id)? {
                state = engine
                    .review(&state, event.chosen_grade, event.reviewed_at_ms)?
                    .next_state;
                state.last_review_event_id = Some(event.id);
            }
            rebuilt.push(state);
        }
        storage.replace_schedule_projections(deck_id, &rebuilt)?;
        Ok(RebuildSchedulerResultDto {
            backup_path: backup_path.to_string_lossy().into_owned(),
            rebuilt_cards: desktop_u32(rebuilt.len() as u64, "rebuilt card count")?,
        })
    }

    /// Exports a content-free scheduler diagnostic report beside the
    /// collection.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError`] when scheduler metadata cannot be loaded
    /// or the report cannot be written.
    pub fn export_scheduler_diagnostics(
        &self,
        deck_id: &str,
    ) -> Result<SchedulerDiagnosticsExportDto, ApplicationError> {
        let storage = self.open_storage()?;
        let settings = scheduler_settings_dto(&storage, deck_id)?;
        let path = scheduler_diagnostics_path(&self.collection_path);
        let report = serde_json::json!({
            "engine_version": settings.engine_version,
            "active_parameter_set_id": settings.active_parameter_set_id,
            "has_rollback_parameter_set": settings.previous_parameter_set_id.is_some(),
            "optimizer_status": optimizer_status_name(settings.optimizer_status),
            "optimizer_diagnostics": settings
                .optimizer_diagnostics
                .as_deref()
                .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        });
        let report = serde_json::to_string_pretty(&report)
            .map_err(ApplicationError::DiagnosticSerialization)?;
        fs::write(&path, report).map_err(ApplicationError::DiagnosticExport)?;
        Ok(SchedulerDiagnosticsExportDto {
            path: path.to_string_lossy().into_owned(),
        })
    }

    fn open_storage(&self) -> Result<Storage, ApplicationError> {
        if let Some(parent) = self.collection_path.parent() {
            fs::create_dir_all(parent).map_err(ApplicationError::CollectionDirectory)?;
        }
        Ok(Storage::open(&self.collection_path)?)
    }
}

fn scheduler_for_card(
    storage: &Storage,
    card: &StoredStudyCard,
) -> Result<(Fsrs7Engine, SchedulerProfile, StudySettings), ApplicationError> {
    let deck = storage.get_deck(&card.source_item.deck_id)?;
    let settings = StudySettings::resolve(
        &StudySettings::default(),
        &deck.settings,
        &card.card.settings,
    );
    let profile = storage.get_scheduler_profile(&deck.id)?;
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
    let resolved = StudySettings::resolve(
        &StudySettings::default(),
        &deck.settings,
        &StudySettingsOverride::default(),
    );
    Ok(SchedulerSettingsDto {
        deck_id: deck_id.to_owned(),
        intensity: profile.intensity.into(),
        target_retention_basis_points: resolved.target_retention_basis_points,
        new_cards_per_day: resolved.new_cards_per_day,
        daily_time_budget_minutes: profile.daily_time_budget_minutes,
        maximum_interval_days: resolved.maximum_interval_days,
        day_boundary_minutes: profile.day_boundary_minutes,
        engine_version: profile.engine_version,
        active_parameter_set_id: profile.active_parameter_set_id,
        previous_parameter_set_id: profile.previous_parameter_set_id,
        optimizer_status: profile.optimizer_status.into(),
        optimizer_diagnostics: profile.optimizer_diagnostics,
    })
}

fn run_optimizer(
    storage: &mut Storage,
    deck_id: &str,
    now_ms: i64,
) -> Result<(), ApplicationError> {
    let deck = storage.get_deck(deck_id)?;
    let settings = StudySettings::resolve(
        &StudySettings::default(),
        &deck.settings,
        &StudySettingsOverride::default(),
    );
    let mut profile = storage.get_scheduler_profile(deck_id)?;
    let engine = scheduler_from_profile(storage, &profile, &settings)?;
    let history = storage
        .active_review_events_for_deck(deck_id)?
        .into_iter()
        .map(|event| ReviewHistoryEntry {
            card_id: event.card_id,
            reviewed_at_ms: event.reviewed_at_ms,
            grade: event.chosen_grade,
        })
        .collect::<Vec<_>>();

    match engine.optimize(&history) {
        OptimizationResult::InsufficientData { reviews, minimum } => {
            profile.optimizer_status = OptimizerStatus::InsufficientData;
            profile.optimizer_diagnostics = Some(
                serde_json::json!({
                    "result": "insufficient_data",
                    "reviews": reviews,
                    "minimum": minimum
                })
                .to_string(),
            );
            profile.updated_at_ms = now_ms;
            storage.update_scheduler_profile(&profile)?;
        }
        OptimizationResult::Adopted {
            parameters,
            diagnostics,
        } => {
            let diagnostics = optimization_diagnostics_json("adopted", &diagnostics);
            let parameter_set = SchedulerParameterSet {
                id: format!("fsrs7-personal-{}-{}", now_ms, Uuid::new_v4()),
                engine_version: ENGINE_VERSION.to_owned(),
                parameters: parameters.to_vec(),
                created_at_ms: now_ms,
            };
            storage.adopt_scheduler_parameter_set(deck_id, &parameter_set, &diagnostics, now_ms)?;
        }
        OptimizationResult::Rejected { diagnostics } => {
            profile.optimizer_status = OptimizerStatus::Rejected;
            profile.optimizer_diagnostics =
                Some(optimization_diagnostics_json("rejected", &diagnostics));
            profile.updated_at_ms = now_ms;
            storage.update_scheduler_profile(&profile)?;
        }
        OptimizationResult::Failed { reason } => {
            profile.optimizer_status = OptimizerStatus::Failed;
            profile.optimizer_diagnostics = Some(
                serde_json::json!({
                    "result": "failed",
                    "reason": reason
                })
                .to_string(),
            );
            profile.updated_at_ms = now_ms;
            storage.update_scheduler_profile(&profile)?;
        }
    }
    Ok(())
}

fn optimization_diagnostics_json(result: &str, diagnostics: &OptimizationDiagnostics) -> String {
    serde_json::json!({
        "result": result,
        "reviews": diagnostics.reviews,
        "training_reviews": diagnostics.training_reviews,
        "holdout_reviews": diagnostics.holdout_reviews,
        "current_holdout_loss": diagnostics.current_holdout_loss,
        "candidate_holdout_loss": diagnostics.candidate_holdout_loss
    })
    .to_string()
}

fn maybe_run_automatic_optimizer(
    storage: &mut Storage,
    deck_id: &str,
    now_ms: i64,
) -> Result<(), ApplicationError> {
    let review_count = storage.active_review_events_for_deck(deck_id)?.len();
    if review_count == MINIMUM_OPTIMIZATION_REVIEWS
        || (review_count > MINIMUM_OPTIMIZATION_REVIEWS && review_count % 32 == 0)
    {
        run_optimizer(storage, deck_id, now_ms)?;
    }
    Ok(())
}

fn scheduler_backup_path(collection_path: &Path) -> PathBuf {
    let name = collection_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("collection.db");
    collection_path.with_file_name(format!(
        "{name}.scheduler-rebuild-{}-{}.bak",
        Utc::now().timestamp_millis(),
        Uuid::new_v4()
    ))
}

fn scheduler_diagnostics_path(collection_path: &Path) -> PathBuf {
    let name = collection_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("collection.db");
    collection_path.with_file_name(format!(
        "{name}.scheduler-diagnostics-{}-{}.json",
        Utc::now().timestamp_millis(),
        Uuid::new_v4()
    ))
}

const fn optimizer_status_name(status: OptimizerStatusDto) -> &'static str {
    match status {
        OptimizerStatusDto::NeverRun => "never_run",
        OptimizerStatusDto::InsufficientData => "insufficient_data",
        OptimizerStatusDto::Adopted => "adopted",
        OptimizerStatusDto::Rejected => "rejected",
        OptimizerStatusDto::Failed => "failed",
        OptimizerStatusDto::RolledBack => "rolled_back",
    }
}

fn study_card_dto(storage: &Storage, card_id: &str) -> Result<StudyCardDto, ApplicationError> {
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
        prompt_media: study_media(&stored.source_item.media)
            .into_iter()
            .filter(|media| media.kind == MediaKindDto::Audio)
            .collect(),
    })
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

fn study_media(media: &[MediaReference]) -> Vec<StudyMediaDto> {
    media
        .iter()
        .map(|media| StudyMediaDto {
            id: media.id.clone(),
            kind: media.kind.into(),
            media_type: media.media_type.clone(),
            original_file_name: media.original_file_name.clone(),
            alt_text: media.alt_text.clone(),
            language_tag: media.language_tag.clone(),
            direction: media.direction.into(),
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

impl From<MediaKind> for MediaKindDto {
    fn from(value: MediaKind) -> Self {
        match value {
            MediaKind::Audio => Self::Audio,
            MediaKind::Image => Self::Image,
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

impl From<StudyIntensity> for StudyIntensityDto {
    fn from(value: StudyIntensity) -> Self {
        match value {
            StudyIntensity::Light => Self::Light,
            StudyIntensity::Balanced => Self::Balanced,
            StudyIntensity::Intensive => Self::Intensive,
        }
    }
}

impl From<StudyIntensityDto> for StudyIntensity {
    fn from(value: StudyIntensityDto) -> Self {
        match value {
            StudyIntensityDto::Light => Self::Light,
            StudyIntensityDto::Balanced => Self::Balanced,
            StudyIntensityDto::Intensive => Self::Intensive,
        }
    }
}

impl From<OptimizerStatus> for OptimizerStatusDto {
    fn from(value: OptimizerStatus) -> Self {
        match value {
            OptimizerStatus::NeverRun => Self::NeverRun,
            OptimizerStatus::InsufficientData => Self::InsufficientData,
            OptimizerStatus::Adopted => Self::Adopted,
            OptimizerStatus::Rejected => Self::Rejected,
            OptimizerStatus::Failed => Self::Failed,
            OptimizerStatus::RolledBack => Self::RolledBack,
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
    StudyIntensityDto::export_all_to(output)?;
    OptimizerStatusDto::export_all_to(output)?;
    TextDiffKindDto::export_all_to(output)?;
    MediaKindDto::export_all_to(output)?;
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
    SuspendCardRequest::export_all_to(output)?;
    UndoReviewRequest::export_all_to(output)?;
    UndoReviewResultDto::export_all_to(output)?;
    SchedulerSettingsDto::export_all_to(output)?;
    UpdateSchedulerSettingsRequest::export_all_to(output)?;
    RebuildSchedulerResultDto::export_all_to(output)?;
    SchedulerDiagnosticsExportDto::export_all_to(output)?;
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use meiki_scheduler::DEFAULT_PARAMETERS;
    use meiki_storage::{
        DEFAULT_DECK_ID, SAMPLE_CARD_ID, SchedulerParameterSetRepository, Storage,
    };
    use tempfile::tempdir;

    use super::{
        ApplicationService, CheckAnswerRequest, ComparisonResultDto, GradeDto, GradeReviewRequest,
        OptimizerStatusDto, StudyIntensityDto, SuspendCardRequest, UndoReviewRequest,
        UpdateSchedulerSettingsRequest,
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
    fn settings_personalization_status_and_backup_first_rebuild_round_trip() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("collection.db");
        let service = ApplicationService::new(&path);
        let card = service.initialize_collection().unwrap();
        let defaults = service.get_scheduler_settings(DEFAULT_DECK_ID).unwrap();
        assert_eq!(defaults.intensity, StudyIntensityDto::Balanced);
        assert_eq!(defaults.engine_version, "fsrs-7");
        assert_eq!(defaults.target_retention_basis_points, 9_000);
        let stored_parameters = Storage::open(&path)
            .unwrap()
            .get_scheduler_parameter_set(&defaults.active_parameter_set_id)
            .unwrap()
            .parameters;
        assert!(
            stored_parameters
                .iter()
                .zip(DEFAULT_PARAMETERS)
                .all(|(stored, expected)| stored.to_bits() == expected.to_bits())
        );

        let updated = service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                intensity: StudyIntensityDto::Light,
                target_retention_basis_points: 8_750,
                new_cards_per_day: 12,
                daily_time_budget_minutes: Some(25),
                maximum_interval_days: 2_000,
                day_boundary_minutes: 180,
            })
            .unwrap();
        assert_eq!(updated.intensity, StudyIntensityDto::Light);
        assert_eq!(updated.target_retention_basis_points, 8_750);
        assert_eq!(updated.daily_time_budget_minutes, Some(25));

        let reveal = service
            .check_answer(&CheckAnswerRequest {
                card_id: card.card_id.clone(),
                card_content_version: card.card_content_version,
                schedule_version: card.schedule_version,
                raw_response: "行きます".into(),
            })
            .unwrap();
        service
            .grade_review(&GradeReviewRequest {
                review_event_id: "optimizer-review".into(),
                card_id: card.card_id,
                card_content_version: card.card_content_version,
                schedule_version: card.schedule_version,
                raw_response: reveal.raw_response,
                chosen_grade: GradeDto::Good,
                response_duration_ms: 900,
            })
            .unwrap();

        let optimized = service.optimize_scheduler(DEFAULT_DECK_ID).unwrap();
        assert_eq!(
            optimized.optimizer_status,
            OptimizerStatusDto::InsufficientData
        );
        assert!(
            optimized
                .optimizer_diagnostics
                .as_deref()
                .unwrap()
                .contains("\"minimum\":64")
        );
        let diagnostic_export = service
            .export_scheduler_diagnostics(DEFAULT_DECK_ID)
            .unwrap();
        let diagnostic_json = std::fs::read_to_string(&diagnostic_export.path).unwrap();
        assert!(diagnostic_json.contains("\"engine_version\": \"fsrs-7\""));
        assert!(!diagnostic_json.contains("行きます"));
        assert!(!diagnostic_json.contains(SAMPLE_CARD_ID));

        let history_before = Storage::open(&path)
            .unwrap()
            .review_events(SAMPLE_CARD_ID)
            .unwrap();
        let rebuilt = service.rebuild_scheduler(DEFAULT_DECK_ID).unwrap();
        assert_eq!(rebuilt.rebuilt_cards, 1);
        assert!(Path::new(&rebuilt.backup_path).is_file());
        let history_after = Storage::open(&path)
            .unwrap()
            .review_events(SAMPLE_CARD_ID)
            .unwrap();
        assert_eq!(history_after, history_before);
    }
}
