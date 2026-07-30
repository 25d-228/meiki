use std::path::PathBuf;

use meiki_application::{
    ApplicationService, ArchiveExportRequest, ArchiveImportRequest, ArchiveImportResultDto,
    AuthoringDraftDto, AuthoringPreviewDto, BackupDto, CheckAnswerRequest, CreateDeckRequest,
    DeckDto, DeleteDeckRequest, DeleteDeckResultDto, GradeReviewRequest, GradeReviewResultDto,
    ImportMediaRequest, ImportSchedulerParametersRequest, LibraryBulkRequest, LibraryBulkResultDto,
    LibraryOverviewDto, LibraryRequest, MakeClozeRequest, PortableArchivePreviewDto,
    PortableExportResultDto, ReconcileStudyQueueRequest, RemoveClozeRequest, RenameDeckRequest,
    ReorderSegmentsRequest, RevealDto, SchedulerParametersExportDto, SchedulerPolicyPreviewDto,
    SchedulerSettingsDto, StudyCardDto, StudyMediaDto, StudyPlanDto, StudyQueueEntryDto,
    SuspendCardRequest, TodayOverviewDto, TodayRequest, UndoReviewRequest, UndoReviewResultDto,
    UpdateSchedulerSettingsRequest,
};
use tauri::{Manager, State};

struct AppContext {
    collection_path: PathBuf,
}

