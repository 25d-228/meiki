<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteDate } from "svelte/reactivity";

  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Collapsible from "$lib/components/ui/collapsible/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import type { SchedulerSettingsDto } from "../lib/generated/SchedulerSettingsDto";
  import type { SchedulerPolicyPreviewDto } from "../lib/generated/SchedulerPolicyPreviewDto";
  import type { SchedulingModeDto } from "../lib/generated/SchedulingModeDto";
  import type { UpdateSchedulerSettingsRequest } from "../lib/generated/UpdateSchedulerSettingsRequest";
  import type { BudgetSourceDto } from "../lib/generated/BudgetSourceDto";
  import type { ThemeMode } from "../lib/ui";

  type Props = {
    theme: ThemeMode;
    onThemeChange: (theme: ThemeMode) => void;
  };

  const autoplayKey = "meiki-autoplay-prompt-audio";
  const settingsDeckId = "default-deck";

  let { theme, onThemeChange }: Props = $props();
  let settings = $state<SchedulerSettingsDto | null>(null);
  let schedulingMode = $state<SchedulingModeDto>("automatic");
  let collectionBudgetHours = $state(0);
  let collectionBudgetMinutes = $state(30);
  let targetRetention = $state(9000);
  let newCardsPerDay = $state(20);
  let maximumIntervalDays = $state(36500);
  let dayBoundaryMinutes = $state(240);
  let policyPreview = $state<SchedulerPolicyPreviewDto | null>(null);
  let previewedRequest = $state<UpdateSchedulerSettingsRequest | null>(null);
  let autoplayPromptAudio = $state(false);
  let busy = $state(false);
  let notice = $state("");
  let error = $state("");

  onMount(() => {
    autoplayPromptAudio = localStorage.getItem(autoplayKey) === "true";
    void loadSettings();
  });

  function applySettings(next: SchedulerSettingsDto): void {
    settings = next;
    schedulingMode = next.scheduling_mode;
    [collectionBudgetHours, collectionBudgetMinutes] = splitDuration(
      next.collection_daily_time_budget_minutes,
    );
    targetRetention = next.target_retention_basis_points;
    newCardsPerDay = next.new_cards_per_day;
    maximumIntervalDays = next.maximum_interval_days;
    dayBoundaryMinutes = next.day_boundary_minutes;
    policyPreview = {
      effective_daily_time_budget_minutes:
        next.effective_daily_time_budget_minutes,
      budget_source: next.budget_source,
      target_retention_basis_points: next.target_retention_basis_points,
      new_cards_per_day: next.new_cards_per_day,
      backlog_exceeds_budget: next.controller_backlog_exceeds_budget,
      explanation: next.controller_explanation,
    };
    previewedRequest = null;
  }

  async function loadSettings(): Promise<void> {
    busy = true;
    error = "";
    try {
      applySettings(await api.getSchedulerSettings(settingsDeckId));
      const request = schedulingRequest();
      policyPreview = await api.previewSchedulerPolicy(request);
      previewedRequest = request;
    } catch (cause) {
      error = message(cause);
    } finally {
      busy = false;
    }
  }

  function markPolicyChanged(): void {
    policyPreview = null;
    previewedRequest = null;
  }

  function chooseBudget(minutes: number): void {
    [collectionBudgetHours, collectionBudgetMinutes] = splitDuration(minutes);
    markPolicyChanged();
  }

  function schedulingRequest(): UpdateSchedulerSettingsRequest {
    const now = new SvelteDate();
    const dayStart = new SvelteDate(now);
    dayStart.setHours(0, dayBoundaryMinutes, 0, 0);
    if (now.getTime() < dayStart.getTime()) {
      dayStart.setDate(dayStart.getDate() - 1);
    }
    return {
      deck_id: settingsDeckId,
      scheduling_mode: schedulingMode,
      collection_daily_time_budget_minutes: collectionBudgetTotal(),
      deck_daily_time_budget_minutes: null,
      target_retention_basis_points: targetRetention,
      new_cards_per_day: newCardsPerDay,
      maximum_interval_days: maximumIntervalDays,
      day_boundary_minutes: dayBoundaryMinutes,
      now_ms: now.getTime(),
      day_start_ms: dayStart.getTime(),
    };
  }

  async function previewPolicy(): Promise<void> {
    busy = true;
    notice = "";
    error = "";
    try {
      const request = schedulingRequest();
      policyPreview = await api.previewSchedulerPolicy(request);
      previewedRequest = request;
    } catch (cause) {
      policyPreview = null;
      previewedRequest = null;
      error = message(cause);
    } finally {
      busy = false;
    }
  }

  async function save(): Promise<void> {
    busy = true;
    notice = "";
    error = "";
    try {
      applySettings(
        await api.updateSchedulerSettings(
          previewedRequest ?? schedulingRequest(),
        ),
      );
      localStorage.setItem(autoplayKey, String(autoplayPromptAudio));
      notice = "Scheduling preferences saved.";
    } catch (cause) {
      error = message(cause);
    } finally {
      busy = false;
    }
  }

  async function importParameters(): Promise<void> {
    const path = await api.pickSchedulerParametersFile();
    if (!path) return;
    await runAction(
      () =>
        api.importSchedulerParameters({
          deck_id: settingsDeckId,
          path,
        }),
      "Scheduler parameters imported for future reviews.",
    );
  }

  async function exportParameters(): Promise<void> {
    busy = true;
    notice = "";
    error = "";
    try {
      const result = await api.exportSchedulerParameters(settingsDeckId);
      notice = `Scheduler parameters exported: ${result.path}`;
    } catch (cause) {
      error = message(cause);
    } finally {
      busy = false;
    }
  }

  async function runAction(
    action: () => Promise<SchedulerSettingsDto>,
    success: string,
  ): Promise<void> {
    busy = true;
    notice = "";
    error = "";
    try {
      applySettings(await action());
      notice = success;
    } catch (cause) {
      error = message(cause);
    } finally {
      busy = false;
    }
  }

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }

  function formatDuration(minutes: number): string {
    if (minutes < 60) return `${minutes} minutes`;
    const hours = Math.floor(minutes / 60);
    const remainder = minutes % 60;
    return remainder === 0
      ? `${hours} ${hours === 1 ? "hour" : "hours"}`
      : `${hours} hr ${remainder} min`;
  }

  function splitDuration(minutes: number): [number, number] {
    return [Math.floor(minutes / 60), minutes % 60];
  }

  function collectionBudgetTotal(): number {
    return collectionBudgetHours * 60 + collectionBudgetMinutes;
  }

  function budgetSourceLabel(source: BudgetSourceDto): string {
    return source === "deck_override" ? "Deck override" : "Collection budget";
  }
