use meiki_application::{
    ApplicationService, ArchiveAddDeckRequest, ArchiveAddDeckResultDto, ArchiveExportRequest,
    ArchiveImportRequest, ArchiveImportResultDto, AuthoringDraftDto, AuthoringPreviewDto,
    BackupDto, BundleImportProgressDto, BundleImportRequest, BundleImportResultDto,
    BundlePreviewDto, CheckAnswerRequest, CreateDeckRequest, DeckCardActionRequest,
    DeckCardActionResultDto, DeckCardOverviewDto, DeckCardRequest, DeckDto, DeckSummaryDto,
    DeleteDeckRequest, DeleteDeckResultDto, GradeReviewRequest, GradeReviewResultDto,
    ImportMediaRequest, ImportSchedulerParametersRequest, LibraryBulkRequest, LibraryBulkResultDto,
    LibraryOverviewDto, LibraryRequest, MakeClozeRequest, PortableArchivePreviewDto,
    PortableExportResultDto, ReconcileStudyQueueRequest, RemoveClozeRequest, RenameDeckRequest,
    ReorderSegmentsRequest, RevealDto, SchedulerParametersExportDto, SchedulerPolicyPreviewDto,
    SchedulerSettingsDto, StudyCardDto, StudyMediaDto, StudyPlanDto, StudyQueueEntryDto,
    SuspendCardRequest, TodayOverviewDto, TodayRequest, UndoReviewRequest, UndoReviewResultDto,
    UpdateSchedulerSettingsRequest,
};

type CommandResult<T> = Result<T, String>;

fn map_error<T>(result: Result<T, meiki_application::ApplicationError>) -> CommandResult<T> {
    result.map_err(|error| error.to_string())
}

pub(crate) fn get_study_card(
    service: &ApplicationService,
    card_id: &str,
) -> CommandResult<StudyCardDto> {
    map_error(service.get_study_card(card_id))
}

pub(crate) fn get_today_overview(
    service: &ApplicationService,
    request: &TodayRequest,
) -> CommandResult<TodayOverviewDto> {
    map_error(service.get_today_overview(request))
}

pub(crate) fn prepare_study(
    service: &ApplicationService,
    request: &TodayRequest,
) -> CommandResult<StudyPlanDto> {
    map_error(service.prepare_study(request))
}

pub(crate) fn reconcile_study_queue(
    service: &ApplicationService,
    request: &ReconcileStudyQueueRequest,
) -> CommandResult<Vec<StudyQueueEntryDto>> {
    map_error(service.reconcile_study_queue(request))
}

pub(crate) fn get_library(
    service: &ApplicationService,
    request: &LibraryRequest,
) -> CommandResult<LibraryOverviewDto> {
    map_error(service.get_library(request))
}

pub(crate) fn apply_library_bulk_action(
    service: &ApplicationService,
    request: &LibraryBulkRequest,
) -> CommandResult<LibraryBulkResultDto> {
    map_error(service.apply_library_bulk_action(request))
}

pub(crate) fn get_deck_cards(
    service: &ApplicationService,
    request: &DeckCardRequest,
) -> CommandResult<DeckCardOverviewDto> {
    map_error(service.get_deck_cards(request))
}

pub(crate) fn apply_deck_card_action(
    service: &ApplicationService,
    request: &DeckCardActionRequest,
) -> CommandResult<DeckCardActionResultDto> {
    map_error(service.apply_deck_card_action(request))
}

pub(crate) fn export_archive(
    service: &ApplicationService,
    request: &ArchiveExportRequest,
) -> CommandResult<PortableExportResultDto> {
    map_error(service.export_archive(request))
}

pub(crate) fn preview_archive(
    service: &ApplicationService,
    path: &str,
) -> CommandResult<PortableArchivePreviewDto> {
    map_error(service.preview_archive(path))
}

pub(crate) fn preview_bundle(
    service: &ApplicationService,
    path: &str,
) -> CommandResult<BundlePreviewDto> {
    map_error(service.preview_bundle(path))
}

pub(crate) fn import_bundle(
    service: &ApplicationService,
    request: &BundleImportRequest,
    on_progress: impl FnMut(BundleImportProgressDto),
) -> CommandResult<BundleImportResultDto> {
    map_error(service.import_bundle(request, on_progress))
}

pub(crate) fn add_archive_deck(
    service: &ApplicationService,
    request: &ArchiveAddDeckRequest,
) -> CommandResult<ArchiveAddDeckResultDto> {
    map_error(service.add_archive_deck(request))
}

pub(crate) fn import_archive(
    service: &ApplicationService,
    request: &ArchiveImportRequest,
) -> CommandResult<ArchiveImportResultDto> {
    map_error(service.import_archive(request))
}

pub(crate) fn list_backups(service: &ApplicationService) -> CommandResult<Vec<BackupDto>> {
    map_error(service.list_backups())
}

