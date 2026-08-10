<script lang="ts">
  import { usesManagedAudioTransport } from "$lib/media";
  import AudioControl from "./AudioControl.svelte";

  type Props = {
    kind: "audio" | "image";
    label: string;
    role?: "prompt_audio" | "answer_audio" | "reveal_image";
    availability?: "ready" | "missing" | "corrupt" | "unsupported";
    source?: string;
    contentHash?: string;
    mediaType?: string;
    altText?: string | null;
    width?: number | null;
    height?: number | null;
    durationMs?: number | null;
    state?: "empty" | "loading" | "ready" | "error";
    onAudioReady?: (play: () => Promise<void>) => void;
  };

  let {
    kind,
    label,
    role,
    availability,
    source,
    contentHash,
    mediaType,
    altText,
    width,
    height,
    durationMs,
    state = "empty",
    onAudioReady,
  }: Props = $props();

  const resolvedState = $derived(
    availability
      ? availability === "ready" &&
        (source ||
          (kind === "audio" &&
            usesManagedAudioTransport(contentHash, mediaType)))
        ? "ready"
        : "error"
      : state,
  );

  function unavailableMessage(): string {
    if (!availability) {
      return resolvedState === "loading"
        ? "Loading media…"
        : `No ${kind} added`;
    }
    if (availability === "missing") return "Media file is missing";
    if (availability === "corrupt") return "Media integrity check failed";
    return "Media format is unsupported";
  }
</script>

<div
  class="media-frame"
  data-state={resolvedState}
  data-kind={kind}
  data-media-role={role}
>
  {#if resolvedState === "ready"}
    {#if kind === "audio"}
      <div class="media-heading">
        <strong>{label}</strong>
        <span>{mediaType}</span>
      </div>
      <AudioControl
        {source}
        {contentHash}
        {mediaType}
        label={altText ?? label}
        {durationMs}
        onReady={onAudioReady}
      />
    {:else if source}
      <figure>
        <img
          src={source}
          alt={altText ?? label}
          width={width ?? undefined}
          height={height ?? undefined}
        />
        <figcaption>{label}</figcaption>
      </figure>
    {/if}
  {:else}
    <span class="media-mark" aria-hidden="true"
      >{kind === "audio" ? "AU" : "IM"}</span
    >
    <div class="media-heading">
      <strong>{label}</strong>
      <span>{unavailableMessage()}</span>
    </div>
  {/if}
</div>

<style>
  .media-frame {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    min-height: 4.5rem;
    padding: 0.75rem;
    border: 1px dashed var(--input);
    border-radius: var(--radius-lg);
    color: var(--muted-foreground);
    background: var(--muted);
  }

  .media-frame[data-state="empty"],
  .media-frame[data-state="loading"],
  .media-frame[data-state="error"] {
    flex-direction: row;
    align-items: center;
  }

  .media-frame[data-state="error"] {
    border-color: color-mix(in oklch, var(--destructive) 35%, var(--border));
    color: var(--destructive);
    background: color-mix(in oklch, var(--destructive) 12%, transparent);
  }

  .media-mark {
    display: inline-grid;
    width: 2.4rem;
    height: 2.4rem;
    flex: 0 0 auto;
    border-radius: var(--radius-lg);
    color: var(--primary);
    background: var(--accent);
    font-size: 0.65rem;
    font-weight: 800;
    place-items: center;
  }

  .media-heading strong,
  .media-heading span {
    display: block;
  }

  .media-heading strong {
    color: var(--foreground);
    font-size: var(--text-sm);
  }

  .media-heading span {
    margin-top: 0.25rem;
    font-size: var(--text-xs);
  }

  .media-frame[data-kind="audio"][data-state="ready"] {
    border-style: solid;
    border-radius: 0;
    color: var(--muted-foreground);
    background: var(--card);
  }

  figure {
    display: grid;
    gap: 0.5rem;
    margin: 0;
    justify-items: center;
  }

  img {
    width: auto;
    max-width: 100%;
    height: auto;
    max-height: min(50vh, 26rem);
    border-radius: var(--radius-lg);
    object-fit: contain;
  }

  figcaption {
    color: var(--muted-foreground);
    font-size: var(--text-xs);
  }
</style>
