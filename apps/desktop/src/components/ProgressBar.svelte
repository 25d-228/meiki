<script lang="ts">
  type Props = {
    label: string;
    current?: number | null;
    total?: number | null;
  };

  let { label, current = null, total = null }: Props = $props();
  let counted = $derived(current !== null && total !== null);
  let percentage = $derived(
    current === null || total === null
      ? 0
      : total === 0
        ? 100
        : Math.min(100, Math.max(0, (current / total) * 100)),
  );
</script>

<div
  class="progress-track"
  class:indeterminate={!counted}
  role="progressbar"
  aria-label={label}
  aria-valuemin={counted ? 0 : undefined}
  aria-valuemax={counted ? Math.max(1, total ?? 0) : undefined}
  aria-valuenow={counted ? (total === 0 ? 1 : current) : undefined}
  aria-valuetext={counted ? `${current} of ${total}` : `${label}, in progress`}
>
  <span
    class="progress-indicator"
    style:width={counted ? `${percentage}%` : undefined}
  ></span>
</div>

<style>
  .progress-track {
    position: relative;
    width: 100%;
    height: 0.5rem;
    overflow: hidden;
    border: 1px solid var(--border);
    background: var(--muted);
  }

  .progress-indicator {
    display: block;
    height: 100%;
    background: var(--foreground);
    transition: width 160ms ease-out;
  }

  .indeterminate .progress-indicator {
    width: 35%;
    animation: progress-slide 1.2s ease-in-out infinite;
  }

  @keyframes progress-slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(286%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-indicator {
      transition: none;
    }

    .indeterminate .progress-indicator {
      animation: none;
      transform: translateX(90%);
    }
  }
</style>
