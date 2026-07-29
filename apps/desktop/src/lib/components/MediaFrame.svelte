<script lang="ts">
  type Props = {
    kind: "audio" | "image";
    label: string;
    state?: "empty" | "loading" | "ready" | "error";
  };

  let { kind, label, state = "empty" }: Props = $props();
</script>

<div class="media-frame" data-state={state}>
  <span class="media-mark" aria-hidden="true"
    >{kind === "audio" ? "AU" : "IM"}</span
  >
  <div>
    <strong>{label}</strong>
    <span>
      {state === "loading"
        ? "Loading media…"
        : state === "error"
          ? "Media unavailable"
          : state === "ready"
            ? "Ready"
            : `No ${kind} added`}
    </span>
  </div>
</div>

<style>
  .media-frame {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    min-height: 4.5rem;
    padding: var(--space-3);
    border: var(--border-width) dashed var(--color-border-strong);
    border-radius: var(--radius-control);
    color: var(--color-text-muted);
    background: var(--color-surface-muted);
  }

  .media-frame[data-state="error"] {
    border-color: var(--color-danger-border);
    color: var(--color-danger);
    background: var(--color-danger-soft);
  }

  .media-mark {
    display: inline-grid;
    width: 2.4rem;
    height: 2.4rem;
    flex: 0 0 auto;
    border-radius: 50%;
    color: var(--color-accent);
    background: var(--color-accent-soft);
    font-size: 0.65rem;
    font-weight: 800;
    place-items: center;
  }

  strong,
  span {
    display: block;
  }

  strong {
    color: var(--color-text);
    font-size: var(--text-sm);
  }

  div > span {
    margin-top: var(--space-1);
    font-size: var(--text-xs);
  }
</style>
