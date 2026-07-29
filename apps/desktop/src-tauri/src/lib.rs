use std::path::PathBuf;

use meiki_application::{
    ApplicationService, AuthoringDraftDto, AuthoringPreviewDto, CheckAnswerRequest,
    GradeReviewRequest, GradeReviewResultDto, MakeClozeRequest, RemoveClozeRequest,
    ReorderSegmentsRequest, RevealDto, StudyCardDto,
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
fn initialize_collection(state: State<'_, AppContext>) -> Result<StudyCardDto, String> {
    state
        .service()
        .initialize_collection()
        .map_err(|error| error.to_string())
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
fn new_authoring_draft(state: State<'_, AppContext>) -> Result<AuthoringDraftDto, String> {
    state
        .service()
        .new_authoring_draft()
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
        .setup(|app| {
            let collection_path = app.path().app_data_dir()?.join("collection.db");
            app.manage(AppContext { collection_path });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initialize_collection,
            get_study_card,
            check_answer,
            grade_review,
            new_authoring_draft,
            make_cloze,
            remove_cloze,
            reorder_segments,
            preview_authoring_draft,
            save_authoring_draft
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Meiki");
}