pub(crate) fn restore_backup(
    service: &ApplicationService,
    path: &str,
    confirmation: &str,
) -> CommandResult<BackupDto> {
    map_error(service.restore_backup(path, confirmation))
}

pub(crate) fn get_authoring_draft_for_card(
    service: &ApplicationService,
    card_id: &str,
) -> CommandResult<AuthoringDraftDto> {
    map_error(service.get_authoring_draft_for_card(card_id))
}

pub(crate) fn check_answer(
    service: &ApplicationService,
    request: &CheckAnswerRequest,
) -> CommandResult<RevealDto> {
    map_error(service.check_answer(request))
}

pub(crate) fn grade_review(
    service: &ApplicationService,
    request: &GradeReviewRequest,
) -> CommandResult<GradeReviewResultDto> {
    map_error(service.grade_review(request))
}

pub(crate) fn suspend_card(
    service: &ApplicationService,
    request: &SuspendCardRequest,
) -> CommandResult<StudyCardDto> {
    map_error(service.suspend_card(request))
}

pub(crate) fn undo_review(
    service: &ApplicationService,
    request: &UndoReviewRequest,
) -> CommandResult<UndoReviewResultDto> {
    map_error(service.undo_review(request))
}

pub(crate) fn get_scheduler_settings(
    service: &ApplicationService,
    deck_id: &str,
) -> CommandResult<SchedulerSettingsDto> {
    map_error(service.get_scheduler_settings(deck_id))
}

pub(crate) fn update_scheduler_settings(
    service: &ApplicationService,
    request: &UpdateSchedulerSettingsRequest,
) -> CommandResult<SchedulerSettingsDto> {
    map_error(service.update_scheduler_settings(request))
}

pub(crate) fn preview_scheduler_policy(
    service: &ApplicationService,
    request: &UpdateSchedulerSettingsRequest,
) -> CommandResult<SchedulerPolicyPreviewDto> {
    map_error(service.preview_scheduler_policy(request))
}

pub(crate) fn import_scheduler_parameters(
    service: &ApplicationService,
    request: &ImportSchedulerParametersRequest,
) -> CommandResult<SchedulerSettingsDto> {
    map_error(service.import_scheduler_parameters(request))
}

pub(crate) fn export_scheduler_parameters(
    service: &ApplicationService,
    deck_id: &str,
) -> CommandResult<SchedulerParametersExportDto> {
    map_error(service.export_scheduler_parameters(deck_id))
}

pub(crate) fn list_decks(service: &ApplicationService) -> CommandResult<Vec<DeckDto>> {
    map_error(service.list_decks())
}

pub(crate) fn list_deck_summaries(
    service: &ApplicationService,
    now_ms: i64,
) -> CommandResult<Vec<DeckSummaryDto>> {
    map_error(service.list_deck_summaries(now_ms))
}

pub(crate) fn create_deck(
    service: &ApplicationService,
    request: &CreateDeckRequest,
) -> CommandResult<DeckDto> {
    map_error(service.create_deck(request))
}

pub(crate) fn rename_deck(
    service: &ApplicationService,
    request: &RenameDeckRequest,
) -> CommandResult<DeckDto> {
    map_error(service.rename_deck(request))
}

pub(crate) fn delete_deck(
    service: &ApplicationService,
    request: &DeleteDeckRequest,
) -> CommandResult<DeleteDeckResultDto> {
    map_error(service.delete_deck(request))
}

pub(crate) fn new_authoring_draft(
    service: &ApplicationService,
) -> CommandResult<AuthoringDraftDto> {
    map_error(service.new_authoring_draft())
}

pub(crate) fn import_media(
    service: &ApplicationService,
    request: &ImportMediaRequest,
) -> CommandResult<StudyMediaDto> {
    map_error(service.import_media(request))
}

pub(crate) fn make_cloze(
    service: &ApplicationService,
    request: MakeClozeRequest,
) -> CommandResult<AuthoringDraftDto> {
    map_error(service.make_cloze(request))
}

pub(crate) fn remove_cloze(
    service: &ApplicationService,
    request: RemoveClozeRequest,
) -> CommandResult<AuthoringDraftDto> {
    map_error(service.remove_cloze(request))
}

pub(crate) fn reorder_segments(
    service: &ApplicationService,
    request: ReorderSegmentsRequest,
) -> CommandResult<AuthoringDraftDto> {
    map_error(service.reorder_segments(request))
}

pub(crate) fn preview_authoring_draft(
    service: &ApplicationService,
    draft: &AuthoringDraftDto,
) -> CommandResult<Vec<AuthoringPreviewDto>> {
    map_error(service.preview_authoring_draft(draft))
}

pub(crate) fn save_authoring_draft(
    service: &ApplicationService,
    draft: &AuthoringDraftDto,
) -> CommandResult<AuthoringDraftDto> {
    map_error(service.save_authoring_draft(draft))
}
