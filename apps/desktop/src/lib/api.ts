import { invoke as tauriInvoke } from "@tauri-apps/api/core";

import type { CheckAnswerRequest } from "./generated/CheckAnswerRequest";
import type { AuthoringDraftDto } from "./generated/AuthoringDraftDto";
import type { AuthoringPreviewDto } from "./generated/AuthoringPreviewDto";
import type { GradeReviewRequest } from "./generated/GradeReviewRequest";
import type { GradeReviewResultDto } from "./generated/GradeReviewResultDto";
import type { RevealDto } from "./generated/RevealDto";
import type { StudyCardDto } from "./generated/StudyCardDto";
import type { MakeClozeRequest } from "./generated/MakeClozeRequest";
import type { RemoveClozeRequest } from "./generated/RemoveClozeRequest";
import type { ReorderSegmentsRequest } from "./generated/ReorderSegmentsRequest";
import type { RebuildSchedulerResultDto } from "./generated/RebuildSchedulerResultDto";
import type { SchedulerSettingsDto } from "./generated/SchedulerSettingsDto";
import type { SchedulerDiagnosticsExportDto } from "./generated/SchedulerDiagnosticsExportDto";
import type { UpdateSchedulerSettingsRequest } from "./generated/UpdateSchedulerSettingsRequest";

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
  initializeCollection(): Promise<StudyCardDto> {
    return invoke("initialize_collection");
  },

  getStudyCard(cardId: string): Promise<StudyCardDto> {
    return invoke("get_study_card", { cardId });
  },

  checkAnswer(request: CheckAnswerRequest): Promise<RevealDto> {
    return invoke("check_answer", { request });
  },

  gradeReview(request: GradeReviewRequest): Promise<GradeReviewResultDto> {
    return invoke("grade_review", { request });
  },

  getSchedulerSettings(deckId: string): Promise<SchedulerSettingsDto> {
    return invoke("get_scheduler_settings", { deckId });
  },

  updateSchedulerSettings(
    request: UpdateSchedulerSettingsRequest,
  ): Promise<SchedulerSettingsDto> {
    return invoke("update_scheduler_settings", { request });
  },

  optimizeScheduler(deckId: string): Promise<SchedulerSettingsDto> {
    return invoke("optimize_scheduler", { deckId });
  },

  rollbackScheduler(deckId: string): Promise<SchedulerSettingsDto> {
    return invoke("rollback_scheduler", { deckId });
  },

  rebuildScheduler(deckId: string): Promise<RebuildSchedulerResultDto> {
    return invoke("rebuild_scheduler", { deckId });
  },

  exportSchedulerDiagnostics(
    deckId: string,
  ): Promise<SchedulerDiagnosticsExportDto> {
    return invoke("export_scheduler_diagnostics", { deckId });
  },

  newAuthoringDraft(): Promise<AuthoringDraftDto> {
    return invoke("new_authoring_draft");
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
