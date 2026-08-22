<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteDate } from "svelte/reactivity";

  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import type { TodayOverviewDto } from "../lib/generated/TodayOverviewDto";
  import type { TodayStatisticsDto } from "../lib/generated/TodayStatisticsDto";
  import type { TodayStatisticsRequest } from "../lib/generated/TodayStatisticsRequest";
  import { localDayBounds } from "../lib/local-day";
  import {
    clearStudyQueue,
    clearStudySession,
    readStudyQueue,
    replaceStudyQueue,
    remainingStudyCards,
    type StudyQueueSession,
  } from "../lib/study-queue";

  type Props = {
    onStart: () => void;
    onSettings: () => void;
    onDeckContextChange: (value: string) => void;
    deletionRefresh: number;
    progressResetRefresh: number;
  };

  const selectedDeckKey = "meiki-today-deck";
  const defaultDeckId = "default-deck";
  const allDecksId = "__all_decks__";

  let {
    onStart,
    onSettings,
    onDeckContextChange,
    deletionRefresh,
    progressResetRefresh,
  }: Props = $props();
  let overview = $state<TodayOverviewDto | null>(null);
  let statistics = $state<TodayStatisticsDto | null>(null);
  let statisticsRequest = $state<TodayStatisticsRequest | null>(null);
  let statisticsLoading = $state(false);
  let statisticsError = $state(false);
  let activeQueue = $state<StudyQueueSession | null>(null);
  let selectedDeckId = $state(allDecksId);
  let loading = $state(true);
  let startingStudy = $state(false);
  let error = $state("");
  let retryStudyStart = $state(false);
  let loadedDeletionRefresh = $state<number | null>(null);
  let loadedProgressResetRefresh = $state<number | null>(null);
  let activityMaximum = $derived(
    Math.max(
      1,
      ...(statistics?.review_activity.map((day) => day.reviews) ?? []),
    ),
  );
  let recentMaximum = $derived(
    Math.max(
      1,
      ...(statistics?.recent_reviews.flatMap((day) => [
        day.correct_reviews,
        day.error_reviews,
      ]) ?? []),
    ),
  );
  let activityReviewTotal = $derived(
    statistics?.review_activity.reduce(
      (total, day) => total + day.reviews,
      0,
    ) ?? 0,
  );
  let activityDayTotal = $derived(
    statistics?.review_activity.filter((day) => day.reviews > 0).length ?? 0,
  );
  let recentCorrectTotal = $derived(
    statistics?.recent_reviews.reduce(
      (total, day) => total + day.correct_reviews,
      0,
    ) ?? 0,
  );
  let recentErrorTotal = $derived(
    statistics?.recent_reviews.reduce(
      (total, day) => total + day.error_reviews,
      0,
    ) ?? 0,
  );

  onMount(() => {
    const storedQueue = readStudyQueue();
    if (storedQueue && remainingStudyCards(storedQueue) > 0) {
      activeQueue = storedQueue;
    } else {
      if (storedQueue) clearStudyQueue();
    }
    selectedDeckId = localStorage.getItem(selectedDeckKey) ?? allDecksId;
    void loadOverview();
  });

  $effect(() => {
    if (loadedDeletionRefresh === null) {
      loadedDeletionRefresh = deletionRefresh;
      return;
    }
    if (deletionRefresh === loadedDeletionRefresh) return;
    loadedDeletionRefresh = deletionRefresh;
    void loadOverview();
  });

  $effect(() => {
    if (loadedProgressResetRefresh === null) {
      loadedProgressResetRefresh = progressResetRefresh;
      return;
    }
    if (progressResetRefresh === loadedProgressResetRefresh) return;
    loadedProgressResetRefresh = progressResetRefresh;
    void loadOverview();
  });

  async function loadOverview(): Promise<void> {
    loading = true;
    error = "";
    retryStudyStart = false;
    statistics = null;
    statisticsRequest = null;
    statisticsLoading = false;
    statisticsError = false;
    try {
      const decks = await api.listDeckSummaries(Date.now());
      if (
        activeQueue &&
        activeQueue.deckId !== allDecksId &&
        !decks.some((deck) => deck.id === activeQueue?.deckId)
      ) {
        clearStudyQueue();
        clearStudySession();
        activeQueue = null;
      }
      if (
        selectedDeckId !== allDecksId &&
        !decks.some((deck) => deck.id === selectedDeckId)
      ) {
        selectedDeckId = allDecksId;
        localStorage.setItem(selectedDeckKey, allDecksId);
      }
      const settings = await api.getSchedulerSettings(
        selectedDeckId === allDecksId ? defaultDeckId : selectedDeckId,
      );
      const now = new SvelteDate();
      const { start, end } = localDayBounds(now, settings.day_boundary_minutes);
      const todayRequest = {
        deck_id: selectedDeckId,
        now_ms: now.getTime(),
        day_start_ms: start.getTime(),
        day_end_ms: end.getTime(),
      };
      overview = await api.getTodayOverview(todayRequest);
      onDeckContextChange(overview.deck_name);
      localStorage.setItem(selectedDeckKey, overview.deck_id);
      statisticsRequest = {
        ...todayRequest,
        day_boundary_minutes: settings.day_boundary_minutes,
      };
      void loadStatistics();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading = false;
    }
  }

  async function loadStatistics(): Promise<void> {
    if (!statisticsRequest) return;
    const request = statisticsRequest;
    statisticsLoading = true;
    statisticsError = false;
    try {
      const loaded = await api.getTodayStatistics(request);
      if (statisticsRequest === request) statistics = loaded;
    } catch {
      if (statisticsRequest === request) statisticsError = true;
    } finally {
      if (statisticsRequest === request) statisticsLoading = false;
    }
  }

  async function changeDeck(deckId: string): Promise<void> {
    selectedDeckId = deckId;
    await loadOverview();
  }

  async function beginStudy(): Promise<void> {
    if (
      activeQueue &&
      activeQueue.deckId === selectedDeckId &&
      remainingStudyCards(activeQueue) > 0
    ) {
      onStart();
      return;
    }
    if (!overview || startingStudy) return;
    startingStudy = true;
    error = "";
    retryStudyStart = false;
    try {
      activeQueue = await replaceStudyQueue(
        activeQueue,
        overview,
        api.gradeReview,
      );
      onStart();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
      retryStudyStart = true;
    } finally {
      startingStudy = false;
    }
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

  function percentage(value: number | null): string {
    return value === null ? "No reviews" : `${(value / 100).toFixed(0)}%`;
  }

  function activityLevel(reviews: number): number {
    if (reviews === 0) return 0;
    return Math.max(1, Math.ceil((reviews / activityMaximum) * 4));
  }

  function barHeight(reviews: number): number {
    return (reviews / recentMaximum) * 88;
  }

  function displayDate(value: string): string {
    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
    }).format(new SvelteDate(`${value}T00:00:00`));
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
            disabled={loading || startingStudy}
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
      <Alert.Title>
        {retryStudyStart
          ? "The saved review could not be completed"
          : "Today’s queue could not be planned"}
      </Alert.Title>
      <Alert.Description>
        <p>{error}</p>
        <Button
          class="mt-3"
          variant="outline"
          onclick={() =>
            retryStudyStart ? void beginStudy() : void loadOverview()}
          >Try again</Button
        >
      </Alert.Description>
    </Alert.Root>
  {/if}

  <div class="today-overview" aria-busy={loading}>
    <Card.Root class="p-6">
      <div class="queue">
        <span class="eyebrow">{overview?.deck_name ?? "Review queue"}</span>
        {#if loading && !overview}
          <strong>Planning today…</strong>
          <p>Reading due timestamps and local review history.</p>
        {:else if activeQueue && activeQueue.deckId === selectedDeckId && remainingStudyCards(activeQueue) > 0}
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
            · {overview.daily_time_budget_minutes ?? "—"} min/day · automatic
            {(overview.target_retention_basis_points / 100).toFixed(0)}%
            retention target
          </p>
        {/if}
        <Button
          variant="default"
          data-primary-action
          disabled={loading || startingStudy || !overview}
          onclick={() => void beginStudy()}
        >
          {activeQueue &&
          activeQueue.deckId === selectedDeckId &&
          remainingStudyCards(activeQueue) > 0
            ? "Resume study"
            : startingStudy
              ? "Starting…"
              : "Start study"}
        </Button>
      </div>
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
    {/if}
  </div>

  <section class="statistics" aria-labelledby="statistics-title">
    <div class="section-heading">
      <div>
        <span class="eyebrow">Local review history</span>
        <h2 id="statistics-title">Review statistics</h2>
      </div>
      {#if statisticsLoading}
        <span class="statistics-loading" role="status">Loading statistics…</span
        >
      {/if}
    </div>

    {#if statisticsError}
      <Alert.Root variant="destructive" role="alert">
        <Alert.Title>Review statistics are unavailable</Alert.Title>
        <Alert.Description>
          <p>Your study queue is still ready to use.</p>
          <Button
            class="mt-3"
            variant="outline"
            onclick={() => void loadStatistics()}>Try statistics again</Button
          >
        </Alert.Description>
      </Alert.Root>
    {:else if statistics}
      <dl class="statistics-summary">
        <Card.Root class="summary-card">
          <dt>Cards learned today</dt>
          <dd>{statistics.cards_learned_today}</dd>
        </Card.Root>
        <Card.Root class="summary-card">
          <dt>Reviews today</dt>
          <dd>{statistics.reviews_today}</dd>
        </Card.Root>
        <Card.Root class="summary-card">
          <dt>Correct rate</dt>
          <dd>{percentage(statistics.correct_rate_basis_points)}</dd>
        </Card.Root>
        <Card.Root class="summary-card">
          <dt>Error rate</dt>
          <dd>{percentage(statistics.error_rate_basis_points)}</dd>
        </Card.Root>
        <Card.Root class="summary-card">
          <dt>Longest streak</dt>
          <dd>
            {statistics.longest_streak}
            {statistics.longest_streak === 1 ? "day" : "days"}
          </dd>
        </Card.Root>
      </dl>

      {#if statistics.review_activity.every((day) => day.reviews === 0)}
        <p class="empty-statistics">
          No active reviews yet. Review activity will appear here after you
          study a card.
        </p>
      {/if}

      <div class="chart-grid">
        <Card.Root class="chart-card">
          <figure aria-labelledby="review-activity-title">
            <figcaption>
              <h3 id="review-activity-title">Review activity</h3>
              <p>
                {activityReviewTotal} reviews across the past 52 weeks on
                {activityDayTotal} active {activityDayTotal === 1
                  ? "day"
                  : "days"}.
              </p>
            </figcaption>
            <svg
              class="activity-chart"
              viewBox="0 0 520 70"
              role="img"
              aria-label={`Daily review activity from ${displayDate(statistics.review_activity[0].date)} through ${displayDate(statistics.review_activity.at(-1)?.date ?? statistics.review_activity[0].date)}`}
              focusable="false"
            >
              {#each statistics.review_activity as day, index (day.date)}
                <rect
                  class={`activity-level-${activityLevel(day.reviews)}`}
                  x={Math.floor(index / 7) * 10 + 1}
                  y={(index % 7) * 10 + 1}
                  width="8"
                  height="8"
                  aria-hidden="true"
                >
                  <title>{day.date}: {day.reviews} reviews</title>
                </rect>
              {/each}
            </svg>
            <div
              class="activity-key"
              role="group"
              aria-label="Activity intensity legend"
            >
              <span>Less</span>
              {#each [0, 1, 2, 3, 4] as level (level)}
                <span
                  class={`activity-swatch activity-level-${level}`}
                  aria-hidden="true"
                ></span>
              {/each}
              <span>More</span>
            </div>
          </figure>
        </Card.Root>

        <Card.Root class="chart-card">
          <figure aria-labelledby="accuracy-activity-title">
            <figcaption>
              <h3 id="accuracy-activity-title">Correct and error reviews</h3>
              <p>
                {recentCorrectTotal} correct and {recentErrorTotal} error reviews
                over 30 study days.
              </p>
            </figcaption>
            <svg
              class="accuracy-chart"
              viewBox="0 0 600 110"
              role="img"
              aria-label={`Daily correct and error reviews from ${displayDate(statistics.recent_reviews[0].date)} through ${displayDate(statistics.recent_reviews.at(-1)?.date ?? statistics.recent_reviews[0].date)}`}
              focusable="false"
            >
              <defs>
                <pattern
                  id="today-error-hatch"
                  width="5"
                  height="5"
                  patternUnits="userSpaceOnUse"
                >
                  <rect width="5" height="5" class="error-pattern-fill"></rect>
                  <path d="M-1,1 l2,-2 M0,5 l5,-5 M4,6 l2,-2"></path>
                </pattern>
              </defs>
              <line x1="0" y1="99" x2="600" y2="99" class="chart-axis"></line>
              {#each statistics.recent_reviews as day, index (day.date)}
                {@const correctHeight = barHeight(day.correct_reviews)}
                {@const errorHeight = barHeight(day.error_reviews)}
                <rect
                  class="correct-bar"
                  x={index * 20 + 2}
                  y={99 - correctHeight}
                  width="7"
                  height={correctHeight}
                  aria-hidden="true"
                >
                  <title>{day.date}: {day.correct_reviews} correct</title>
                </rect>
                <rect
                  class="error-bar"
                  x={index * 20 + 11}
                  y={99 - errorHeight}
                  width="7"
                  height={errorHeight}
                  aria-hidden="true"
                >
                  <title>{day.date}: {day.error_reviews} errors</title>
                </rect>
              {/each}
            </svg>
            <div
              class="chart-legend"
              role="group"
              aria-label="Review result legend"
            >
              <span><i class="correct-key" aria-hidden="true"></i>Correct</span>
              <span><i class="error-key" aria-hidden="true"></i>Error</span>
            </div>
          </figure>
        </Card.Root>
      </div>
    {:else}
      <div class="statistics-placeholder" aria-hidden="true">
        <span></span><span></span><span></span><span></span><span></span>
      </div>
    {/if}
  </section>
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
    flex-wrap: wrap;
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

  .today-overview {
    display: grid;
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

  .statistics {
    display: grid;
    min-width: 0;
    gap: 1rem;
    margin-top: 2rem;
  }

  .section-heading {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .section-heading h2,
  figure h3 {
    margin: 0;
    font-family: var(--font-sans);
  }

  .section-heading h2 {
    font-size: var(--text-2xl);
  }

  .statistics-loading,
  figure p,
  .empty-statistics {
    color: var(--muted-foreground);
    font-size: var(--text-sm);
  }

  .statistics-summary {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 0.75rem;
    margin: 0;
  }

  .summary-card {
    min-width: 0;
    padding: 1rem;
  }

  .statistics-summary dt {
    min-height: 2.25rem;
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    line-height: 1.4;
  }

  .statistics-summary dd {
    margin: 0.5rem 0 0;
    font-size: clamp(1.15rem, 2.5vw, 1.5rem);
    font-weight: 750;
    overflow-wrap: anywhere;
  }

  .empty-statistics {
    margin: 0;
  }

  .chart-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1rem;
  }

  .chart-card {
    min-width: 0;
    padding: 1.25rem;
  }

  figure,
  figcaption {
    display: grid;
    min-width: 0;
    gap: 0.4rem;
    margin: 0;
  }

  figure p {
    min-height: 2.5rem;
    margin: 0;
    line-height: 1.5;
  }

  .activity-chart,
  .accuracy-chart {
    display: block;
    width: 100%;
    min-width: 0;
    margin-top: 0.5rem;
    overflow: visible;
  }

  .activity-chart {
    aspect-ratio: 52 / 7;
  }

  .accuracy-chart {
    aspect-ratio: 60 / 11;
  }

  .activity-level-0 {
    fill: var(--muted);
  }

  .activity-level-1 {
    fill: color-mix(in oklch, var(--success) 30%, var(--muted));
  }

  .activity-level-2 {
    fill: color-mix(in oklch, var(--success) 50%, var(--muted));
  }

  .activity-level-3 {
    fill: color-mix(in oklch, var(--success) 72%, var(--muted));
  }

  .activity-level-4,
  .correct-bar,
  .correct-key {
    fill: var(--success);
    background: var(--success);
  }

  .activity-key,
  .chart-legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 0.35rem;
    color: var(--muted-foreground);
    font-size: var(--text-xs);
  }

  .activity-swatch,
  .correct-key,
  .error-key {
    display: inline-block;
    width: 0.65rem;
    height: 0.65rem;
    border: 1px solid var(--border);
  }

  .chart-legend {
    gap: 1rem;
  }

  .chart-legend span {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }

  .error-bar {
    fill: url("#today-error-hatch");
  }

  .error-pattern-fill {
    fill: color-mix(in oklch, var(--destructive) 22%, var(--card));
  }

  .accuracy-chart pattern path {
    stroke: var(--destructive);
    stroke-width: 1.5;
  }

  .error-key {
    background-color: color-mix(in oklch, var(--destructive) 22%, var(--card));
    background-image: repeating-linear-gradient(
      135deg,
      transparent 0 2px,
      var(--destructive) 2px 3px
    );
  }

  .chart-axis {
    stroke: var(--border);
    stroke-width: 1;
  }

  .statistics-placeholder {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 0.75rem;
  }

  .statistics-placeholder span {
    min-height: 5.5rem;
    border: 1px solid var(--border);
    background: var(--muted);
  }

  @media (max-width: 880px) {
    .chart-grid {
      grid-template-columns: 1fr;
    }

    .statistics-summary,
    .statistics-placeholder {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .today-actions {
      align-self: stretch;
      justify-content: space-between;
    }
  }

  @media (max-width: 420px) {
    .statistics-summary,
    .statistics-placeholder {
      grid-template-columns: 1fr;
    }

    .summary-card {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      align-items: center;
      gap: 0.75rem;
    }

    .statistics-summary dt {
      min-height: 0;
    }

    .statistics-summary dd {
      margin: 0;
      text-align: end;
    }
  }
</style>
