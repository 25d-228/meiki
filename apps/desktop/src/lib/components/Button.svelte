<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type ButtonVariant = "primary" | "secondary" | "quiet" | "danger";
  type ButtonSize = "small" | "medium";

  type Props = HTMLButtonAttributes & {
    children: Snippet;
    variant?: ButtonVariant;
    size?: ButtonSize;
    full?: boolean;
    shortcut?: string;
  };

  let {
    children,
    variant = "secondary",
    size = "medium",
    full = false,
    shortcut,
    class: className = "",
    type = "button",
    ...rest
  }: Props = $props();
</script>

<button
  {...rest}
  {type}
  class="button {className}"
  class:full
  data-size={size}
  data-variant={variant}
>
  <span class="label">{@render children()}</span>
  {#if shortcut}
    <kbd aria-hidden="true">{shortcut}</kbd>
  {/if}
</button>

<style>
  .button {
    display: inline-flex;
    gap: var(--space-3);
    align-items: center;
    justify-content: center;
    min-height: var(--control-height);
    padding: 0 var(--space-4);
    border: var(--border-width) solid transparent;
    border-radius: var(--radius-control);
    font: inherit;
    font-size: var(--text-sm);
    font-weight: 720;
    line-height: 1;
    cursor: pointer;
    transition:
      background-color var(--motion-fast),
      border-color var(--motion-fast),
      color var(--motion-fast),
      transform var(--motion-fast);
  }

  .button:hover:not(:disabled) {
    transform: translateY(-1px);
  }

  .button:active:not(:disabled) {
    transform: translateY(0);
  }

  .button:disabled {
    cursor: wait;
    opacity: 0.62;
  }

  .button[data-size="small"] {
    min-height: var(--control-height-small);
    padding-inline: var(--space-3);
    font-size: var(--text-xs);
  }

  .button[data-variant="primary"] {
    color: var(--color-on-accent);
    background: var(--color-accent);
    box-shadow: var(--shadow-control);
  }

  .button[data-variant="primary"]:hover:not(:disabled) {
    background: var(--color-accent-strong);
  }

  .button[data-variant="secondary"] {
    border-color: var(--color-border-strong);
    color: var(--color-text);
    background: var(--color-surface);
  }

  .button[data-variant="secondary"]:hover:not(:disabled),
  .button[data-variant="quiet"]:hover:not(:disabled) {
    background: var(--color-surface-raised);
  }

  .button[data-variant="quiet"] {
    color: var(--color-text-muted);
    background: transparent;
  }

  .button[data-variant="danger"] {
    border-color: var(--color-danger-border);
    color: var(--color-danger);
    background: var(--color-danger-soft);
  }

  .full {
    width: 100%;
  }

  .label {
    min-width: 0;
  }

  kbd {
    display: inline-grid;
    min-width: 1.45rem;
    min-height: 1.45rem;
    padding-inline: var(--space-1);
    border: 1px solid currentColor;
    border-radius: var(--radius-xs);
    font: inherit;
    font-size: 0.68rem;
    opacity: 0.68;
    place-items: center;
  }
</style>
