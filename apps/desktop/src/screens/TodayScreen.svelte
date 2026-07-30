<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteDate } from "svelte/reactivity";

  import { api } from "../lib/api";
  import Button from "../lib/components/Button.svelte";
  import Feedback from "../lib/components/Feedback.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";
  import type { TodayOverviewDto } from "../lib/generated/TodayOverviewDto";
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
  };

  const selectedDeckKey = "meiki-today-deck";
  const defaultDeckId = "default-deck";
  const allDecksId = "__all_decks__";

  let { onStart, onSettings }: Props = $props();
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
    if (!overview?.queue.length) return;
    activeQueue = startStudyQueue(overview);
    onStart();
  }

  function localDayBounds(
    now: Date,
    boundaryMinutes: number,
  ): { start: SvelteDate; end: SvelteDate } {
    const start = new SvelteDate(now);
    start.setHours(0, boundaryMinutes, 0, 0);
    if (now.getTime() < start.getTime()) {
      start.setDate(start.getDate() - 1);
    }
    const end = new SvelteDate(start);
    end.setDate(end.getDate() + 1);
    return { start, end };
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
      <Button variant="quiet" onclick={onSettings}>Settings</Button>
    </div>
  </header>

  {#if error}
    <Feedback tone="error" title="Today’s queue could not be planned">
      <p>{error}</p>
      <Button variant="secondary" onclick={loadOverview}>Try again</Button>
    </Feedback>
  {/if}

  <div class="today-grid" aria-busy={loading}>
    <SurfaceCard>
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
        <Button
          variant="primary"
          data-primary-action
          disabled={loading ||
            (!activeQueue && (overview?.queue.length ?? 0) === 0)}
          onclick={beginStudy}
        >
          {activeQueue && remainingStudyCards(activeQueue) > 0
            ? "Resume study"
            : "Start study"}
        </Button>
      </div>
    </SurfaceCard>

    <div class="stack">
      <SurfaceCard padding="compact" tone="quiet">
        <dl>
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
      </SurfaceCard>

      {#if overview?.backlog_exceeds_budget}
        <Feedback tone="warning" title="Due work exceeds today’s budget">
          <p>
            Every due review remains available. New intake is paused before
            automatic retention changes.
          </p>
          <p class="policy-explanation">{overview.policy_explanation}</p>
        </Feedback>
      {:else if overview?.overdue_reviews}
        <Feedback
          tone="warning"
          title={`${overview.overdue_reviews} overdue ${
            overview.overdue_reviews === 1 ? "review" : "reviews"
          }`}
        >
          <p>Overdue cards remain first in the queue.</p>
        </Feedback>
      {:else if overview?.deferred_new_cards}
        <Feedback tone="info" title="New-card intake capped">
          <p>
            {overview.deferred_new_cards} new
            {overview.deferred_new_cards === 1 ? "card is" : "cards are"}
            deferred by today’s limit or time budget. Due reviews were not deferred.
          </p>
        </Feedback>
      {:else if overview}
        <Feedback tone="info" title="Estimated from local history">
          <p>
            {overview.estimate_uses_history
              ? `${overview.response_time_samples} local response-time samples inform this estimate.`
              : "A conservative default is used until local response-time history is available."}
          </p>
        </Feedback>
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
    gap: var(--space-2);
    align-items: center;
  }

  .today-actions select {
    min-height: var(--control-height);
    padding-inline: var(--space-3);
    border: var(--border-width) solid var(--color-border-strong);
    border-radius: var(--radius-control);
    color: var(--color-text);
    background: var(--color-surface);
    font: inherit;
  }

  .today-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(16rem, 0.8fr);
    gap: var(--space-5);
  }

  .queue {
    display: grid;
    justify-items: start;
    min-height: 18rem;
    place-content: center start;
  }

  .queue strong {
    font-family: var(--font-display);
    font-size: clamp(1.7rem, 4vw, 2.5rem);
  }

  .queue p {
    max-width: 34rem;
    margin: var(--space-3) 0 var(--space-6);
    color: var(--color-text-muted);
    line-height: 1.6;
  }

  dl {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    margin: 0;
  }

  dl div {
    padding: var(--space-3);
    text-align: center;
  }

  dl div + div {
    border-left: var(--border-width) solid var(--color-border);
  }

  dt {
    color: var(--color-text-muted);
    font-size: var(--text-xs);
  }

  dd {
    margin: var(--space-2) 0 0;
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
