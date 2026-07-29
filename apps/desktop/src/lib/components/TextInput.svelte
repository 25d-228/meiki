<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";

  type Props = Omit<HTMLInputAttributes, "value"> & {
    value?: string;
    element?: HTMLInputElement;
  };

  let {
    value = $bindable(""),
    element = $bindable(),
    class: className = "",
    ...rest
  }: Props = $props();
</script>

<input
  {...rest}
  bind:this={element}
  bind:value
  class="text-input {className}"
/>

<style>
  .text-input {
    width: 100%;
    min-height: var(--control-height-large);
    padding: 0 var(--space-4);
    border: var(--border-width) solid var(--color-border-strong);
    border-radius: var(--radius-control);
    color: var(--color-text);
    background: var(--color-input);
    font: inherit;
    font-size: var(--text-md);
    transition:
      border-color var(--motion-fast),
      box-shadow var(--motion-fast);
  }

  .text-input::placeholder {
    color: var(--color-text-subtle);
  }

  .text-input:hover:not(:disabled) {
    border-color: var(--color-border-hover);
  }

  .text-input:disabled {
    cursor: wait;
    opacity: 0.68;
  }
</style>
