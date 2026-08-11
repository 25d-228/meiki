<script lang="ts">
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import type { SingleDeckDeletion } from "../lib/deletion-activity";
  import type { DeckCardDeckDto } from "../lib/generated/DeckCardDeckDto";

  type Props = {
    open: boolean;
    deckId: string;
    deckName: string;
    isBundleStage: boolean;
    cardCount: number;
    destinationDecks: DeckCardDeckDto[];
    deletionRunning: boolean;
    onDelete: (deletion: SingleDeckDeletion) => void;
  };

  let {
    open = $bindable(false),
    deckId,
    deckName,
    isBundleStage,
    cardCount,
    destinationDecks,
    deletionRunning,
    onDelete,
  }: Props = $props();
  let moveDialogOpen = $state(false);
  let destinationDeckId = $state("");

  function openMoveDialog(): void {
    destinationDeckId = destinationDecks[0]?.id ?? "";
    open = false;
    moveDialogOpen = true;
  }

  function deleteDeck(moveCardsToDeckId: string | null): void {
    if (deletionRunning) return;
    open = false;
    moveDialogOpen = false;
    onDelete({ deckId, deckName, moveCardsToDeckId });
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
      <AlertDialog.Cancel disabled={deletionRunning}>Cancel</AlertDialog.Cancel>
      <Button
        variant="outline"
        disabled={deletionRunning || destinationDecks.length === 0}
        onclick={openMoveDialog}>Move cards instead</Button
      >
      <AlertDialog.Action
        class="bg-destructive/10 text-destructive hover:bg-destructive/20"
        disabled={deletionRunning}
        onclick={() => deleteDeck(null)}>Delete deck</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>

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
        disabled={deletionRunning}
        onclick={() => (moveDialogOpen = false)}>Cancel</Button
      >
      <Button
        variant="destructive"
        disabled={deletionRunning || !destinationDeckId}
        onclick={() => deleteDeck(destinationDeckId)}
        >Move cards and delete</Button
      >
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
