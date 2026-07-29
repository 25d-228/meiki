<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    id: string;
    label: string;
    description?: string;
    error?: string;
    optional?: boolean;
    children: Snippet;
  };

  let {
    id,
    label,
    description,
    error,
    optional = false,
    children,
  }: Props = $props();
</script>

<div class="field" class:invalid={Boolean(error)}>
  <div class="label-row">
    <label for={id}>{label}</label>
    {#if optional}<span>Optional</span>{/if}
  </div>
  {@render children()}
  {#if error}
    <p id="{id}-error" class="message" role="alert">{error}</p>
  {:else if description}
    <p id="{id}-description" class="message">{description}</p>
  {/if}
</div>

<style>
  .field {
    display: grid;
    gap: var(--space-2);
  }

  .label-row {
    display: flex;
    gap: var(--space-3);
    align-items: baseline;
    justify-content: space-between;
  }

  label {
    color: var(--color-text);
    font-size: var(--text-sm);
    font-weight: 700;
  }

  .label-row span,
  .message {
    color: var(--color-text-muted);
    font-size: var(--text-xs);
  }

  .message {
    margin: 0;
    line-height: 1.5;
  }

  .invalid .message {
    color: var(--color-danger);
  }
</style>
