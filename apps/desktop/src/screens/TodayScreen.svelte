<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteDate } from "svelte/reactivity";

  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import type { TodayOverviewDto } from "../lib/generated/TodayOverviewDto";
  import { localDayBounds } from "../lib/local-day";
  import {
    clearStudyQueue,
    readStudyQueue,
    remainingStudyCards,
    startStudyQueue,
    type StudyQueueSession,
  } from "../lib/study-queue";

  type Props = {
    onStart: () => void;
    onSettings: () => void;
    onDeckContextChange: (value: string) => void;
  };

  const selectedDeckKey = "meiki-today-deck";
  const defaultDeckId = "default-deck";
  const allDecksId = "__all_decks__";

  let { onStart, onSettings, onDeckContextChange }: Props = $props();
  let overview = $state<TodayOverviewDto | null>(null);
  let activeQueue = $state<StudyQueueSession | null>(null);
  let selectedDeckId = $state(allDecksId);
  let loading = $state(true);
  let error = $state("");

  onMount(() => {
    const storedQueue = readStudyQueue();
    if (storedQueue && remainingStudyCards(storedQueue) > 0) {
      activeQueue = storedQueue;
      selectedDeckId = storedQueue.deckId;
    } else {
      if (storedQueue) clearStudyQueue();
      selectedDeckId = localStorage.getItem(selectedDeckKey) ?? allDecksId;
    }
    void loadOverview();
  });

  async function loadOverview(): Promise<void> {
    loading = true;
    error = "";
    try {
      const settings = await api.getSchedulerSettings(
        selectedDeckId === allDecksId ? defaultDeckId : selectedDeckId,
      );
      const now = new SvelteDate();
      const { start, end } = localDayBounds(now, settings.day_boundary_minutes);
      overview = await api.getTodayOverview({
        deck_id: selectedDeckId,
        now_ms: now.getTime(),
        day_start_ms: start.getTime(),
        day_end_ms: end.getTime(),
      });
      onDeckContextChange(overview.deck_name);
      localStorage.setItem(selectedDeckKey, overview.deck_id);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading = false;
    }
  }

  async function changeDeck(deckId: string): Promise<void> {
    selectedDeckId = deckId;
    await loadOverview();
  }

  function beginStudy(): void {
    if (activeQueue && remainingStudyCards(activeQueue) > 0) {
      onStart();
      return;
    }
    if (!overview) return;
    if (overview.queue.length) activeQueue = startStudyQueue(overview);
    onStart();
  }

  function estimate(seconds: number): string {
    if (seconds === 0) return "0 min";
    if (seconds < 60) return "< 1 min";
    return `~${Math.max(1, Math.ceil(seconds / 60))} min`;
  }

  function nextDue(value: string): string {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new SvelteDate(value));
  }
</script>

