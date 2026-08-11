<script lang="ts">
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";

  import type { MultipleDeckDeletion } from "../lib/deletion-activity";
  import type { DeckSummaryDto } from "../lib/generated/DeckSummaryDto";

  type Props = {
    open: boolean;
    deckIds: string[];
    decks: DeckSummaryDto[];
    deletionRunning: boolean;
    onDelete: (deletion: MultipleDeckDeletion) => void;
  };

  let {
    open = $bindable(false),
    deckIds,
    decks,
    deletionRunning,
    onDelete,
  }: Props = $props();
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

  function deleteSelectedDecks(): void {
    if (deletionRunning || deckIds.length === 0) return;
    open = false;
    onDelete({ deckIds: [...deckIds] });
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
      <AlertDialog.Cancel disabled={deletionRunning}>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        class="bg-destructive/10 text-destructive hover:bg-destructive/20"
        disabled={deletionRunning}
        onclick={deleteSelectedDecks}>Delete selected</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
