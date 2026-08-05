use std::path::PathBuf;

use meiki_application::{
    ApplicationService, ArchiveAddDeckRequest, ArchiveAddDeckResultDto, ArchiveExportRequest,
    ArchiveImportRequest, ArchiveImportResultDto, AuthoringDraftDto, AuthoringPreviewDto,
    BackupDto, CheckAnswerRequest, CreateDeckRequest, DeckDto, DeckSummaryDto, DeleteDeckRequest,
    DeleteDeckResultDto, GradeReviewRequest, GradeReviewResultDto, ImportMediaRequest,
    ImportSchedulerParametersRequest, LibraryBulkRequest, LibraryBulkResultDto, LibraryOverviewDto,
    LibraryRequest, MakeClozeRequest, PortableArchivePreviewDto, PortableExportResultDto,
    ReconcileStudyQueueRequest, RemoveClozeRequest, RenameDeckRequest, ReorderSegmentsRequest,
    RevealDto, SchedulerParametersExportDto, SchedulerPolicyPreviewDto, SchedulerSettingsDto,
    StudyCardDto, StudyMediaDto, StudyPlanDto, StudyQueueEntryDto, SuspendCardRequest,
    TodayOverviewDto, TodayRequest, UndoReviewRequest, UndoReviewResultDto,
    UpdateSchedulerSettingsRequest,
};
use tauri::{Manager, State};

mod commands;

macro_rules! desktop_commands {
    ($apply:ident) => {
        $apply!(
            get_study_card,
            get_today_overview,
            prepare_study,
            reconcile_study_queue,
            get_library,
            apply_library_bulk_action,
            export_archive,
            preview_archive,
            add_archive_deck,
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
            list_deck_summaries,
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
        )
    };
}

macro_rules! tauri_handler {
    ($($command:ident),+ $(,)?) => {
        tauri::generate_handler![$($command),+]
    };
}

#[cfg(test)]
macro_rules! command_names {
    ($($command:ident),+ $(,)?) => {
        &[$(stringify!($command)),+]
    };
}