<section class="screen today-screen" aria-labelledby="today-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Daily overview</span>
      <h1 id="today-title" class="screen-title">Today</h1>
      <p class="screen-description">
        Due work stays visible. Time limits reduce new-card intake only.
      </p>
    </div>
    <div class="today-actions">
      {#if overview && overview.decks.length > 0}
        <label>
          <span class="visually-hidden">Deck</span>
          <select
            aria-label="Deck"
            value={selectedDeckId}
            disabled={loading || Boolean(activeQueue)}
            onchange={(event) => changeDeck(event.currentTarget.value)}
          >
            <option value={allDecksId}>All decks</option>
            {#each overview.decks as deck (deck.id)}
              <option value={deck.id}>{deck.name}</option>
            {/each}
          </select>
        </label>
      {/if}
      <Button variant="ghost" onclick={onSettings}>Settings</Button>
    </div>
  </header>

  {#if error}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Today’s queue could not be planned</Alert.Title>
      <Alert.Description>
        <p>{error}</p>
        <Button class="mt-3" variant="outline" onclick={loadOverview}
          >Try again</Button
        >
      </Alert.Description>
    </Alert.Root>
  {/if}

  <div class="today-grid" aria-busy={loading}>
    <Card.Root class="p-6">
      <div class="queue">
        <span class="eyebrow">{overview?.deck_name ?? "Review queue"}</span>
        {#if loading && !overview}
          <strong>Planning today…</strong>
          <p>Reading due timestamps and local review history.</p>
        {:else if activeQueue && remainingStudyCards(activeQueue) > 0}
          <strong>Resume where you stopped</strong>
          <p>
            {remainingStudyCards(activeQueue)}
            {remainingStudyCards(activeQueue) === 1
              ? "card remains"
              : "cards remain"}
            in the saved session.
          </p>
        {:else if overview?.queue.length}
          <strong>
            {overview.overdue_reviews
              ? `${overview.overdue_reviews} overdue`
              : "Ready when you are"}
          </strong>
          <p>
            {overview.due_reviews} due and {overview.new_cards} new. The session is
            estimated at {estimate(overview.estimated_seconds)}.
          </p>
        {:else}
          <strong>You’re caught up</strong>
          <p>
            {#if overview?.next_due_at}
              Next review: {nextDue(overview.next_due_at)}.
            {:else}
              No cards are currently available in this selection.
            {/if}
          </p>
        {/if}
        {#if overview}
          <p class="policy-summary">
            {overview.budget_source === "deck_override"
              ? "Deck budget"
              : "Collection budget"}
            · automatic
            {(overview.target_retention_basis_points / 100).toFixed(0)}%
            retention target
          </p>
        {/if}
        <Button
          variant="default"
          data-primary-action
          disabled={loading || !overview}
          onclick={beginStudy}
        >
          {activeQueue && remainingStudyCards(activeQueue) > 0
            ? "Resume study"
            : "Start study"}
        </Button>
      </div>
    </Card.Root>

    <div class="stack">
      <Card.Root class="bg-muted/40 p-4 shadow-none">
        <dl>
          <div>
            <dt>Daily budget</dt>
            <dd>
              {overview?.daily_time_budget_minutes == null
                ? "—"
                : `${overview.daily_time_budget_minutes} min`}
            </dd>
          </div>
          <div>
            <dt>Due</dt>
            <dd>{overview?.due_reviews ?? "—"}</dd>
          </div>
          <div>
            <dt>New</dt>
            <dd>{overview?.new_cards ?? "—"}</dd>
          </div>
          <div>
            <dt>Estimate</dt>
            <dd>{overview ? estimate(overview.estimated_seconds) : "—"}</dd>
          </div>
        </dl>
      </Card.Root>

      {#if overview?.backlog_exceeds_budget}
        <Alert.Root role="status" class="bg-muted/40">
          <Alert.Title>Due work exceeds today’s budget</Alert.Title>
          <Alert.Description>
            Every due review remains available. New intake is paused before
            automatic retention changes.
            <span class="policy-explanation mt-2 block"
              >{overview.policy_explanation}</span
            >
          </Alert.Description>
        </Alert.Root>
      {:else if overview?.overdue_reviews}
        <Alert.Root role="status" class="bg-muted/40">
          <Alert.Title>
            {overview.overdue_reviews} overdue
            {overview.overdue_reviews === 1 ? "review" : "reviews"}
          </Alert.Title>
          <Alert.Description>
            Overdue cards remain first in the queue.
          </Alert.Description>
        </Alert.Root>
      {:else if overview?.deferred_new_cards}
        <Alert.Root role="status">
          <Alert.Title>New-card intake capped</Alert.Title>
          <Alert.Description>
            {overview.deferred_new_cards} new
            {overview.deferred_new_cards === 1 ? "card is" : "cards are"}
            deferred by today’s limit or time budget. Due reviews were not deferred.
          </Alert.Description>
        </Alert.Root>
      {:else if overview}
        <Alert.Root role="status">
          <Alert.Title>Estimated from local history</Alert.Title>
          <Alert.Description>
            {overview.estimate_uses_history
              ? `${overview.response_time_samples} local response-time samples inform this estimate.`
              : "A conservative default is used until local response-time history is available."}
          </Alert.Description>
        </Alert.Root>
      {/if}
    </div>
  </div>
</section>

<style>
  .today-screen {
    width: min(100%, 68rem);
  }

  .policy-explanation {
    white-space: pre-line;
  }

  .today-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .today-actions select {
    min-height: 2.75rem;
    padding-inline: 0.75rem;
    border: 1px solid var(--input);
    border-radius: var(--radius-lg);
    color: var(--foreground);
    background: var(--card);
    font: inherit;
  }

  .today-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(16rem, 0.8fr);
    gap: 1.25rem;
  }

  .queue {
    display: grid;
    justify-items: start;
    min-height: 18rem;
    place-content: center start;
  }

  .queue strong {
    font-family: var(--font-sans);
    font-size: clamp(1.7rem, 4vw, 2.5rem);
  }

  .queue p {
    max-width: 34rem;
    margin: 0.75rem 0 1.5rem;
    color: var(--muted-foreground);
    line-height: 1.6;
  }

  .queue .policy-summary {
    margin-top: -0.75rem;
    font-size: var(--text-sm);
  }

  dl {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    margin: 0;
  }

  dl div {
    padding: 0.75rem;
    text-align: center;
  }

  dl div + div {
    border-left: 1px solid var(--border);
  }

  dt {
    color: var(--muted-foreground);
    font-size: var(--text-xs);
  }

  dd {
    margin: 0.5rem 0 0;
    font-size: var(--text-lg);
    font-weight: 750;
  }

  @media (max-width: 880px) {
    .today-grid {
      grid-template-columns: 1fr;
    }

    .today-actions {
      align-self: stretch;
      justify-content: space-between;
    }
  }
</style>
