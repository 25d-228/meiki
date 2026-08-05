import { Channel, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type { CheckAnswerRequest } from "./generated/CheckAnswerRequest";
import type { AuthoringDraftDto } from "./generated/AuthoringDraftDto";
import type { AuthoringPreviewDto } from "./generated/AuthoringPreviewDto";
import type { GradeReviewRequest } from "./generated/GradeReviewRequest";
import type { GradeReviewResultDto } from "./generated/GradeReviewResultDto";
import type { RevealDto } from "./generated/RevealDto";
import type { StudyCardDto } from "./generated/StudyCardDto";
import type { SuspendCardRequest } from "./generated/SuspendCardRequest";
import type { UndoReviewRequest } from "./generated/UndoReviewRequest";
import type { UndoReviewResultDto } from "./generated/UndoReviewResultDto";
import type { MakeClozeRequest } from "./generated/MakeClozeRequest";
import type { RemoveClozeRequest } from "./generated/RemoveClozeRequest";
import type { ReorderSegmentsRequest } from "./generated/ReorderSegmentsRequest";
import type { SchedulerSettingsDto } from "./generated/SchedulerSettingsDto";
import type { SchedulerPolicyPreviewDto } from "./generated/SchedulerPolicyPreviewDto";
import type { SchedulerParametersExportDto } from "./generated/SchedulerParametersExportDto";
import type { ImportSchedulerParametersRequest } from "./generated/ImportSchedulerParametersRequest";
import type { UpdateSchedulerSettingsRequest } from "./generated/UpdateSchedulerSettingsRequest";
import type { DirectionDto } from "./generated/DirectionDto";
import type { ImportMediaRequest } from "./generated/ImportMediaRequest";
import type { LibraryBulkRequest } from "./generated/LibraryBulkRequest";
import type { LibraryBulkResultDto } from "./generated/LibraryBulkResultDto";
import type { LibraryOverviewDto } from "./generated/LibraryOverviewDto";
import type { LibraryRequest } from "./generated/LibraryRequest";
import type { MediaRoleDto } from "./generated/MediaRoleDto";
import type { StudyMediaDto } from "./generated/StudyMediaDto";
import type { TodayOverviewDto } from "./generated/TodayOverviewDto";
import type { TodayRequest } from "./generated/TodayRequest";
import type { ArchiveExportRequest } from "./generated/ArchiveExportRequest";
import type { ArchiveAddDeckRequest } from "./generated/ArchiveAddDeckRequest";
import type { ArchiveAddDeckResultDto } from "./generated/ArchiveAddDeckResultDto";
import type { ArchiveImportRequest } from "./generated/ArchiveImportRequest";
import type { ArchiveImportResultDto } from "./generated/ArchiveImportResultDto";
import type { BackupDto } from "./generated/BackupDto";
import type { PortableArchivePreviewDto } from "./generated/PortableArchivePreviewDto";
import type { PortableExportResultDto } from "./generated/PortableExportResultDto";
import type { ReconcileStudyQueueRequest } from "./generated/ReconcileStudyQueueRequest";
import type { StudyQueueEntryDto } from "./generated/StudyQueueEntryDto";
import type { StudyPlanDto } from "./generated/StudyPlanDto";
import type { CreateDeckRequest } from "./generated/CreateDeckRequest";
import type { DeckDto } from "./generated/DeckDto";
import type { DeckSummaryDto } from "./generated/DeckSummaryDto";
import type { DeckCardActionRequest } from "./generated/DeckCardActionRequest";
import type { DeckCardActionResultDto } from "./generated/DeckCardActionResultDto";
import type { DeckCardOverviewDto } from "./generated/DeckCardOverviewDto";
import type { DeckCardRequest } from "./generated/DeckCardRequest";
import type { DeleteDeckRequest } from "./generated/DeleteDeckRequest";
import type { DeleteDeckResultDto } from "./generated/DeleteDeckResultDto";
import type { RenameDeckRequest } from "./generated/RenameDeckRequest";
import type { BundlePreviewDto } from "./generated/BundlePreviewDto";
import type { BundleImportRequest } from "./generated/BundleImportRequest";
import type { BundleImportProgressDto } from "./generated/BundleImportProgressDto";
import type { BundleImportResultDto } from "./generated/BundleImportResultDto";
import type { BundleExportRequest } from "./generated/BundleExportRequest";
import type { BundleRemovalPreviewDto } from "./generated/BundleRemovalPreviewDto";
import type { BundleRemovalProgressDto } from "./generated/BundleRemovalProgressDto";
import type { BundleRemovalRequest } from "./generated/BundleRemovalRequest";
import type { BundleRemovalResultDto } from "./generated/BundleRemovalResultDto";

function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const testInvoke = window.__MEIKI_TEST_INVOKE__;
  return testInvoke
    ? testInvoke<T>(command, args)
    : tauriInvoke<T>(command, args);
}

