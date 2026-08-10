<script lang="ts">
  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import type { DeckCardDeckDto } from "../lib/generated/DeckCardDeckDto";
  import type { DeleteDeckProgressDto } from "../lib/generated/DeleteDeckProgressDto";
  import type { DeleteDeckResultDto } from "../lib/generated/DeleteDeckResultDto";
  import ProgressBar from "./ProgressBar.svelte";

  type Props = {
    open: boolean;
    deckId: string;
    deckName: string;
    isBundleStage: boolean;
    cardCount: number;
    destinationDecks: DeckCardDeckDto[];
    onCommitted?: (result: DeleteDeckResultDto) => void | Promise<void>;
    onFinished: (result: DeleteDeckResultDto) => void;
  };

  let {
    open = $bindable(false),
    deckId,
    deckName,
    isBundleStage,
    cardCount,
    destinationDecks,
    onCommitted,
    onFinished,
  }: Props = $props();
  let moveDialogOpen = $state(false);
  let destinationDeckId = $state("");
  let busy = $state(false);
  let progress = $state<DeleteDeckProgressDto | null>(null);
  let progressDialogOpen = $state(false);
  let failure = $state("");
  let cleanupWarning = $state("");
  let result = $state<DeleteDeckResultDto | null>(null);

  function openMoveDialog(): void {
    destinationDeckId = destinationDecks[0]?.id ?? "";
    open = false;
    moveDialogOpen = true;
  }

  async function deleteDeck(moveCardsToDeckId: string | null): Promise<void> {
    if (busy) return;
    busy = true;
    failure = "";
    cleanupWarning = "";
    progress = null;
    result = null;
    open = false;
    moveDialogOpen = false;
    progressDialogOpen = true;
    let deletionResult: DeleteDeckResultDto;
    try {
      deletionResult = await api.deleteDeck(
        {
          deck_id: deckId,
          move_cards_to_deck_id: moveCardsToDeckId,
          confirmation: deckName,
          now_ms: Date.now(),
        },
        (update) => (progress = update),
      );
    } catch {
      failure = "Could not delete the deck. Try again.";
      busy = false;
      return;
    }

    result = deletionResult;
    if (onCommitted) await onCommitted(deletionResult);
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

  function cardCountLabel(count: number): string {
    return `${count} ${count === 1 ? "card" : "cards"}`;
  }
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete “{deckName}”?</AlertDialog.Title>
      <AlertDialog.Description>
        {#if isBundleStage}
          Bundled cards in this deck will be permanently removed. Personal cards
          will be moved to Trash.
        {:else}
          Its {cardCountLabel(cardCount)} will be moved to Trash.
        {/if}
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel disabled={busy}>Cancel</AlertDialog.Cancel>
      <Button
        variant="outline"
        disabled={busy || destinationDecks.length === 0}
        onclick={openMoveDialog}>Move cards instead</Button
      >
      <AlertDialog.Action
        class="bg-destructive/10 text-destructive hover:bg-destructive/20"
        disabled={busy}
        onclick={() => void deleteDeck(null)}>Delete deck</AlertDialog.Action
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
          ? "Deck deleted"
          : failure
            ? "Deck was not deleted"
            : `Deleting “${deckName}”`}
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

<Dialog.Root bind:open={moveDialogOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Move cards instead</Dialog.Title>
      <Dialog.Description>
        Move active cards to another deck, then delete “{deckName}”.
      </Dialog.Description>
    </Dialog.Header>
    <div class="grid gap-2">
      <Label for="delete-destination-deck">Destination deck</Label>
      <select
        id="delete-destination-deck"
        class="w-full"
        bind:value={destinationDeckId}
      >
        {#each destinationDecks as deck (deck.id)}
          <option value={deck.id}>{deck.name}</option>
        {/each}
      </select>
    </div>
    <Dialog.Footer>
      <Button
        variant="outline"
        disabled={busy}
        onclick={() => (moveDialogOpen = false)}>Cancel</Button
      >
      <Button
        variant="destructive"
        disabled={busy || !destinationDeckId}
        onclick={() => void deleteDeck(destinationDeckId)}
        >Move cards and delete</Button
      >
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
