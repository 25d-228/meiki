import type { GradeDto } from "./generated/GradeDto";
import type { GradeReviewRequest } from "./generated/GradeReviewRequest";
import type { StudyQueueEntryDto } from "./generated/StudyQueueEntryDto";
import type { TodayOverviewDto } from "./generated/TodayOverviewDto";

export const studyQueueKey = "meiki-active-study-queue";
export const studyQueueVersion = 2;

export type StudyQueueSession = {
  version: typeof studyQueueVersion;
  deckId: string;
  entries: StudyQueueEntryDto[];
  position: number;
  startedAtMs: number;
  pendingReview: GradeReviewRequest | null;
};

export function startStudyQueue(overview: TodayOverviewDto): StudyQueueSession {
  const queue: StudyQueueSession = {
    version: studyQueueVersion,
    deckId: overview.deck_id,
    entries: overview.queue.map((card) => ({
      card_id: card.card_id,
      card_content_version: card.card_content_version,
      schedule_version: card.schedule_version,
    })),
    position: 0,
    startedAtMs: Date.now(),
    pendingReview: null,
  };
  writeStudyQueue(queue);
  return queue;
}

export function readStudyQueue(): StudyQueueSession | null {
  const stored = localStorage.getItem(studyQueueKey);
  if (!stored) return null;
  try {
    const value = JSON.parse(stored) as Partial<StudyQueueSession>;
    if (
      value.version !== studyQueueVersion ||
      typeof value.deckId !== "string" ||
      value.deckId.length === 0 ||
      !Array.isArray(value.entries) ||
      value.entries.length === 0 ||
      value.entries.some((entry) => !isQueueEntry(entry)) ||
      !isNonnegativeInteger(value.position) ||
      value.position > value.entries.length ||
      !Number.isFinite(value.startedAtMs) ||
      (value.pendingReview !== null &&
        !isGradeReviewRequest(value.pendingReview)) ||
      (isGradeReviewRequest(value.pendingReview) &&
        (value.position >= value.entries.length ||
          value.entries[value.position]?.card_id !==
            value.pendingReview.card_id ||
          value.entries[value.position]?.card_content_version !==
            value.pendingReview.card_content_version ||
          value.entries[value.position]?.schedule_version !==
            value.pendingReview.schedule_version))
    ) {
      throw new Error("invalid queue");
    }
    return value as StudyQueueSession;
  } catch {
    localStorage.removeItem(studyQueueKey);
    return null;
  }
}

export function writeStudyQueue(queue: StudyQueueSession): void {
  localStorage.setItem(studyQueueKey, JSON.stringify(queue));
}

export function clearStudyQueue(): void {
  localStorage.removeItem(studyQueueKey);
}

export function remainingStudyCards(queue: StudyQueueSession): number {
  return Math.max(0, queue.entries.length - queue.position);
}

export function remainingQueueEntries(
  queue: StudyQueueSession,
): StudyQueueEntryDto[] {
  return queue.entries.slice(queue.position);
}

function isQueueEntry(value: unknown): value is StudyQueueEntryDto {
  if (!value || typeof value !== "object") return false;
  const entry = value as Partial<StudyQueueEntryDto>;
  return (
    typeof entry.card_id === "string" &&
    entry.card_id.length > 0 &&
    isNonnegativeInteger(entry.card_content_version) &&
    isNonnegativeInteger(entry.schedule_version)
  );
}

function isGradeReviewRequest(value: unknown): value is GradeReviewRequest {
  if (!value || typeof value !== "object") return false;
  const request = value as Partial<GradeReviewRequest>;
  return (
    typeof request.review_event_id === "string" &&
    request.review_event_id.length > 0 &&
    typeof request.card_id === "string" &&
    request.card_id.length > 0 &&
    isNonnegativeInteger(request.card_content_version) &&
    isNonnegativeInteger(request.schedule_version) &&
    typeof request.raw_response === "string" &&
    isGrade(request.chosen_grade) &&
    isNonnegativeInteger(request.response_duration_ms)
  );
}

function isNonnegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}

function isGrade(value: unknown): value is GradeDto {
  return (
    value === "again" ||
    value === "hard" ||
    value === "good" ||
    value === "easy"
  );
}
