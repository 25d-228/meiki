<script lang="ts">
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";

  import { api } from "../lib/api";
  import type { DeckSummaryDto } from "../lib/generated/DeckSummaryDto";
  import type { DeleteDeckProgressDto } from "../lib/generated/DeleteDeckProgressDto";
  import type { DeleteDecksResultDto } from "../lib/generated/DeleteDecksResultDto";
  import ProgressBar from "./ProgressBar.svelte";

  type Props = {
    open: boolean;
    deckIds: string[];
    decks: DeckSummaryDto[];
    onCommitted: (result: DeleteDecksResultDto) => void | Promise<void>;
    onFinished: (result: DeleteDecksResultDto) => void;
  };

  let {
    open = $bindable(false),
    deckIds,
    decks,
    onCommitted,
    onFinished,
  }: Props = $props();
  let busy = $state(false);
  let progress = $state<DeleteDeckProgressDto | null>(null);
  let progressDialogOpen = $state(false);
  let failure = $state("");
  let cleanupWarning = $state("");
  let result = $state<DeleteDecksResultDto | null>(null);
  let ordinaryDeckCount = $derived(
    decks.filter((deck) => !deck.is_bundle_stage).length,
  );
  let bundleStageCount = $derived(
    decks.filter((deck) => deck.is_bundle_stage).length,
  );
  let ordinaryCardCount = $derived(
    decks
      .filter((deck) => !deck.is_bundle_stage)
      .reduce((total, deck) => total + deck.total_cards, 0),
  );

  async function deleteSelectedDecks(): Promise<void> {
    if (busy || deckIds.length === 0) return;
    busy = true;
    failure = "";
    cleanupWarning = "";
    progress = null;
    result = null;
    open = false;
    progressDialogOpen = true;
    let deletionResult: DeleteDecksResultDto;
    try {
      deletionResult = await api.deleteDecks(
        {
          deck_ids: deckIds,
          now_ms: Date.now(),
        },
        (update) => (progress = update),
      );
    } catch {
      failure = "Could not delete the selected decks. Try again.";
      busy = false;
      return;
    }

    result = deletionResult;
    await onCommitted(deletionResult);
    busy = false;
    if (deletionResult.media_cleanup_warning) {
      cleanupWarning = deletionResult.media_cleanup_warning;
      return;
    }
    progressDialogOpen = false;
    onFinished(deletionResult);
  }

  function closeProgress(): void {
    if (busy) return;
    progressDialogOpen = false;
    if (cleanupWarning && result) onFinished(result);
  }

  function progressLabel(value: DeleteDeckProgressDto): string {
    if (value.phase === "preparing") return "Preparing";
    if (value.phase === "removing_cards") return "Removing cards";
    if (value.phase === "cleaning_audio") return "Cleaning audio";
    return "Finalizing";
  }

  function countLabel(count: number, singular: string, plural: string): string {
    return `${count.toLocaleString()} ${count === 1 ? singular : plural}`;
  }
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>
        Delete {countLabel(deckIds.length, "selected deck", "selected decks")}?
      </AlertDialog.Title>
      <AlertDialog.Description>
        This deletes the complete selection in one operation.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <div class="grid gap-2 text-sm text-muted-foreground">
      {#if ordinaryDeckCount > 0}
        <p>
          {countLabel(ordinaryCardCount, "card", "cards")} in {countLabel(
            ordinaryDeckCount,
            "ordinary deck",
            "ordinary decks",
          )} will be moved to Trash.
        </p>
      {/if}
      {#if bundleStageCount > 0}
        <p>
          Bundled content in {countLabel(
            bundleStageCount,
            "bundle stage",
            "bundle stages",
          )} will be permanently removed. Personal cards in those stages will be moved
          to Trash.
        </p>
      {/if}
    </div>
    <AlertDialog.Footer>
      <AlertDialog.Cancel disabled={busy}>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        class="bg-destructive/10 text-destructive hover:bg-destructive/20"
        disabled={busy}
        onclick={() => void deleteSelectedDecks()}
        >Delete selected</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>

<Dialog.Root
  open={progressDialogOpen}
  onOpenChange={(nextOpen) => {
    if (!nextOpen) closeProgress();
  }}
>
  <Dialog.Content class="rounded-none sm:max-w-md" showCloseButton={!busy}>
    <Dialog.Header>
      <Dialog.Title>
        {cleanupWarning
          ? "Decks deleted"
          : failure
            ? "Decks were not deleted"
            : `Deleting ${countLabel(deckIds.length, "deck", "decks")}`}
      </Dialog.Title>
      <Dialog.Description>
        {cleanupWarning
          ? "The collection was updated, but audio cleanup needs attention."
          : failure
            ? "Your collection was left unchanged."
            : "Keep Meiki open while this finishes."}
      </Dialog.Description>
    </Dialog.Header>

    {#if cleanupWarning}
      <Alert.Root role="alert">
        <Alert.Title>{cleanupWarning}</Alert.Title>
      </Alert.Root>
    {:else if failure}
      <Alert.Root variant="destructive" role="alert">
        <Alert.Title>{failure}</Alert.Title>
      </Alert.Root>
    {:else if progress}
      <div class="grid gap-2" role="status" aria-live="polite">
        <strong>{progressLabel(progress)}</strong>
        <ProgressBar
          label={progressLabel(progress)}
          current={progress.current}
          total={progress.total}
        />
        {#if progress.current !== null && progress.total !== null}
          <span class="text-sm text-muted-foreground">
            {progress.current.toLocaleString()} / {progress.total.toLocaleString()}
          </span>
        {/if}
      </div>
    {/if}

    {#if !busy}
      <Dialog.Footer>
        <Button onclick={closeProgress}>Close</Button>
      </Dialog.Footer>
    {/if}
  </Dialog.Content>
</Dialog.Root>
