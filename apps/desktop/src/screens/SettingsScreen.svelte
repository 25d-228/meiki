<script lang="ts">
  import { onMount } from "svelte";

  import { api } from "../lib/api";
  import Button from "../lib/components/Button.svelte";
  import Feedback from "../lib/components/Feedback.svelte";
  import Field from "../lib/components/Field.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";
  import type { SchedulerSettingsDto } from "../lib/generated/SchedulerSettingsDto";
  import type { StudyIntensityDto } from "../lib/generated/StudyIntensityDto";
  import type { ThemeMode } from "../lib/ui";

  type Props = {
    theme: ThemeMode;
    onThemeChange: (theme: ThemeMode) => void;
  };

  const deckId = "default-deck";
  const retentionPreset: Record<StudyIntensityDto, number> = {
    light: 8500,
    balanced: 9000,
    intensive: 9300,
  };

  let { theme, onThemeChange }: Props = $props();
  let settings = $state<SchedulerSettingsDto | null>(null);
  let intensity = $state<StudyIntensityDto>("balanced");
  let targetRetention = $state(9000);
  let newCardsPerDay = $state(20);
  let dailyBudget = $state("");
  let maximumIntervalDays = $state(36500);
  let dayBoundaryMinutes = $state(240);
  let busy = $state(false);
  let notice = $state("");
  let error = $state("");

  onMount(() => {
    void loadSettings();
  });

  function applySettings(next: SchedulerSettingsDto): void {
    settings = next;
    intensity = next.intensity;
    targetRetention = next.target_retention_basis_points;
    newCardsPerDay = next.new_cards_per_day;
    dailyBudget = next.daily_time_budget_minutes?.toString() ?? "";
    maximumIntervalDays = next.maximum_interval_days;
    dayBoundaryMinutes = next.day_boundary_minutes;
  }

  async function loadSettings(): Promise<void> {
    busy = true;
    error = "";
    try {
      applySettings(await api.getSchedulerSettings(deckId));
    } catch (cause) {
      error = message(cause);
    } finally {
      busy = false;
    }
  }

  function chooseIntensity(next: StudyIntensityDto): void {
    intensity = next;
    targetRetention = retentionPreset[next];
  }

  async function save(): Promise<void> {
    busy = true;
    notice = "";
    error = "";
    try {
      applySettings(
        await api.updateSchedulerSettings({
          deck_id: deckId,
          intensity,
          target_retention_basis_points: targetRetention,
          new_cards_per_day: newCardsPerDay,
          daily_time_budget_minutes:
            dailyBudget.trim() === "" ? null : Number(dailyBudget),
          maximum_interval_days: maximumIntervalDays,
          day_boundary_minutes: dayBoundaryMinutes,
        }),
      );
      notice = "Scheduling preferences saved.";
    } catch (cause) {
      error = message(cause);
    } finally {
      busy = false;
    }
  }

  async function optimize(): Promise<void> {
    await runAction(
      () => api.optimizeScheduler(deckId),
      "Local personalization finished.",
    );
  }

  async function rollback(): Promise<void> {
    await runAction(
      () => api.rollbackScheduler(deckId),
      "Previous scheduler parameters restored.",
    );
  }

  async function rebuild(): Promise<void> {
    if (
      !window.confirm(
        "Create a backup and rebuild every schedule in this deck from review history?",
      )
    )
      return;
    busy = true;
    notice = "";
    error = "";
    try {
      const result = await api.rebuildScheduler(deckId);
      notice = `Rebuilt ${result.rebuilt_cards} cards. Backup: ${result.backup_path}`;
    } catch (cause) {
      error = message(cause);
    } finally {
      busy = false;
    }
  }

  async function exportDiagnostics(): Promise<void> {
    busy = true;
    notice = "";
    error = "";
    try {
      const result = await api.exportSchedulerDiagnostics(deckId);
      notice = `Diagnostics exported: ${result.path}`;
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
</script>

<section class="screen settings-screen" aria-labelledby="settings-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Preferences</span>
      <h1 id="settings-title" class="screen-title">Settings</h1>
      <p class="screen-description">
        Balanced scheduling works immediately. Exact controls remain available
        when you need them.
      </p>
    </div>
    <Button
      variant="primary"
      data-primary-action
      disabled={busy || !settings}
      onclick={save}>Save preferences</Button
    >
  </header>

  {#if error}
    <Feedback
      tone="error"
      title="Scheduling settings could not be updated"
      compact
    >
      <p>{error}</p>
    </Feedback>
  {:else if notice}
    <Feedback tone="success" title={notice} compact />
  {:else if busy && !settings}
    <Feedback title="Loading scheduling settings…" compact />
  {/if}

  <SurfaceCard>
    <div class="settings-list" aria-busy={busy}>
      <Field
        id="theme"
        label="Appearance"
        description="System follows the operating system light or dark preference."
      >
        <div class="segmented" id="theme" role="group" aria-label="Appearance">
          {#each ["system", "light", "dark"] as ThemeMode[] as mode (mode)}
            <Button
              variant={theme === mode ? "primary" : "secondary"}
              size="small"
              aria-pressed={theme === mode}
              onclick={() => onThemeChange(mode)}
            >
              {mode[0].toUpperCase() + mode.slice(1)}
            </Button>
          {/each}
        </div>
      </Field>

      <Field
        id="study-intensity"
        label="Study intensity"
        description="A preset changes target retention; you can refine it under Advanced."
      >
        <div
          class="segmented"
          id="study-intensity"
          role="group"
          aria-label="Study intensity"
        >
          {#each ["light", "balanced", "intensive"] as StudyIntensityDto[] as mode (mode)}
            <Button
              variant={intensity === mode ? "primary" : "secondary"}
              size="small"
              aria-pressed={intensity === mode}
              disabled={busy}
              onclick={() => chooseIntensity(mode)}
            >
              {mode[0].toUpperCase() + mode.slice(1)}
            </Button>
          {/each}
        </div>
      </Field>

      <div class="control-grid">
        <Field
          id="new-cards"
          label="New cards per day"
          description="Use zero to pause new cards."
        >
          <input
            id="new-cards"
            type="number"
            min="0"
            max="10000"
            bind:value={newCardsPerDay}
            disabled={busy}
          />
        </Field>
        <Field
          id="daily-budget"
          label="Daily time budget"
          description="Minutes; leave empty for no limit."
          optional
        >
          <input
            id="daily-budget"
            type="number"
            min="1"
            max="1440"
            placeholder="No limit"
            bind:value={dailyBudget}
            disabled={busy}
          />
        </Field>
      </div>

      <div class="setting-row">
        <div>
          <strong>Collection</strong>
          <p>Learning content and personalization remain local.</p>
        </div>
        <span class="value">On this device</span>
      </div>

      <details>
        <summary>Advanced</summary>
        <div class="advanced">
          <div class="control-grid">
            <Field
              id="target-retention"
              label="Target retention (basis points)"
              description="9000 means a 90% target."
            >
              <input
                id="target-retention"
                type="number"
                min="7000"
                max="9900"
                step="10"
                bind:value={targetRetention}
                disabled={busy}
              />
            </Field>
            <Field id="maximum-interval" label="Maximum interval (days)">
              <input
                id="maximum-interval"
                type="number"
                min="1"
                max="36500"
                bind:value={maximumIntervalDays}
                disabled={busy}
              />
            </Field>
            <Field
              id="day-boundary"
              label="Day boundary (minutes after midnight)"
              description="240 means 04:00 local time."
            >
              <input
                id="day-boundary"
                type="number"
                min="0"
                max="1439"
                bind:value={dayBoundaryMinutes}
                disabled={busy}
              />
            </Field>
          </div>

          {#if settings}
            <dl class="scheduler-status">
              <div>
                <dt>Engine</dt>
                <dd>{settings.engine_version}</dd>
              </div>
              <div>
                <dt>Parameters</dt>
                <dd>{settings.active_parameter_set_id}</dd>
              </div>
              <div>
                <dt>Optimizer</dt>
                <dd>{settings.optimizer_status.replaceAll("_", " ")}</dd>
              </div>
            </dl>
            {#if settings.optimizer_diagnostics}
              <pre
                aria-label="Scheduler diagnostics">{settings.optimizer_diagnostics}</pre>
            {/if}
          {/if}

          <div class="scheduler-actions">
            <Button size="small" disabled={busy} onclick={optimize}
              >Personalize now</Button
            >
            <Button
              size="small"
              disabled={busy || !settings?.previous_parameter_set_id}
              onclick={rollback}>Roll back parameters</Button
            >
            <Button size="small" disabled={busy} onclick={exportDiagnostics}
              >Export diagnostics</Button
            >
            <Button
              variant="danger"
              size="small"
              disabled={busy}
              onclick={rebuild}>Back up and rebuild schedules</Button
            >
          </div>
          <p class="advanced-note">
            Parameter changes affect future reviews only. A full rebuild is a
            separate explicit action and always creates a backup first.
          </p>
        </div>
      </details>
    </div>
  </SurfaceCard>
</section>

<style>
  .settings-screen {
    width: min(100%, 54rem);
  }

  .settings-list {
    display: grid;
  }

  .settings-list > :global(*) {
    padding-block: var(--space-5);
  }

  .settings-list > :global(* + *) {
    border-top: var(--border-width) solid var(--color-border);
  }

  .segmented,
  .scheduler-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .control-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
    gap: var(--space-5);
  }

  input {
    width: 100%;
    min-height: var(--control-height);
    padding-inline: var(--space-3);
    border: var(--border-width) solid var(--color-border-strong);
    border-radius: var(--radius-control);
    color: var(--color-text);
    background: var(--color-surface);
    font: inherit;
  }

  .setting-row {
    display: flex;
    gap: var(--space-6);
    align-items: center;
    justify-content: space-between;
  }

  strong,
  summary {
    font-size: var(--text-sm);
  }

  p {
    margin: var(--space-1) 0 0;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    line-height: 1.5;
  }

  .value {
    flex: 0 0 auto;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    font-weight: 650;
  }

  .advanced {
    display: grid;
    gap: var(--space-5);
    padding-block: var(--space-5) var(--space-2);
  }

  .scheduler-status {
    display: grid;
    gap: var(--space-3);
    margin: 0;
  }

  .scheduler-status div {
    display: grid;
    grid-template-columns: minmax(6rem, 0.25fr) 1fr;
    gap: var(--space-4);
  }

  dt {
    color: var(--color-text-muted);
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
    padding: var(--space-3);
    overflow: auto;
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    font-size: var(--text-xs);
    white-space: pre-wrap;
  }

  .advanced-note {
    margin: 0;
  }
</style>