export const api = {
  getStudyCard(cardId: string): Promise<StudyCardDto> {
    return invoke("get_study_card", { cardId });
  },

  getTodayOverview(request: TodayRequest): Promise<TodayOverviewDto> {
    return invoke("get_today_overview", { request });
  },

  prepareStudy(request: TodayRequest): Promise<StudyPlanDto> {
    return invoke("prepare_study", { request });
  },

  reconcileStudyQueue(
    request: ReconcileStudyQueueRequest,
  ): Promise<StudyQueueEntryDto[]> {
    return invoke("reconcile_study_queue", { request });
  },

  getLibrary(request: LibraryRequest): Promise<LibraryOverviewDto> {
    return invoke("get_library", { request });
  },

  applyLibraryBulkAction(
    request: LibraryBulkRequest,
  ): Promise<LibraryBulkResultDto> {
    return invoke("apply_library_bulk_action", { request });
  },

  getDeckCards(request: DeckCardRequest): Promise<DeckCardOverviewDto> {
    return invoke("get_deck_cards", { request });
  },

  applyDeckCardAction(
    request: DeckCardActionRequest,
  ): Promise<DeckCardActionResultDto> {
    return invoke("apply_deck_card_action", { request });
  },

  exportArchive(
    request: ArchiveExportRequest,
  ): Promise<PortableExportResultDto> {
    return invoke("export_archive", { request });
  },

  async pickArchiveFile(): Promise<string | null> {
    const testPick = window.__MEIKI_TEST_PICK_ARCHIVE__;
    if (testPick) return testPick();
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Meiki archive", extensions: ["meiki"] }],
    });
    return typeof selected === "string" ? selected : null;
  },

  previewArchive(path: string): Promise<PortableArchivePreviewDto> {
    return invoke("preview_archive", { path });
  },

  previewBundle(path: string): Promise<BundlePreviewDto> {
    return invoke("preview_bundle", { path });
  },

  importBundle(
    request: BundleImportRequest,
    onProgress: (progress: BundleImportProgressDto) => void,
  ): Promise<BundleImportResultDto> {
    if (window.__MEIKI_TEST_INVOKE__) {
      window.__MEIKI_TEST_BUNDLE_PROGRESS__ = onProgress;
      return invoke<BundleImportResultDto>("import_bundle", {
        request,
      }).finally(() => {
        delete window.__MEIKI_TEST_BUNDLE_PROGRESS__;
      });
    }
    const progress = new Channel<BundleImportProgressDto>(onProgress);
    return invoke("import_bundle", { request, onProgress: progress });
  },

  listInstalledBundles(): Promise<BundleRemovalPreviewDto[]> {
    return invoke("list_installed_bundles");
  },

  exportBundle(request: BundleExportRequest): Promise<PortableExportResultDto> {
    return invoke("export_bundle", { request });
  },

  removeBundle(
    request: BundleRemovalRequest,
    onProgress: (progress: BundleRemovalProgressDto) => void,
  ): Promise<BundleRemovalResultDto> {
    if (window.__MEIKI_TEST_INVOKE__) {
      window.__MEIKI_TEST_BUNDLE_REMOVAL_PROGRESS__ = onProgress;
      return invoke<BundleRemovalResultDto>("remove_bundle", {
        request,
      }).finally(() => {
        delete window.__MEIKI_TEST_BUNDLE_REMOVAL_PROGRESS__;
      });
    }
    const progress = new Channel<BundleRemovalProgressDto>(onProgress);
    return invoke("remove_bundle", { request, onProgress: progress });
  },

  addArchiveDeck(
    request: ArchiveAddDeckRequest,
  ): Promise<ArchiveAddDeckResultDto> {
    return invoke("add_archive_deck", { request });
  },

  importArchive(
    request: ArchiveImportRequest,
  ): Promise<ArchiveImportResultDto> {
    return invoke("import_archive", { request });
  },

  listBackups(): Promise<BackupDto[]> {
    return invoke("list_backups");
  },

  restoreBackup(path: string, confirmation: string): Promise<BackupDto> {
    return invoke("restore_backup", { path, confirmation });
  },

  getAuthoringDraftForCard(cardId: string): Promise<AuthoringDraftDto> {
    return invoke("get_authoring_draft_for_card", { cardId });
  },

  checkAnswer(request: CheckAnswerRequest): Promise<RevealDto> {
    return invoke("check_answer", { request });
  },

  gradeReview(request: GradeReviewRequest): Promise<GradeReviewResultDto> {
    return invoke("grade_review", { request });
  },

  suspendCard(request: SuspendCardRequest): Promise<StudyCardDto> {
    return invoke("suspend_card", { request });
  },

  undoReview(request: UndoReviewRequest): Promise<UndoReviewResultDto> {
    return invoke("undo_review", { request });
  },

  getSchedulerSettings(deckId: string): Promise<SchedulerSettingsDto> {
    return invoke("get_scheduler_settings", { deckId });
  },

  updateSchedulerSettings(
    request: UpdateSchedulerSettingsRequest,
  ): Promise<SchedulerSettingsDto> {
    return invoke("update_scheduler_settings", { request });
  },

  previewSchedulerPolicy(
    request: UpdateSchedulerSettingsRequest,
  ): Promise<SchedulerPolicyPreviewDto> {
    return invoke("preview_scheduler_policy", { request });
  },

  async pickSchedulerParametersFile(): Promise<string | null> {
    const testPick = window.__MEIKI_TEST_PICK_SCHEDULER_PARAMETERS__;
    if (testPick) return testPick();
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Meiki scheduler parameters", extensions: ["json"] }],
    });
    return typeof selected === "string" ? selected : null;
  },

  importSchedulerParameters(
    request: ImportSchedulerParametersRequest,
  ): Promise<SchedulerSettingsDto> {
    return invoke("import_scheduler_parameters", { request });
  },

  exportSchedulerParameters(
    deckId: string,
  ): Promise<SchedulerParametersExportDto> {
    return invoke("export_scheduler_parameters", { deckId });
  },

  listDecks(): Promise<DeckDto[]> {
    return invoke("list_decks");
  },

  listDeckSummaries(nowMs: number): Promise<DeckSummaryDto[]> {
    return invoke("list_deck_summaries", { nowMs });
  },

  createDeck(request: CreateDeckRequest): Promise<DeckDto> {
    return invoke("create_deck", { request });
  },

  renameDeck(request: RenameDeckRequest): Promise<DeckDto> {
    return invoke("rename_deck", { request });
  },

  deleteDeck(request: DeleteDeckRequest): Promise<DeleteDeckResultDto> {
    return invoke("delete_deck", { request });
  },

  newAuthoringDraft(): Promise<AuthoringDraftDto> {
    return invoke("new_authoring_draft");
  },

  async pickMediaFile(role: MediaRoleDto): Promise<string | null> {
    const testPick = window.__MEIKI_TEST_PICK_FILE__;
    if (testPick) return testPick(role);
    const audio = role !== "reveal_image";
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        audio
          ? {
              name: "Audio",
              extensions: ["mp3", "m4a", "opus", "ogg", "flac", "wav", "aac"],
            }
          : {
              name: "Images",
              extensions: ["png", "jpg", "jpeg", "gif", "webp"],
            },
      ],
    });
    return typeof selected === "string" ? selected : null;
  },

  importMedia(
    path: string,
    role: MediaRoleDto,
    languageTag: string | null,
    direction: DirectionDto,
  ): Promise<StudyMediaDto> {
    const request: ImportMediaRequest = {
      path,
      role,
      language_tag: languageTag,
      direction,
    };
    return invoke("import_media", { request });
  },

  makeCloze(request: MakeClozeRequest): Promise<AuthoringDraftDto> {
    return invoke("make_cloze", { request });
  },

  removeCloze(request: RemoveClozeRequest): Promise<AuthoringDraftDto> {
    return invoke("remove_cloze", { request });
  },

  reorderSegments(request: ReorderSegmentsRequest): Promise<AuthoringDraftDto> {
    return invoke("reorder_segments", { request });
  },

  previewAuthoringDraft(
    draft: AuthoringDraftDto,
  ): Promise<AuthoringPreviewDto[]> {
    return invoke("preview_authoring_draft", { draft });
  },

  saveAuthoringDraft(draft: AuthoringDraftDto): Promise<AuthoringDraftDto> {
    return invoke("save_authoring_draft", { draft });
  },
};
