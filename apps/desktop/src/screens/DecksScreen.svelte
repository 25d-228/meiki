<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteDate } from "svelte/reactivity";

  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import type { DeckSummaryDto } from "../lib/generated/DeckSummaryDto";
  import { localDayBounds } from "../lib/local-day";
  import {
    clearStudyQueue,
    readStudyQueue,
    remainingStudyCards,
    startStudyQueue,
    type StudyQueueSession,
  } from "../lib/study-queue";

  type Props = {
    onStudy: (deckName: string) => void;
    onOpen: (deckId: string, deckName: string) => void;
    onDeckContextChange: (value: string) => void;
  };

  let { onStudy, onOpen, onDeckContextChange }: Props = $props();
  let decks = $state<DeckSummaryDto[]>([]);
  let activeQueue = $state<StudyQueueSession | null>(null);
  let newDeckName = $state("");
  let newDeckDialogOpen = $state(false);
  let loading = $state(true);
  let busyDeckId = $state("");
  let creating = $state(false);
  let error = $state("");
  let notice = $state("");

  onMount(() => {
    onDeckContextChange("All decks");
    const storedQueue = readStudyQueue();
    if (storedQueue && remainingStudyCards(storedQueue) > 0) {
      activeQueue = storedQueue;
    } else if (storedQueue) {
      clearStudyQueue();
    }
    void loadDecks();
  });

  async function loadDecks(): Promise<void> {
    loading = true;
    error = "";
    try {
      decks = await api.listDeckSummaries(Date.now());
    } catch (cause) {
      error = message(cause);
    } finally {
      loading = false;
    }
  }

  async function createDeck(): Promise<void> {
    if (!newDeckName.trim() || creating) return;
    creating = true;
    error = "";
    notice = "";
    try {
      const created = await api.createDeck({
        name: newDeckName,
        now_ms: Date.now(),
      });
      newDeckName = "";
      newDeckDialogOpen = false;
      await loadDecks();
      notice = `Created deck “${created.name}”.`;
    } catch (cause) {
      error = message(cause);
    } finally {
      creating = false;
    }
  }

  async function beginStudy(deck: DeckSummaryDto): Promise<void> {
    if (activeQueue && remainingStudyCards(activeQueue) > 0) {
      if (activeQueue.deckId === deck.id) onStudy(deck.name);
      return;
    }
    busyDeckId = deck.id;
    error = "";
    notice = "";
    try {
      const settings = await api.getSchedulerSettings(deck.id);
      const now = new SvelteDate();
      const { start, end } = localDayBounds(now, settings.day_boundary_minutes);
      const plan = await api.prepareStudy({
        deck_id: deck.id,
        now_ms: now.getTime(),
        day_start_ms: start.getTime(),
        day_end_ms: end.getTime(),
      });
      if (plan.availability !== "ready" || plan.overview.queue.length === 0) {
        notice = `${deck.name} has no cards ready to study.`;
        return;
      }
      activeQueue = startStudyQueue(plan.overview);
      onStudy(deck.name);
    } catch (cause) {
      error = message(cause);
    } finally {
      busyDeckId = "";
    }
  }

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }
</script>

<section class="screen decks-screen" aria-labelledby="decks-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Your collection</span>
      <h1 id="decks-title" class="screen-title">Decks</h1>
      <p class="screen-description">
        Open a deck to manage its cards, or begin a focused study session.
      </p>
    </div>
    <Button data-primary-action onclick={() => (newDeckDialogOpen = true)}
      >New deck</Button
    >
  </header>

  {#if error}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>The deck action was not completed</Alert.Title>
      <Alert.Description>
        <p>{error}</p>
        <Button class="mt-3" variant="outline" onclick={loadDecks}
          >Try again</Button
        >
      </Alert.Description>
    </Alert.Root>
  {:else if notice}
    <Alert.Root role="status">
      <Alert.Title>{notice}</Alert.Title>
    </Alert.Root>
  {/if}

  {#if activeQueue && remainingStudyCards(activeQueue) > 0}
    <p class="saved-session-note">
      A saved session is active. Resume its deck here, or return to Today for an
      all-decks session.
    </p>
  {/if}

  <div class="deck-grid" aria-busy={loading}>
    {#if loading && decks.length === 0}
      <Card.Root class="p-6">
        <p class="text-muted-foreground">Loading decks…</p>
      </Card.Root>
    {:else}
      {#each decks as deck (deck.id)}
        <Card.Root class="gap-5 p-5" data-testid={`deck-${deck.id}`}>
          <Card.Header class="p-0">
            <Card.Title>{deck.name}</Card.Title>
            <Card.Description>
              {deck.total_cards}
              {deck.total_cards === 1 ? "card" : "cards"}
            </Card.Description>
          </Card.Header>
          <dl class="deck-counts">
            <div>
              <dt>Total</dt>
              <dd>{deck.total_cards}</dd>
            </div>
            <div>
              <dt>Due</dt>
              <dd>{deck.due_cards}</dd>
            </div>
            <div>
              <dt>New</dt>
              <dd>{deck.new_cards}</dd>
            </div>
          </dl>
          <Card.Footer class="justify-end p-0">
            <Button variant="outline" onclick={() => onOpen(deck.id, deck.name)}
              >Open</Button
            >
            <Button
              disabled={busyDeckId !== "" ||
                deck.total_cards === 0 ||
                Boolean(activeQueue && activeQueue.deckId !== deck.id)}
              onclick={() => void beginStudy(deck)}
            >
              {activeQueue && activeQueue.deckId === deck.id
                ? "Resume"
                : busyDeckId === deck.id
                  ? "Planning…"
                  : "Study"}
            </Button>
          </Card.Footer>
        </Card.Root>
      {/each}
    {/if}
  </div>

  {#if !loading && decks.length === 0}
    <div class="empty-state">
      <span class="empty-mark" aria-hidden="true">＋</span>
      <h2>Create your first deck</h2>
      <p>Name a deck now, then add cards when you are ready.</p>
      <Button variant="outline" onclick={() => (newDeckDialogOpen = true)}
        >New deck</Button
      >
    </div>
  {/if}
</section>

<Dialog.Root bind:open={newDeckDialogOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>New deck</Dialog.Title>
      <Dialog.Description>
        Choose a name. You can configure scheduling later in Settings.
      </Dialog.Description>
    </Dialog.Header>
    <form
      class="grid gap-4"
      onsubmit={(event) => {
        event.preventDefault();
        void createDeck();
      }}
    >
      <div class="grid gap-2">
        <Label for="new-deck-name">Name</Label>
        <Input
          id="new-deck-name"
          bind:value={newDeckName}
          maxlength={80}
          autocomplete="off"
        />
      </div>
      <Dialog.Footer>
        <Button
          type="button"
          variant="outline"
          disabled={creating}
          onclick={() => (newDeckDialogOpen = false)}>Cancel</Button
        >
        <Button type="submit" disabled={creating || !newDeckName.trim()}>
          {creating ? "Creating…" : "Create deck"}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<style>
  .deck-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 18rem), 1fr));
    gap: 1rem;
  }

  .deck-counts {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.75rem;
  }

  .deck-counts div {
    display: grid;
    gap: 0.15rem;
    border: 1px solid var(--border);
    padding: 0.75rem;
  }

  .deck-counts dt,
  .saved-session-note {
    color: var(--muted-foreground);
    font-size: 0.8rem;
  }

  .deck-counts dd {
    font-size: 1.25rem;
    font-weight: 700;
  }

  .saved-session-note {
    margin: 0;
  }
</style>
