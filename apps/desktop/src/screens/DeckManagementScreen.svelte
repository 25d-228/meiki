<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteDate } from "svelte/reactivity";

  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import type { DeckCardActionDto } from "../lib/generated/DeckCardActionDto";
  import type { DeckCardDto } from "../lib/generated/DeckCardDto";
  import type { DeckCardOverviewDto } from "../lib/generated/DeckCardOverviewDto";
  import type { DeckCardStatusDto } from "../lib/generated/DeckCardStatusDto";
  import type { DeckCardTrashDto } from "../lib/generated/DeckCardTrashDto";
  import type { SchedulerSettingsDto } from "../lib/generated/SchedulerSettingsDto";
  import { localDayBounds } from "../lib/local-day";

  type Props = {
    selectedDeckId: string;
    deckName: string;
    onBack: () => void;
    onCreate: () => void;
    onDeleted: () => void;
    onEdit: (cardId: string) => void;
    onRename: (name: string) => void;
  };

  const defaultDeckId = "default-deck";
  const pageSize = 25;

  let {
    selectedDeckId,
    deckName,
    onBack,
    onCreate,
    onDeleted,
    onEdit,
    onRename,
  }: Props = $props();
  let overview = $state<DeckCardOverviewDto | null>(null);
  let query = $state("");
  let trash = $state<DeckCardTrashDto>("active");
  let offset = $state(0);
  let loading = $state(true);
  let busyCardId = $state("");
  let error = $state("");
  let notice = $state("");
  let movingCard = $state<DeckCardDto | null>(null);
  let destinationDeckId = $state("");
  let renameDialogOpen = $state(false);
  let renameName = $state("");
  let timeOverrideDialogOpen = $state(false);
  let deckSettings = $state<SchedulerSettingsDto | null>(null);
  let useTimeOverride = $state(false);
  let timeOverrideMinutes = $state(30);
  let deleteDialogOpen = $state(false);
  let moveBeforeDeleteDialogOpen = $state(false);
  let deleteCardCount = $state(0);
  let busyDeckAction = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    void loadCards();
    return () => {
      if (searchTimer) clearTimeout(searchTimer);
    };
  });

  async function loadCards(): Promise<void> {
    loading = true;
    error = "";
    try {
      let nextOverview = await api.getDeckCards({
        deck_id: selectedDeckId,
        query,
        trash,
        now_ms: Date.now(),
        offset,
        limit: pageSize,
      });
      const lastOffset = nextOverview.total_matches
        ? Math.floor((nextOverview.total_matches - 1) / pageSize) * pageSize
        : 0;
      if (offset > lastOffset) {
        offset = lastOffset;
        nextOverview = await api.getDeckCards({
          deck_id: selectedDeckId,
          query,
          trash,
          now_ms: Date.now(),
          offset,
          limit: pageSize,
        });
      }
      overview = nextOverview;
      destinationDeckId =
        overview.decks.find((deck) => deck.id !== selectedDeckId)?.id ?? "";
    } catch (reason) {
      error = message(reason);
    } finally {
      loading = false;
    }
  }

  function scheduleSearch(): void {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      offset = 0;
      void loadCards();
    }, 160);
  }

  function show(nextTrash: DeckCardTrashDto): void {
    trash = nextTrash;
    offset = 0;
    notice = "";
    void loadCards();
  }

  function changePage(nextOffset: number): void {
    offset = Math.max(0, nextOffset);
    void loadCards();
  }

  async function runAction(
    card: DeckCardDto,
    action: DeckCardActionDto,
    destinationDeckId: string | null = null,
  ): Promise<void> {
    if (busyCardId) return;
    busyCardId = card.id;
    error = "";
    notice = "";
    try {
      await api.applyDeckCardAction({
        deck_id: selectedDeckId,
        card_ids: [card.id],
        action,
        destination_deck_id: destinationDeckId,
        now_ms: Date.now(),
      });
      notice = actionNotice(action);
      movingCard = null;
      await loadCards();
    } catch (reason) {
      error = message(reason);
    } finally {
      busyCardId = "";
    }
  }

  function openMove(card: DeckCardDto): void {
    movingCard = card;
    destinationDeckId =
      overview?.decks.find((deck) => deck.id !== selectedDeckId)?.id ?? "";
  }

  function openRename(): void {
    renameName = deckName;
    renameDialogOpen = true;
  }

  async function renameDeck(): Promise<void> {
    if (!renameName.trim() || busyDeckAction) return;
    busyDeckAction = true;
    error = "";
    notice = "";
    try {
      const renamed = await api.renameDeck({
        deck_id: selectedDeckId,
        name: renameName,
        now_ms: Date.now(),
      });
      renameDialogOpen = false;
      onRename(renamed.name);
      notice = `Renamed deck to “${renamed.name}”.`;
    } catch (reason) {
      error = message(reason);
    } finally {
      busyDeckAction = false;
    }
  }

  async function openTimeOverride(): Promise<void> {
    if (busyDeckAction) return;
    busyDeckAction = true;
    error = "";
    try {
      deckSettings = await api.getSchedulerSettings(selectedDeckId);
      useTimeOverride = deckSettings.deck_daily_time_budget_minutes !== null;
      timeOverrideMinutes =
        deckSettings.deck_daily_time_budget_minutes ??
        deckSettings.collection_daily_time_budget_minutes;
      timeOverrideDialogOpen = true;
    } catch (reason) {
      error = message(reason);
    } finally {
      busyDeckAction = false;
    }
  }

  async function saveTimeOverride(): Promise<void> {
    if (!deckSettings || busyDeckAction) return;
    busyDeckAction = true;
    error = "";
    notice = "";
    try {
      const now = new SvelteDate();
      const { start } = localDayBounds(now, deckSettings.day_boundary_minutes);
      deckSettings = await api.updateSchedulerSettings({
        deck_id: selectedDeckId,
        scheduling_mode: deckSettings.scheduling_mode,
        collection_daily_time_budget_minutes:
          deckSettings.collection_daily_time_budget_minutes,
        deck_daily_time_budget_minutes: useTimeOverride
          ? timeOverrideMinutes
          : null,
        target_retention_basis_points:
          deckSettings.target_retention_basis_points,
        new_cards_per_day: deckSettings.new_cards_per_day,
        maximum_interval_days: deckSettings.maximum_interval_days,
        day_boundary_minutes: deckSettings.day_boundary_minutes,
        now_ms: now.getTime(),
        day_start_ms: start.getTime(),
      });
      timeOverrideDialogOpen = false;
      notice = useTimeOverride
        ? `Set this deck’s daily time to ${timeOverrideMinutes} minutes.`
        : "Cleared this deck’s daily-time override.";
    } catch (reason) {
      error = message(reason);
    } finally {
      busyDeckAction = false;
    }
  }

  async function openDeleteDeck(): Promise<void> {
    if (busyDeckAction) return;
    busyDeckAction = true;
    error = "";
    try {
      const summaries = await api.listDeckSummaries(Date.now());
      deleteCardCount =
        summaries.find((deck) => deck.id === selectedDeckId)?.total_cards ?? 0;
      destinationDeckId =
        overview?.decks.find((deck) => deck.id !== selectedDeckId)?.id ?? "";
      deleteDialogOpen = true;
    } catch (reason) {
      error = message(reason);
    } finally {
      busyDeckAction = false;
    }
  }

  async function deleteDeck(moveCardsToDeckId: string | null): Promise<void> {
    if (busyDeckAction) return;
    busyDeckAction = true;
    error = "";
    notice = "";
    try {
      await api.deleteDeck({
        deck_id: selectedDeckId,
        move_cards_to_deck_id: moveCardsToDeckId,
        confirmation: deckName,
        now_ms: Date.now(),
      });
      deleteDialogOpen = false;
      moveBeforeDeleteDialogOpen = false;
      onDeleted();
    } catch (reason) {
      error = message(reason);
    } finally {
      busyDeckAction = false;
    }
  }

  function cardCountLabel(count: number): string {
    return `${count} ${count === 1 ? "card" : "cards"}`;
  }

  function statusLabel(status: DeckCardStatusDto): string {
    if (status === "new") return "New";
    if (status === "due") return "Due";
    if (status === "suspended") return "Suspended";
    return "Scheduled";
  }

  function actionNotice(action: DeckCardActionDto): string {
    if (action === "move") return "Moved the card.";
    if (action === "suspend") return "Suspended the card.";
    if (action === "unsuspend") return "Unsuspended the card.";
    if (action === "trash") return "Moved the card to Trash.";
    return "Restored the card.";
  }

  function hasNextPage(): boolean {
    return Boolean(
      overview && offset + overview.cards.length < overview.total_matches,
    );
  }

  function message(reason: unknown): string {
    return reason instanceof Error ? reason.message : String(reason);
  }
