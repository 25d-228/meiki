<script lang="ts">
  import type { Snippet } from "svelte";

  type FeedbackTone = "info" | "success" | "warning" | "error";

  type Props = {
    tone?: FeedbackTone;
    title: string;
    children?: Snippet;
    compact?: boolean;
  };

  let { tone = "info", title, children, compact = false }: Props = $props();

  const marks: Record<FeedbackTone, string> = {
    info: "i",
    success: "✓",
    warning: "!",
    error: "×",
  };
</script>

<aside
  class="feedback"
  class:compact
  data-tone={tone}
  role={tone === "error" ? "alert" : "status"}
>
  <span class="mark" aria-hidden="true">{marks[tone]}</span>
  <div>
    <strong>{title}</strong>
    {#if children}
      <div class="detail">{@render children()}</div>
    {/if}
  </div>
</aside>

<style>
  .feedback {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-3);
    align-items: start;
    padding: var(--space-4);
    border: var(--border-width) solid var(--feedback-border);
    border-radius: var(--radius-control);
    color: var(--feedback-ink);
    background: var(--feedback-surface);
  }

  .feedback[data-tone="info"] {
    --feedback-border: var(--color-info-border);
    --feedback-ink: var(--color-info);
    --feedback-surface: var(--color-info-soft);
  }

  .feedback[data-tone="success"] {
    --feedback-border: var(--color-success-border);
    --feedback-ink: var(--color-success);
    --feedback-surface: var(--color-success-soft);
  }

  .feedback[data-tone="warning"] {
    --feedback-border: var(--color-warning-border);
    --feedback-ink: var(--color-warning);
    --feedback-surface: var(--color-warning-soft);
  }

  .feedback[data-tone="error"] {
    --feedback-border: var(--color-danger-border);
    --feedback-ink: var(--color-danger);
    --feedback-surface: var(--color-danger-soft);
  }

  .compact {
    padding: var(--space-3) var(--space-4);
  }

  .mark {
    display: inline-grid;
    width: 1.4rem;
    height: 1.4rem;
    border: 1px solid currentColor;
    border-radius: 50%;
    font-size: var(--text-xs);
    font-weight: 800;
    place-items: center;
  }

  strong {
    display: block;
    font-size: var(--text-sm);
  }

  .detail {
    margin-top: var(--space-1);
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    line-height: 1.5;
  }
</style>
