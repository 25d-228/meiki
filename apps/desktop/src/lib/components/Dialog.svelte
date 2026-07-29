<script lang="ts">
  import type { Snippet } from "svelte";

  import Button from "./Button.svelte";

  type Props = {
    open: boolean;
    title: string;
    description?: string;
    children: Snippet;
    actions?: Snippet;
    onClose: () => void;
  };

  let { open, title, description, children, actions, onClose }: Props =
    $props();
  let element: HTMLDialogElement;
  const titleId = `dialog-${crypto.randomUUID()}`;

  $effect(() => {
    if (!element) return;
    if (open && !element.open) element.showModal();
    if (!open && element.open) element.close();
  });
</script>

<dialog
  bind:this={element}
  aria-labelledby={titleId}
  onclose={onClose}
  onclick={(event) => {
    if (event.target === element) onClose();
  }}
>
  <div class="dialog-panel">
    <header>
      <div>
        <h2 id={titleId}>{title}</h2>
        {#if description}<p>{description}</p>{/if}
      </div>
      <Button
        variant="quiet"
        size="small"
        aria-label="Close dialog"
        onclick={onClose}>Close</Button
      >
    </header>
    <div class="dialog-content">{@render children()}</div>
    {#if actions}
      <footer>{@render actions()}</footer>
    {/if}
  </div>
</dialog>

<style>
  dialog {
    width: min(calc(100% - var(--space-6)), 42rem);
    max-height: min(80vh, 46rem);
    padding: 0;
    border: var(--border-width) solid var(--color-border);
    border-radius: var(--radius-card);
    color: var(--color-text);
    background: var(--color-surface);
    box-shadow: var(--shadow-dialog);
  }

  dialog::backdrop {
    background: var(--color-backdrop);
    backdrop-filter: blur(3px);
  }

  .dialog-panel {
    padding: var(--space-6);
  }

  header {
    display: flex;
    gap: var(--space-5);
    align-items: start;
    justify-content: space-between;
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-xl);
  }

  p {
    margin: var(--space-2) 0 0;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .dialog-content {
    padding-block: var(--space-6);
  }

  footer {
    display: flex;
    gap: var(--space-3);
    justify-content: end;
    padding-top: var(--space-4);
    border-top: var(--border-width) solid var(--color-border);
  }
</style>
