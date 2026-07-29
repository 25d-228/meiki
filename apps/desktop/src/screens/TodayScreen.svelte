<script lang="ts">
  import Button from "../lib/components/Button.svelte";
  import Feedback from "../lib/components/Feedback.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";

  type Props = {
    onNavigate: (screen: string) => void;
  };

  let { onNavigate }: Props = $props();
</script>

<section class="screen" aria-labelledby="today-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Daily overview</span>
      <h1 id="today-title" class="screen-title">Today</h1>
      <p class="screen-description">
        A quiet view of the work available on this device.
      </p>
    </div>
  </header>

  <div class="today-grid">
    <SurfaceCard>
      <div class="queue">
        <span class="eyebrow">Review queue</span>
        <strong>Ready when you are</strong>
        <p>Your sample cloze is available for focused recall.</p>
        <Button
          variant="primary"
          data-primary-action
          onclick={() => onNavigate("study")}>Start study</Button
        >
      </div>
    </SurfaceCard>

    <div class="stack">
      <SurfaceCard padding="compact" tone="quiet">
        <dl>
          <div>
            <dt>Due</dt>
            <dd>1</dd>
          </div>
          <div>
            <dt>New</dt>
            <dd>0</dd>
          </div>
          <div>
            <dt>Estimate</dt>
            <dd>&lt; 1 min</dd>
          </div>
        </dl>
      </SurfaceCard>
      <Feedback tone="info" title="Local-first by default">
        <p>Your queue and review history remain available without a network.</p>
      </Feedback>
    </div>
  </div>
</section>

<style>
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
  }
</style>
