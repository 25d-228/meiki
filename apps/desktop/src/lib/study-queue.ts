import type { TodayOverviewDto } from "./generated/TodayOverviewDto";

export const studyQueueKey = "meiki-active-study-queue";

export type StudyQueueSession = {
  deckId: string;
  cardIds: string[];
  position: number;
  startedAtMs: number;
};

export function startStudyQueue(overview: TodayOverviewDto): StudyQueueSession {
  const queue: StudyQueueSession = {
    deckId: overview.deck_id,
    cardIds: overview.queue.map((card) => card.card_id),
    position: 0,
    startedAtMs: Date.now(),
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
      typeof value.deckId !== "string" ||
      !Array.isArray(value.cardIds) ||
      value.cardIds.length === 0 ||
      value.cardIds.some((cardId) => typeof cardId !== "string") ||
      !Number.isInteger(value.position) ||
      (value.position ?? -1) < 0 ||
      (value.position ?? 0) > value.cardIds.length ||
      typeof value.startedAtMs !== "number"
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
  return Math.max(0, queue.cardIds.length - queue.position);
}
