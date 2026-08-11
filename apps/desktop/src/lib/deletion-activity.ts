import type { BundleRemovalPreviewDto } from "./generated/BundleRemovalPreviewDto";
import type { DeleteDeckPhaseDto } from "./generated/DeleteDeckPhaseDto";

export type DeletionStatus = "running" | "success" | "warning" | "failure";

export type DeletionProgress = {
  phase: DeleteDeckPhaseDto;
  current: number | null;
  total: number | null;
};

export type DeletionActivity = {
  operationId: number;
  kind: "deck" | "decks" | "bundle";
  status: DeletionStatus;
  name: string;
  progress: DeletionProgress;
  message: string;
};

export type SingleDeckDeletion = {
  deckId: string;
  deckName: string;
  moveCardsToDeckId: string | null;
};

export type MultipleDeckDeletion = {
  deckIds: string[];
};

export type BundleDeletion = {
  bundle: BundleRemovalPreviewDto;
  deckIdsBeforeRemoval: string[];
};