impl AppContext {
    fn service(&self) -> ApplicationService {
        ApplicationService::new(&self.collection_path)
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_study_card(card_id: String, state: State<'_, AppContext>) -> Result<StudyCardDto, String> {
    state
        .service()
        .get_study_card(&card_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_today_overview(
    request: TodayRequest,
    state: State<'_, AppContext>,
) -> Result<TodayOverviewDto, String> {
    state
        .service()
        .get_today_overview(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn prepare_study(
    request: TodayRequest,
    state: State<'_, AppContext>,
) -> Result<StudyPlanDto, String> {
    state
        .service()
        .prepare_study(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn reconcile_study_queue(
    request: ReconcileStudyQueueRequest,
    state: State<'_, AppContext>,
) -> Result<Vec<StudyQueueEntryDto>, String> {
    state
        .service()
        .reconcile_study_queue(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_library(
    request: LibraryRequest,
    state: State<'_, AppContext>,
) -> Result<LibraryOverviewDto, String> {
    state
        .service()
        .get_library(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn apply_library_bulk_action(
    request: LibraryBulkRequest,
    state: State<'_, AppContext>,
) -> Result<LibraryBulkResultDto, String> {
    state
        .service()
        .apply_library_bulk_action(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn export_archive(
    request: ArchiveExportRequest,
    state: State<'_, AppContext>,
) -> Result<PortableExportResultDto, String> {
    state
        .service()
        .export_archive(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn preview_archive(
    path: String,
    state: State<'_, AppContext>,
) -> Result<PortableArchivePreviewDto, String> {
    state
        .service()
        .preview_archive(&path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn import_archive(
    request: ArchiveImportRequest,
    state: State<'_, AppContext>,
) -> Result<ArchiveImportResultDto, String> {
    state
        .service()
        .import_archive(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_backups(state: State<'_, AppContext>) -> Result<Vec<BackupDto>, String> {
    state
        .service()
        .list_backups()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn restore_backup(
    path: String,
    confirmation: String,
    state: State<'_, AppContext>,
) -> Result<BackupDto, String> {
    state
        .service()
        .restore_backup(&path, &confirmation)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_authoring_draft_for_card(
    card_id: String,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    state
        .service()
        .get_authoring_draft_for_card(&card_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn check_answer(
    request: CheckAnswerRequest,
    state: State<'_, AppContext>,
) -> Result<RevealDto, String> {
    state
        .service()
        .check_answer(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn grade_review(
    request: GradeReviewRequest,
    state: State<'_, AppContext>,
) -> Result<GradeReviewResultDto, String> {
    state
        .service()
        .grade_review(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn suspend_card(
    request: SuspendCardRequest,
    state: State<'_, AppContext>,
) -> Result<StudyCardDto, String> {
    state
        .service()
        .suspend_card(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn undo_review(
    request: UndoReviewRequest,
    state: State<'_, AppContext>,
) -> Result<UndoReviewResultDto, String> {
    state
        .service()
        .undo_review(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_scheduler_settings(
    deck_id: String,
    state: State<'_, AppContext>,
) -> Result<SchedulerSettingsDto, String> {
    state
        .service()
        .get_scheduler_settings(&deck_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn update_scheduler_settings(
    request: UpdateSchedulerSettingsRequest,
    state: State<'_, AppContext>,
) -> Result<SchedulerSettingsDto, String> {
    state
        .service()
        .update_scheduler_settings(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn preview_scheduler_policy(
    request: UpdateSchedulerSettingsRequest,
    state: State<'_, AppContext>,
) -> Result<SchedulerPolicyPreviewDto, String> {
    state
        .service()
        .preview_scheduler_policy(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn import_scheduler_parameters(
    request: ImportSchedulerParametersRequest,
    state: State<'_, AppContext>,
) -> Result<SchedulerSettingsDto, String> {
    state
        .service()
        .import_scheduler_parameters(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn export_scheduler_parameters(
    deck_id: String,
    state: State<'_, AppContext>,
) -> Result<SchedulerParametersExportDto, String> {
    state
        .service()
        .export_scheduler_parameters(&deck_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_decks(state: State<'_, AppContext>) -> Result<Vec<DeckDto>, String> {
    state
        .service()
        .list_decks()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn create_deck(
    request: CreateDeckRequest,
    state: State<'_, AppContext>,
) -> Result<DeckDto, String> {
    state
        .service()
        .create_deck(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn rename_deck(
    request: RenameDeckRequest,
    state: State<'_, AppContext>,
) -> Result<DeckDto, String> {
    state
        .service()
        .rename_deck(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn delete_deck(
    request: DeleteDeckRequest,
    state: State<'_, AppContext>,
) -> Result<DeleteDeckResultDto, String> {
    state
        .service()
        .delete_deck(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn new_authoring_draft(state: State<'_, AppContext>) -> Result<AuthoringDraftDto, String> {
    state
        .service()
        .new_authoring_draft()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn import_media(
    request: ImportMediaRequest,
    state: State<'_, AppContext>,
) -> Result<StudyMediaDto, String> {
    state
        .service()
        .import_media(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn make_cloze(
    request: MakeClozeRequest,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    state
        .service()
        .make_cloze(request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn remove_cloze(
    request: RemoveClozeRequest,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    state
        .service()
        .remove_cloze(request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn reorder_segments(
    request: ReorderSegmentsRequest,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    state
        .service()
        .reorder_segments(request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn preview_authoring_draft(
    draft: AuthoringDraftDto,
    state: State<'_, AppContext>,
) -> Result<Vec<AuthoringPreviewDto>, String> {
    state
        .service()
        .preview_authoring_draft(&draft)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn save_authoring_draft(
    draft: AuthoringDraftDto,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    state
        .service()
        .save_authoring_draft(&draft)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the desktop runtime.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or encounters an unrecoverable runtime
/// failure.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let collection_path = app.path().app_data_dir()?.join("collection.db");
            app.manage(AppContext { collection_path });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_study_card,
            get_today_overview,
            prepare_study,
            reconcile_study_queue,
            get_library,
            apply_library_bulk_action,
            export_archive,
            preview_archive,
            import_archive,
            list_backups,
            restore_backup,
            get_authoring_draft_for_card,
            check_answer,
            grade_review,
            suspend_card,
            undo_review,
            get_scheduler_settings,
            update_scheduler_settings,
            preview_scheduler_policy,
            import_scheduler_parameters,
            export_scheduler_parameters,
            list_decks,
            create_deck,
            rename_deck,
            delete_deck,
            new_authoring_draft,
            import_media,
            make_cloze,
            remove_cloze,
            reorder_segments,
            preview_authoring_draft,
            save_authoring_draft
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Meiki");
}
