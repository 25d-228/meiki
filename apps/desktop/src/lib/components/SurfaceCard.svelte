<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    children: Snippet;
    padding?: "none" | "compact" | "comfortable";
    tone?: "default" | "quiet";
    class?: string;
  };

  let {
    children,
    padding = "comfortable",
    tone = "default",
    class: className = "",
  }: Props = $props();
</script>

<article
  class="surface-card {className}"
  data-padding={padding}
  data-tone={tone}
>
  {@render children()}
</article>

<style>
  .surface-card {
    border: var(--border-width) solid var(--color-border);
    border-radius: var(--radius-card);
    background: var(--color-surface);
    box-shadow: var(--shadow-card);
  }

  .surface-card[data-padding="compact"] {
    padding: var(--space-4);
  }

  .surface-card[data-padding="comfortable"] {
    padding: clamp(var(--space-5), 4vw, var(--space-8));
  }

  .surface-card[data-tone="quiet"] {
    background: var(--color-surface-muted);
    box-shadow: none;
  }
</style>