</script>

<section class="screen settings-screen" aria-labelledby="settings-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Preferences</span>
      <h1 id="settings-title" class="screen-title">Settings</h1>
      <p class="screen-description">
        Set how much time you have. Meiki adapts new intake and retention while
        keeping every due review visible.
      </p>
    </div>
    <Button
      variant="default"
      data-primary-action
      disabled={busy || !settings || !policyPreview}
      onclick={save}>Save preferences</Button
    >
  </header>

  {#if error}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Scheduling settings could not be updated</Alert.Title>
      <Alert.Description>{error}</Alert.Description>
    </Alert.Root>
  {:else if notice}
    <Alert.Root role="status">
      <Alert.Title>{notice}</Alert.Title>
    </Alert.Root>
  {:else if busy && !settings}
    <Alert.Root role="status">
      <Alert.Title>Loading scheduling settings…</Alert.Title>
    </Alert.Root>
  {/if}

  <Card.Root class="p-6">
    <div class="settings-list" aria-busy={busy}>
      <div class="field">
        <Label id="theme-label">Appearance</Label>
        <div class="segmented" id="theme" role="group" aria-label="Appearance">
          {#each ["system", "light", "dark"] as ThemeMode[] as mode (mode)}
            <Button
              variant={theme === mode ? "default" : "outline"}
              size="sm"
              aria-pressed={theme === mode}
              onclick={() => onThemeChange(mode)}
            >
              {mode[0].toUpperCase() + mode.slice(1)}
            </Button>
          {/each}
        </div>
        <p class="field-description">
          System follows the operating system light or dark preference.
        </p>
      </div>

      <div class="field">
        <Label id="scheduling-mode-label">Scheduling mode</Label>
        <div
          class="segmented"
          id="scheduling-mode"
          role="group"
          aria-label="Scheduling mode"
        >
          {#each ["automatic", "expert"] as SchedulingModeDto[] as mode (mode)}
            <Button
              variant={schedulingMode === mode ? "default" : "outline"}
              size="sm"
              aria-pressed={schedulingMode === mode}
              disabled={busy}
              onclick={() => {
                schedulingMode = mode;
                markPolicyChanged();
              }}
            >
              {mode[0].toUpperCase() + mode.slice(1)}
            </Button>
          {/each}
        </div>
        <p class="field-description">
          Automatic is recommended. Expert mode exposes manual policy and
          memory-parameter controls.
        </p>
      </div>

      <div class="field">
        <Label id="collection-daily-budget-label">Daily study time</Label>
        <div class="budget-control">
          <div
            class="segmented"
            role="group"
            aria-label="Daily study time presets"
          >
            {#each [15, 30, 60, 120] as minutes (minutes)}
              <Button
                variant={collectionBudgetTotal() === minutes
                  ? "default"
                  : "outline"}
                size="sm"
                aria-pressed={collectionBudgetTotal() === minutes}
                disabled={busy}
                onclick={() => chooseBudget(minutes)}
              >
                {minutes < 60 ? `${minutes} min` : `${minutes / 60} hr`}
              </Button>
            {/each}
            <Button
              variant={[15, 30, 60, 120].includes(collectionBudgetTotal())
                ? "outline"
                : "default"}
              size="sm"
              aria-pressed={![15, 30, 60, 120].includes(
                collectionBudgetTotal(),
              )}
              disabled={busy}
              onclick={() =>
                document.getElementById("daily-budget-hours")?.focus()}
            >
              Custom
            </Button>
          </div>
          <div class="duration-inputs">
            <label>
              <span>Hours</span>
              <input
                id="daily-budget-hours"
                type="number"
                min="0"
                max="24"
                aria-label="Daily study hours"
                bind:value={collectionBudgetHours}
                oninput={markPolicyChanged}
                disabled={busy}
              />
            </label>
            <label>
              <span>Minutes</span>
              <input
                type="number"
                min="0"
                max="59"
                aria-label="Daily study minutes"
                bind:value={collectionBudgetMinutes}
                oninput={markPolicyChanged}
                disabled={busy}
              />
            </label>
          </div>
          <span class="value">{formatDuration(collectionBudgetTotal())}</span>
        </div>
        <p class="field-description">
          This collection-wide budget includes reviews and new cards.
        </p>
      </div>

      <div class="field">
        <Label for="day-boundary">Day boundary (minutes after midnight)</Label>
        <input
          id="day-boundary"
          type="number"
          min="0"
          max="1439"
          bind:value={dayBoundaryMinutes}
          oninput={markPolicyChanged}
          disabled={busy}
        />
        <p class="field-description">
          240 means that a new study day starts at 04:00 local time.
        </p>
      </div>

      {#if schedulingMode === "expert"}
        <Collapsible.Root open>
          <Collapsible.Trigger
            class="w-full py-4 text-left text-sm font-semibold"
          >
            Expert scheduling policy
          </Collapsible.Trigger>
          <Collapsible.Content>
            <div class="advanced">
              <div class="control-grid">
                <div class="field">
                  <Label for="target-retention"
                    >Target retention (basis points)</Label
                  >
                  <input
                    id="target-retention"
                    type="number"
                    min="7000"
                    max="9900"
                    step="10"
                    bind:value={targetRetention}
                    oninput={markPolicyChanged}
                    disabled={busy}
                  />
                  <p class="field-description">9000 means a 90% target.</p>
                </div>
                <div class="field">
                  <Label for="new-cards">Maximum new cards per day</Label>
                  <input
                    id="new-cards"
                    type="number"
                    min="0"
                    max="10000"
                    bind:value={newCardsPerDay}
                    oninput={markPolicyChanged}
                    disabled={busy}
                  />
                  <p class="field-description">
                    Use zero to pause unseen cards.
                  </p>
                </div>
                <div class="field">
                  <Label for="maximum-interval">Maximum interval (days)</Label>
                  <input
                    id="maximum-interval"
                    type="number"
                    min="1"
                    max="36500"
                    bind:value={maximumIntervalDays}
                    oninput={markPolicyChanged}
                    disabled={busy}
                  />
                </div>
              </div>

              <div class="scheduler-actions">
                <Button
                  size="sm"
                  disabled={busy || settings?.scheduling_mode !== "expert"}
                  onclick={importParameters}>Import parameters</Button
                >
                <Button
                  size="sm"
                  disabled={busy || settings?.scheduling_mode !== "expert"}
                  onclick={exportParameters}>Export parameters</Button
                >
              </div>
              <p class="advanced-note">
                Memory parameters describe recall. The manual policy controls
                workload. Neither change rewrites prior review events.
              </p>
            </div>
          </Collapsible.Content>
        </Collapsible.Root>
      {/if}

      <div class="policy-preview" aria-live="polite">
        <div>
          <strong>Policy preview</strong>
          <p>
            Preview the derived plan before saving. Existing due cards are never
            hidden when the budget is tight.
          </p>
        </div>
        <Button size="sm" disabled={busy} onclick={previewPolicy}
          >Preview policy</Button
        >
        {#if policyPreview}
          <dl class="scheduler-status">
            <div>
              <dt>Budget</dt>
              <dd>
                {policyPreview.effective_daily_time_budget_minutes} min ({budgetSourceLabel(
                  policyPreview.budget_source,
                )})
              </dd>
            </div>
            <div>
              <dt>Retention</dt>
              <dd>
                {(policyPreview.target_retention_basis_points / 100).toFixed(
                  1,
                )}%
              </dd>
            </div>
            <div>
              <dt>New cards</dt>
              <dd>{policyPreview.new_cards_per_day}</dd>
            </div>
          </dl>
          {#if policyPreview.backlog_exceeds_budget}
            <Alert.Root role="status" class="bg-muted/40">
              <Alert.Title>Due work exceeds this budget</Alert.Title>
              <Alert.Description>
                Meiki will still show every due review.
              </Alert.Description>
            </Alert.Root>
          {/if}
          <pre aria-label="Policy explanation">{policyPreview.explanation}</pre>
        {:else}
          <Alert.Root role="status">
            <Alert.Title>Preview required</Alert.Title>
            <Alert.Description>
              Preview these settings to enable Save preferences.
            </Alert.Description>
          </Alert.Root>
        {/if}
      </div>

      <Separator />

      <div class="setting-row">
        <div>
          <strong>Prompt audio autoplay</strong>
          <p>
            Off by default. When enabled, only the first prompt audio clip may
            start automatically.
          </p>
        </div>
        <label class="toggle" for="autoplay-prompt-audio">
          <Switch
            id="autoplay-prompt-audio"
            bind:checked={autoplayPromptAudio}
            disabled={busy}
          />
          <span>Enable</span>
        </label>
      </div>
    </div>
  </Card.Root>
</section>

<style>
  .settings-screen {
    width: min(100%, 54rem);
  }

  .settings-screen > :global(* + *) {
    margin-top: 1.25rem;
  }

  .settings-list {
    display: grid;
  }

  .settings-list > :global(*) {
    padding-block: 1.25rem;
  }

  .settings-list > :global(* + *) {
    border-top: 1px solid var(--border);
  }

  .segmented,
  .scheduler-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .control-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
    gap: 1.25rem;
  }

  .budget-control,
  .policy-preview {
    display: grid;
    gap: 0.75rem;
  }

  .budget-control {
    grid-template-columns: 1fr auto;
    align-items: center;
  }

  .duration-inputs {
    display: grid;
    grid-template-columns: repeat(2, minmax(5.5rem, 8rem));
    gap: 0.75rem;
  }

  .duration-inputs label {
    display: grid;
    gap: 0.25rem;
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  .budget-control .segmented {
    grid-column: 1 / -1;
  }

  .policy-preview {
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--muted);
  }

  .policy-preview p,
  .policy-preview dt {
    color: var(--foreground);
  }

  input:not([type="checkbox"]) {
    width: 100%;
    min-height: 2.75rem;
    padding-inline: 0.75rem;
    border: 1px solid var(--input);
    border-radius: var(--radius-lg);
    color: var(--foreground);
    background: var(--card);
    font: inherit;
  }

  .setting-row {
    display: flex;
    gap: 1.5rem;
    align-items: center;
    justify-content: space-between;
  }

  .toggle {
    display: inline-flex;
    flex: 0 0 auto;
    gap: 0.5rem;
    align-items: center;
    font-size: var(--text-sm);
    font-weight: 650;
  }

  strong {
    font-size: var(--text-sm);
  }

  p {
    margin: 0.25rem 0 0;
    color: var(--muted-foreground);
    font-size: var(--text-sm);
    line-height: 1.5;
  }

  .value {
    flex: 0 0 auto;
    color: var(--muted-foreground);
    font-size: var(--text-sm);
    font-weight: 650;
  }

  .advanced {
    display: grid;
    gap: 1.25rem;
    padding-block: 1.25rem 0.5rem;
  }

  .scheduler-status {
    display: grid;
    gap: 0.75rem;
    margin: 0;
  }

  .scheduler-status div {
    display: grid;
    grid-template-columns: minmax(6rem, 0.25fr) 1fr;
    gap: 1rem;
  }

  dt {
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  dd {
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
    font-family: ui-monospace, "SFMono-Regular", Consolas, monospace;
    font-size: var(--text-xs);
  }

  pre {
    max-width: 100%;
    margin: 0;
    padding: 0.75rem;
    overflow: auto;
    border-radius: var(--radius-lg);
    background: var(--muted);
    font-size: var(--text-xs);
    white-space: pre-wrap;
  }

  .advanced-note {
    margin: 0;
  }

  @media (max-width: 42rem) {
    .budget-control {
      grid-template-columns: 1fr;
    }
  }
</style>
