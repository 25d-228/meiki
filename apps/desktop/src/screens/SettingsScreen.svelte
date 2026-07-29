<script lang="ts">
  import Button from "../lib/components/Button.svelte";
  import Feedback from "../lib/components/Feedback.svelte";
  import Field from "../lib/components/Field.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";
  import type { ThemeMode } from "../lib/ui";

  type Props = {
    theme: ThemeMode;
    onThemeChange: (theme: ThemeMode) => void;
  };

  let { theme, onThemeChange }: Props = $props();
  let saved = $state(false);
</script>

<section class="screen settings-screen" aria-labelledby="settings-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Preferences</span>
      <h1 id="settings-title" class="screen-title">Settings</h1>
      <p class="screen-description">
        Sensible defaults first. Technical controls stay available under
        Advanced.
      </p>
    </div>
    <Button variant="primary" data-primary-action onclick={() => (saved = true)}
      >Save preferences</Button
    >
  </header>

  {#if saved}
    <Feedback tone="success" title="Preferences saved" compact />
  {/if}

  <SurfaceCard>
    <div class="settings-list">
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

      <div class="setting-row">
        <div>
          <strong>Study intensity</strong>
          <p>Balanced scheduling is used without a setup wizard.</p>
        </div>
        <span class="value">Balanced</span>
      </div>

      <div class="setting-row">
        <div>
          <strong>Collection</strong>
          <p>Learning content remains local and works offline.</p>
        </div>
        <span class="value">On this device</span>
      </div>

      <details>
        <summary>Advanced</summary>
        <div class="advanced">
          <Feedback tone="warning" title="Technical controls">
            <p>
              Scheduler diagnostics and full rebuild controls will remain
              explicit, reversible actions.
            </p>
          </Feedback>
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

  .segmented {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .setting-row {
    display: flex;
    gap: var(--space-6);
    align-items: center;
    justify-content: space-between;
  }

  strong {
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
    padding-bottom: var(--space-4);
  }
</style>
