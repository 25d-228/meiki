<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    label: string;
    children: Snippet;
  };

  let { label, children }: Props = $props();
</script>

<div class="toolbar" role="toolbar" aria-label={label}>
  {@render children()}
</div>

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
    align-items: center;
    padding: var(--space-3);
    border: var(--border-width) solid var(--color-border);
    border-radius: var(--radius-control);
    background: var(--color-surface);
  }

  :global(.toolbar-grow) {
    flex: 1 1 15rem;
  }
</style>
