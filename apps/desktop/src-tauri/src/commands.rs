use meiki_application::{
    ApplicationService, AuthoringDraftDto, AuthoringPreviewDto, BundleExportRequest,
    BundleImportProgressDto, BundleImportRequest, BundleImportResultDto, BundlePreviewDto,
    BundleRemovalPreviewDto, BundleRemovalProgressDto, BundleRemovalRequest,
    BundleRemovalResultDto, CheckAnswerRequest, CreateDeckRequest, DeckCardActionRequest,
    DeckCardActionResultDto, DeckCardOverviewDto, DeckCardRequest, DeckDto, DeckSummaryDto,
    DeleteDeckProgressDto, DeleteDeckRequest, DeleteDeckResultDto, DeleteDecksRequest,
    DeleteDecksResultDto, GradeReviewRequest, GradeReviewResultDto, ImportMediaRequest,
    ImportSchedulerParametersRequest, MakeClozeRequest, PortableExportResultDto,
    ReconcileStudyQueueRequest, RemoveClozeRequest, RenameDeckRequest, ReorderSegmentsRequest,
    RevealDto, SchedulerParametersExportDto, SchedulerPolicyPreviewDto, SchedulerSettingsDto,
    StudyCardDto, StudyMediaDto, StudyPlanDto, StudyQueueEntryDto, SuspendCardRequest,
    TodayOverviewDto, TodayRequest, UndoReviewRequest, UndoReviewResultDto,
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

pub(crate) fn list_installed_bundles(
    service: &ApplicationService,
) -> CommandResult<Vec<BundleRemovalPreviewDto>> {
    map_error(service.list_installed_bundles())
}

pub(crate) fn export_bundle(
    service: &ApplicationService,
    request: &BundleExportRequest,
) -> CommandResult<PortableExportResultDto> {
    map_error(service.export_bundle(request))
}

pub(crate) fn remove_bundle(
    service: &ApplicationService,
    request: &BundleRemovalRequest,
    on_progress: impl FnMut(BundleRemovalProgressDto),
) -> CommandResult<BundleRemovalResultDto> {
    map_error(service.remove_bundle(request, on_progress))
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
    on_progress: impl FnMut(DeleteDeckProgressDto),
) -> CommandResult<DeleteDeckResultDto> {
    map_error(service.delete_deck(request, on_progress))
}

pub(crate) fn delete_decks(
    service: &ApplicationService,
    request: &DeleteDecksRequest,
    on_progress: impl FnMut(DeleteDeckProgressDto),
) -> CommandResult<DeleteDecksResultDto> {
    map_error(service.delete_decks(request, on_progress))
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