</script>

<section class="screen deck-management-screen" aria-labelledby="deck-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Deck</span>
      <h1 id="deck-title" class="screen-title">{deckName}</h1>
      <p class="screen-description">
        Find, edit, move, suspend, or recover cards in this deck.
      </p>
    </div>
    <div class="flex flex-wrap gap-2">
      <Button variant="ghost" onclick={onBack}>Back to decks</Button>
      <Button
        variant="outline"
        aria-pressed={trash === "trash"}
        onclick={() => show(trash === "active" ? "trash" : "active")}
      >
        {trash === "active" ? "Show Trash" : "Show cards"}
      </Button>
      {#if selectedDeckId !== defaultDeckId}
        <Button variant="outline" disabled={busyDeckAction} onclick={openRename}
          >Rename deck</Button
        >
        <Button
          variant="outline"
          disabled={busyDeckAction}
          onclick={() => void openTimeOverride()}>Daily time</Button
        >
        <Button
          variant="destructive"
          disabled={busyDeckAction}
          onclick={() => void openDeleteDeck()}>Delete deck</Button
        >
      {/if}
      <Button data-primary-action onclick={onCreate}>Add card</Button>
    </div>
  </header>

  <form
    class="search-row"
    role="search"
    aria-label="Card search"
    onsubmit={(event) => {
      event.preventDefault();
      offset = 0;
      void loadCards();
    }}
  >
    <Label for="card-search">Search</Label>
    <Input
      id="card-search"
      aria-label="Search cards"
      type="search"
      bind:value={query}
      oninput={scheduleSearch}
      placeholder="Sentence or answer"
    />
    <Button type="submit" variant="outline">Search</Button>
  </form>

  {#if error}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>The card action was not completed</Alert.Title>
      <Alert.Description>
        <p>{error}</p>
        <Button class="mt-3" variant="outline" onclick={loadCards}
          >Try again</Button
        >
      </Alert.Description>
    </Alert.Root>
  {:else if notice}
    <Alert.Root role="status">
      <Alert.Title>{notice}</Alert.Title>
    </Alert.Root>
  {/if}

  <div class="card-list" aria-busy={loading}>
    {#if loading && !overview}
      <Card.Root class="p-5">
        <p class="text-muted-foreground">Loading cards…</p>
      </Card.Root>
    {:else}
      {#each overview?.cards ?? [] as card (card.id)}
        <Card.Root class="gap-4 p-5" data-testid={`card-${card.id}`}>
          <Card.Header class="p-0">
            <div class="card-heading">
              <Card.Title>
                <span lang={card.language_tag ?? undefined} dir={card.direction}
                  >{card.sentence}</span
                >
              </Card.Title>
              <Badge
                variant={card.status === "suspended" ? "outline" : "secondary"}
                >{statusLabel(card.status)}</Badge
              >
            </div>
            <Card.Description>
              Answer:
              <span lang={card.language_tag ?? undefined} dir={card.direction}
                >{card.answer}</span
              >
            </Card.Description>
          </Card.Header>
          <Card.Footer class="p-0">
            <div class="card-actions">
              {#if trash === "active"}
                <Button
                  size="sm"
                  variant="outline"
                  disabled={Boolean(busyCardId)}
                  onclick={() => onEdit(card.id)}>Edit</Button
                >
                <Button
                  size="sm"
                  variant="outline"
                  disabled={Boolean(busyCardId) ||
                    !overview?.decks.some((deck) => deck.id !== selectedDeckId)}
                  onclick={() => openMove(card)}>Move</Button
                >
                <Button
                  size="sm"
                  variant="outline"
                  disabled={Boolean(busyCardId)}
                  onclick={() =>
                    void runAction(
                      card,
                      card.status === "suspended" ? "unsuspend" : "suspend",
                    )}
                >
                  {card.status === "suspended" ? "Unsuspend" : "Suspend"}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={Boolean(busyCardId)}
                  onclick={() => void runAction(card, "trash")}
                  >Move to Trash</Button
                >
              {:else}
                <Button
                  size="sm"
                  variant="outline"
                  disabled={Boolean(busyCardId)}
                  onclick={() => void runAction(card, "restore")}
                  >Restore</Button
                >
              {/if}
            </div>
          </Card.Footer>
        </Card.Root>
      {/each}
    {/if}
  </div>

  {#if !loading && (overview?.cards.length ?? 0) === 0}
    <div class="empty-state">
      <span class="empty-mark" aria-hidden="true"
        >{trash === "active" ? "＋" : "↩"}</span
      >
      <h2>{trash === "active" ? "No cards found" : "Trash is empty"}</h2>
      <p>
        {trash === "active"
          ? "Try another search or add a card to this deck."
          : "Cards moved to Trash can be restored here."}
      </p>
      {#if trash === "active"}
        <Button variant="outline" onclick={onCreate}>Add card</Button>
      {/if}
    </div>
  {/if}

  {#if overview && overview.total_matches > overview.limit}
    <nav class="pagination" aria-label="Card pages">
      <Button
        variant="outline"
        disabled={loading || offset === 0}
        onclick={() => changePage(offset - pageSize)}>Previous</Button
      >
      <span>
        {offset + 1}–{Math.min(
          offset + overview.cards.length,
          overview.total_matches,
        )}
        of {overview.total_matches}
      </span>
      <Button
        variant="outline"
        disabled={loading || !hasNextPage()}
        onclick={() => changePage(offset + pageSize)}>Next</Button
      >
    </nav>
  {/if}
</section>

<Dialog.Root bind:open={renameDialogOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Rename deck</Dialog.Title>
      <Dialog.Description>
        Change this deck’s name without changing its cards or schedule.
      </Dialog.Description>
    </Dialog.Header>
    <form
      class="grid gap-4"
      onsubmit={(event) => {
        event.preventDefault();
        void renameDeck();
      }}
    >
      <div class="grid gap-2">
        <Label for="rename-deck-name">Name</Label>
        <Input
          id="rename-deck-name"
          bind:value={renameName}
          maxlength={80}
          autocomplete="off"
        />
      </div>
      <Dialog.Footer>
        <Button
          type="button"
          variant="outline"
          disabled={busyDeckAction}
          onclick={() => (renameDialogOpen = false)}>Cancel</Button
        >
        <Button
          type="submit"
          disabled={busyDeckAction ||
            !renameName.trim() ||
            renameName.trim() === deckName}>Rename deck</Button
        >
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={timeOverrideDialogOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Daily time for {deckName}</Dialog.Title>
      <Dialog.Description>
        Set a daily-time override for this deck or use the collection budget.
      </Dialog.Description>
    </Dialog.Header>
    <div class="grid gap-4">
      <div class="setting-row">
        <div>
          <strong>Override collection budget</strong>
          <p class="text-sm text-muted-foreground">
            {useTimeOverride
              ? "This deck uses its own daily time."
              : `This deck uses the ${deckSettings?.collection_daily_time_budget_minutes ?? 0}-minute collection budget.`}
          </p>
        </div>
        <Switch
          aria-label="Override collection daily time"
          bind:checked={useTimeOverride}
        />
      </div>
      {#if useTimeOverride}
        <div class="grid gap-2">
          <Label for="deck-daily-time">Minutes per day</Label>
          <Input
            id="deck-daily-time"
            type="number"
            min={1}
            max={1440}
            bind:value={timeOverrideMinutes}
          />
        </div>
      {/if}
    </div>
    <Dialog.Footer>
      <Button
        variant="outline"
        disabled={busyDeckAction}
        onclick={() => (timeOverrideDialogOpen = false)}>Cancel</Button
      >
      <Button
        disabled={busyDeckAction ||
          (useTimeOverride &&
            (timeOverrideMinutes < 1 || timeOverrideMinutes > 1440))}
        onclick={() => void saveTimeOverride()}>Save daily time</Button
      >
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<AlertDialog.Root bind:open={deleteDialogOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete “{deckName}”?</AlertDialog.Title>
      <AlertDialog.Description>
        Its {cardCountLabel(deleteCardCount)} will be moved to Trash.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel disabled={busyDeckAction}>Cancel</AlertDialog.Cancel>
      <Button
        variant="outline"
        disabled={busyDeckAction || !destinationDeckId}
        onclick={() => {
          deleteDialogOpen = false;
          moveBeforeDeleteDialogOpen = true;
        }}>Move cards instead</Button
      >
      <AlertDialog.Action
        class="bg-destructive/10 text-destructive hover:bg-destructive/20"
        disabled={busyDeckAction}
        onclick={() => void deleteDeck(null)}>Delete deck</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>

<Dialog.Root bind:open={moveBeforeDeleteDialogOpen}>
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
        {#each overview?.decks.filter((deck) => deck.id !== selectedDeckId) ?? [] as deck (deck.id)}
          <option value={deck.id}>{deck.name}</option>
        {/each}
      </select>
    </div>
    <Dialog.Footer>
      <Button
        variant="outline"
        disabled={busyDeckAction}
        onclick={() => (moveBeforeDeleteDialogOpen = false)}>Cancel</Button
      >
      <Button
        variant="destructive"
        disabled={busyDeckAction || !destinationDeckId}
        onclick={() => void deleteDeck(destinationDeckId)}
        >Move cards and delete</Button
      >
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<Dialog.Root
  open={Boolean(movingCard)}
  onOpenChange={(open) => {
    if (!open) movingCard = null;
  }}
>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Move card</Dialog.Title>
      <Dialog.Description>
        Choose the deck that should contain this card.
      </Dialog.Description>
    </Dialog.Header>
    <div class="grid gap-2">
      <Label for="destination-deck">Destination deck</Label>
      <select
        id="destination-deck"
        class="w-full"
        bind:value={destinationDeckId}
      >
        {#each overview?.decks.filter((deck) => deck.id !== selectedDeckId) ?? [] as deck (deck.id)}
          <option value={deck.id}>{deck.name}</option>
        {/each}
      </select>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (movingCard = null)}
        >Cancel</Button
      >
      <Button
        disabled={!movingCard || !destinationDeckId || Boolean(busyCardId)}
        onclick={() =>
          movingCard && void runAction(movingCard, "move", destinationDeckId)}
        >Move card</Button
      >
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<style>
  .search-row {
    display: grid;
    grid-template-columns: auto minmax(12rem, 32rem) auto;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 1.25rem;
  }

  .card-list {
    display: grid;
    gap: 0.875rem;
  }

  .card-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .card-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .setting-row p {
    margin: 0.25rem 0 0;
  }

  .pagination {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    margin-top: 1.25rem;
    color: var(--muted-foreground);
    font-size: 0.875rem;
  }

  @media (max-width: 40rem) {
    .search-row {
      grid-template-columns: 1fr;
    }

    .card-heading {
      align-items: stretch;
      flex-direction: column;
    }

    .card-actions > :global(*) {
      flex: 1 1 auto;
    }

    .setting-row {
      align-items: flex-start;
      flex-direction: column;
    }
  }
</style>
