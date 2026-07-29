<script lang="ts">
  export type MenuItem = {
    id: string;
    label: string;
    shortLabel: string;
  };

  type Props = {
    label: string;
    items: MenuItem[];
    active: string;
    onSelect: (id: string) => void;
  };

  let { label, items, active, onSelect }: Props = $props();
</script>

<nav aria-label={label}>
  <ul>
    {#each items as item (item.id)}
      <li>
        <button
          type="button"
          aria-current={active === item.id ? "page" : undefined}
          aria-label={item.label}
          onclick={() => onSelect(item.id)}
        >
          <span class="marker" aria-hidden="true">{item.shortLabel}</span>
          <span class="label">{item.label}</span>
        </button>
      </li>
    {/each}
  </ul>
</nav>

<style>
  nav {
    position: sticky;
    top: calc(var(--header-height) + var(--space-5));
    align-self: start;
  }

  ul {
    display: grid;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  button {
    display: grid;
    grid-template-columns: 1.75rem 1fr;
    gap: var(--space-3);
    align-items: center;
    width: 100%;
    min-height: 2.75rem;
    padding: var(--space-2) var(--space-3);
    border: 0;
    border-radius: var(--radius-control);
    color: var(--color-text-muted);
    background: transparent;
    font: inherit;
    font-size: var(--text-sm);
    font-weight: 650;
    text-align: start;
    cursor: pointer;
  }

  button:hover {
    color: var(--color-text);
    background: var(--color-surface-muted);
  }

  button[aria-current="page"] {
    color: var(--color-accent);
    background: var(--color-accent-soft);
  }

  .marker {
    display: inline-grid;
    width: 1.75rem;
    height: 1.75rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: 0.64rem;
    letter-spacing: 0.04em;
    place-items: center;
  }

  button[aria-current="page"] .marker {
    border-color: var(--color-accent-border);
  }

  @media (max-width: 760px) {
    nav {
      position: fixed;
      z-index: var(--z-navigation);
      top: auto;
      right: 0;
      bottom: 0;
      left: 0;
      padding: var(--space-2) max(var(--space-2), env(safe-area-inset-right))
        max(var(--space-2), env(safe-area-inset-bottom))
        max(var(--space-2), env(safe-area-inset-left));
      border-top: var(--border-width) solid var(--color-border);
      background: color-mix(in srgb, var(--color-surface) 94%, transparent);
      backdrop-filter: blur(16px);
    }

    ul {
      grid-template-columns: repeat(5, minmax(0, 1fr));
      gap: 0;
    }

    button {
      display: flex;
      flex-direction: column;
      gap: var(--space-1);
      min-height: 3.5rem;
      padding: var(--space-1);
      font-size: 0.65rem;
      text-align: center;
    }

    .marker {
      width: 1.5rem;
      height: 1.5rem;
      border: 0;
    }

    .label {
      overflow: hidden;
      max-width: 100%;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
  }
</style>