#[cfg(test)]
const REGISTERED_COMMANDS: &[&str] = desktop_commands!(command_names);

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
    commands::get_study_card(&state.service(), &card_id)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_today_overview(
    request: TodayRequest,
    state: State<'_, AppContext>,
) -> Result<TodayOverviewDto, String> {
    commands::get_today_overview(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn prepare_study(
    request: TodayRequest,
    state: State<'_, AppContext>,
) -> Result<StudyPlanDto, String> {
    commands::prepare_study(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn reconcile_study_queue(
    request: ReconcileStudyQueueRequest,
    state: State<'_, AppContext>,
) -> Result<Vec<StudyQueueEntryDto>, String> {
    commands::reconcile_study_queue(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_library(
    request: LibraryRequest,
    state: State<'_, AppContext>,
) -> Result<LibraryOverviewDto, String> {
    commands::get_library(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn apply_library_bulk_action(
    request: LibraryBulkRequest,
    state: State<'_, AppContext>,
) -> Result<LibraryBulkResultDto, String> {
    commands::apply_library_bulk_action(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn export_archive(
    request: ArchiveExportRequest,
    state: State<'_, AppContext>,
) -> Result<PortableExportResultDto, String> {
    commands::export_archive(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn preview_archive(
    path: String,
    state: State<'_, AppContext>,
) -> Result<PortableArchivePreviewDto, String> {
    commands::preview_archive(&state.service(), &path)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn add_archive_deck(
    request: ArchiveAddDeckRequest,
    state: State<'_, AppContext>,
) -> Result<ArchiveAddDeckResultDto, String> {
    commands::add_archive_deck(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn import_archive(
    request: ArchiveImportRequest,
    state: State<'_, AppContext>,
) -> Result<ArchiveImportResultDto, String> {
    commands::import_archive(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_backups(state: State<'_, AppContext>) -> Result<Vec<BackupDto>, String> {
    commands::list_backups(&state.service())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn restore_backup(
    path: String,
    confirmation: String,
    state: State<'_, AppContext>,
) -> Result<BackupDto, String> {
    commands::restore_backup(&state.service(), &path, &confirmation)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_authoring_draft_for_card(
    card_id: String,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    commands::get_authoring_draft_for_card(&state.service(), &card_id)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn check_answer(
    request: CheckAnswerRequest,
    state: State<'_, AppContext>,
) -> Result<RevealDto, String> {
    commands::check_answer(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn grade_review(
    request: GradeReviewRequest,
    state: State<'_, AppContext>,
) -> Result<GradeReviewResultDto, String> {
    commands::grade_review(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn suspend_card(
    request: SuspendCardRequest,
    state: State<'_, AppContext>,
) -> Result<StudyCardDto, String> {
    commands::suspend_card(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn undo_review(
    request: UndoReviewRequest,
    state: State<'_, AppContext>,
) -> Result<UndoReviewResultDto, String> {
    commands::undo_review(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_scheduler_settings(
    deck_id: String,
    state: State<'_, AppContext>,
) -> Result<SchedulerSettingsDto, String> {
    commands::get_scheduler_settings(&state.service(), &deck_id)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn update_scheduler_settings(
    request: UpdateSchedulerSettingsRequest,
    state: State<'_, AppContext>,
) -> Result<SchedulerSettingsDto, String> {
    commands::update_scheduler_settings(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn preview_scheduler_policy(
    request: UpdateSchedulerSettingsRequest,
    state: State<'_, AppContext>,
) -> Result<SchedulerPolicyPreviewDto, String> {
    commands::preview_scheduler_policy(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn import_scheduler_parameters(
    request: ImportSchedulerParametersRequest,
    state: State<'_, AppContext>,
) -> Result<SchedulerSettingsDto, String> {
    commands::import_scheduler_parameters(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn export_scheduler_parameters(
    deck_id: String,
    state: State<'_, AppContext>,
) -> Result<SchedulerParametersExportDto, String> {
    commands::export_scheduler_parameters(&state.service(), &deck_id)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_decks(state: State<'_, AppContext>) -> Result<Vec<DeckDto>, String> {
    commands::list_decks(&state.service())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_deck_summaries(
    now_ms: i64,
    state: State<'_, AppContext>,
) -> Result<Vec<DeckSummaryDto>, String> {
    commands::list_deck_summaries(&state.service(), now_ms)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn create_deck(
    request: CreateDeckRequest,
    state: State<'_, AppContext>,
) -> Result<DeckDto, String> {
    commands::create_deck(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn rename_deck(
    request: RenameDeckRequest,
    state: State<'_, AppContext>,
) -> Result<DeckDto, String> {
    commands::rename_deck(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn delete_deck(
    request: DeleteDeckRequest,
    state: State<'_, AppContext>,
) -> Result<DeleteDeckResultDto, String> {
    commands::delete_deck(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn new_authoring_draft(state: State<'_, AppContext>) -> Result<AuthoringDraftDto, String> {
    commands::new_authoring_draft(&state.service())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn import_media(
    request: ImportMediaRequest,
    state: State<'_, AppContext>,
) -> Result<StudyMediaDto, String> {
    commands::import_media(&state.service(), &request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn make_cloze(
    request: MakeClozeRequest,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    commands::make_cloze(&state.service(), request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn remove_cloze(
    request: RemoveClozeRequest,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    commands::remove_cloze(&state.service(), request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn reorder_segments(
    request: ReorderSegmentsRequest,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    commands::reorder_segments(&state.service(), request)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn preview_authoring_draft(
    draft: AuthoringDraftDto,
    state: State<'_, AppContext>,
) -> Result<Vec<AuthoringPreviewDto>, String> {
    commands::preview_authoring_draft(&state.service(), &draft)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn save_authoring_draft(
    draft: AuthoringDraftDto,
    state: State<'_, AppContext>,
) -> Result<AuthoringDraftDto, String> {
    commands::save_authoring_draft(&state.service(), &draft)
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
            let data_directory = std::env::var_os("MEIKI_DATA_DIR")
                .map(PathBuf::from)
                .map_or_else(|| app.path().app_data_dir(), Ok)?;
            let collection_path = data_directory.join("collection.db");
            app.manage(AppContext { collection_path });
            Ok(())
        })
        .invoke_handler(desktop_commands!(tauri_handler))
        .run(tauri::generate_context!())
        .expect("failed to run Meiki");
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use meiki_application::{ALL_DECKS_ID, CreateDeckRequest, StudyAvailabilityDto, TodayRequest};
    use serde_json::json;
    use tempfile::tempdir;

    use super::{REGISTERED_COMMANDS, commands};

    const EXPECTED_COMMANDS: &[&str] = &[
        "get_study_card",
        "get_today_overview",
        "prepare_study",
        "reconcile_study_queue",
        "get_library",
        "apply_library_bulk_action",
        "export_archive",
        "preview_archive",
        "add_archive_deck",
        "import_archive",
        "list_backups",
        "restore_backup",
        "get_authoring_draft_for_card",
        "check_answer",
        "grade_review",
        "suspend_card",
        "undo_review",
        "get_scheduler_settings",
        "update_scheduler_settings",
        "preview_scheduler_policy",
        "import_scheduler_parameters",
        "export_scheduler_parameters",
        "list_decks",
        "list_deck_summaries",
        "create_deck",
        "rename_deck",
        "delete_deck",
        "new_authoring_draft",
        "import_media",
        "make_cloze",
        "remove_cloze",
        "reorder_segments",
        "preview_authoring_draft",
        "save_authoring_draft",
    ];

    fn service() -> (tempfile::TempDir, meiki_application::ApplicationService) {
        let directory = tempdir().expect("create desktop command test directory");
        let service =
            meiki_application::ApplicationService::new(directory.path().join("collection.db"));
        (directory, service)
    }

    #[test]
    fn handler_registration_is_complete_and_unique() {
        assert_eq!(REGISTERED_COMMANDS, EXPECTED_COMMANDS);
        assert_eq!(
            REGISTERED_COMMANDS
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len(),
            EXPECTED_COMMANDS.len()
        );
    }

    #[test]
    fn plain_commands_map_arguments_and_preserve_dto_serialization() {
        let (_directory, service) = service();
        let request = CreateDeckRequest {
            name: "Adapter contract".into(),
            now_ms: 1_700_000_000_000,
        };
        let created = commands::create_deck(&service, &request)
            .expect("map the complete request to ApplicationService");
        assert_eq!(created.name, "Adapter contract");
        assert!(
            commands::list_decks(&service)
                .expect("list through the same collection path")
                .iter()
                .any(|deck| deck.id == created.id)
        );
        let summary = commands::list_deck_summaries(&service, request.now_ms)
            .expect("map the summary timestamp to ApplicationService")
            .into_iter()
            .find(|deck| deck.id == created.id)
            .expect("include an empty user-created deck");
        assert_eq!(summary.total_cards, 0);
        assert_eq!(summary.due_cards, 0);
        assert_eq!(summary.new_cards, 0);

        let serialized = serde_json::to_value(created).expect("serialize the desktop DTO");
        assert_eq!(serialized["name"], json!("Adapter contract"));
        assert_eq!(serialized["is_default"], json!(false));
        assert_eq!(serialized["note_count"], json!(0));
        assert!(
            serialized
                .get("daily_time_budget_override_minutes")
                .is_some()
        );
    }

    #[test]
    fn plain_commands_keep_empty_state_and_application_errors_as_contract_data() {
        let (_directory, service) = service();
        let request = TodayRequest {
            deck_id: ALL_DECKS_ID.into(),
            now_ms: 1_700_000_000_000,
            day_start_ms: 1_699_956_800_000,
            day_end_ms: 1_700_043_200_000,
        };
        let plan = commands::prepare_study(&service, &request)
            .expect("return an empty collection as DTO data");
        assert_eq!(plan.availability, StudyAvailabilityDto::EmptyCollection);
        assert_eq!(
            serde_json::to_value(plan).expect("serialize the study plan")["availability"],
            json!("empty_collection")
        );

        let expected = service
            .get_study_card("missing-card")
            .expect_err("the application layer rejects a missing card")
            .to_string();
        let actual = commands::get_study_card(&service, "missing-card")
            .expect_err("the command maps the application error");
        assert_eq!(actual, expected);
    }
}
