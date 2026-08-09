<script lang="ts">
  import RiPauseLine from "remixicon-svelte/icons/pause-line";
  import RiPlayLine from "remixicon-svelte/icons/play-line";
  import RiRestartLine from "remixicon-svelte/icons/restart-line";

  import { Button } from "$lib/components/ui/button/index.js";
  import * as Slider from "$lib/components/ui/slider/index.js";
  import { restartAudio } from "$lib/media";

  type Props = {
    source: string;
    label: string;
    durationMs?: number | null;
  };

  const audioEndToleranceSeconds = 0.05;

  let { source, label, durationMs }: Props = $props();
  let audioElement = $state<HTMLAudioElement | null>(null);
  let playing = $state(false);
  let elapsedSeconds = $state(0);
  let metadataDurationSeconds = $state<number | null>(null);
  const totalSeconds = $derived(
    metadataDurationSeconds ??
      (durationMs && durationMs > 0 ? durationMs / 1_000 : 0),
  );
  let playbackError = $state("");

  function syncDuration(): void {
    if (!audioElement || !Number.isFinite(audioElement.duration)) return;
    metadataDurationSeconds = Math.max(0, audioElement.duration);
  }

  function syncElapsed(): void {
    if (!audioElement || !Number.isFinite(audioElement.currentTime)) return;
    elapsedSeconds = Math.max(0, audioElement.currentTime);
  }

  async function play(restart = false): Promise<void> {
    const element = audioElement;
    if (!element) return;
    playbackError = "";
    try {
      if (
        restart ||
        element.ended ||
        (Number.isFinite(element.duration) &&
          element.duration - element.currentTime <= audioEndToleranceSeconds)
      ) {
        elapsedSeconds = 0;
        await restartAudio(element);
      } else {
        await element.play();
      }
    } catch {
      playing = false;
      playbackError = "Audio could not play. Try again.";
    }
  }

  function togglePlayback(): void {
    if (!audioElement) return;
    if (playing) {
      audioElement.pause();
      return;
    }
    void play();
  }

  function replay(): void {
    void play(true);
  }

  function reportLoadFailure(): void {
    playing = false;
    playbackError = "Audio could not load.";
  }

  function seek(value: number): void {
    if (!audioElement || totalSeconds <= 0) return;
    const target = Math.min(totalSeconds, Math.max(0, value));
    audioElement.currentTime = target;
    elapsedSeconds = target;
  }

  function formatTime(seconds: number): string {
    const wholeSeconds = Number.isFinite(seconds)
      ? Math.max(0, Math.floor(seconds))
      : 0;
    const minutes = Math.floor(wholeSeconds / 60);
    return `${minutes}:${String(wholeSeconds % 60).padStart(2, "0")}`;
  }
</script>

<div class="audio-control">
  <div class="audio-actions">
    <Button
      variant="outline"
      size="icon-sm"
      aria-label={playing ? "Pause audio" : "Play audio"}
      title={playing ? "Pause" : "Play"}
      onclick={togglePlayback}
    >
      {#if playing}
        <RiPauseLine aria-hidden="true" />
      {:else}
        <RiPlayLine aria-hidden="true" />
      {/if}
    </Button>
    <Button
      variant="outline"
      size="icon-sm"
      aria-label="Replay audio"
      title="Replay"
      onclick={replay}
    >
      <RiRestartLine aria-hidden="true" />
    </Button>
  </div>
  <div class="audio-seek">
    <Slider.Root
      type="single"
      value={elapsedSeconds}
      min={0}
      max={Math.max(1, totalSeconds)}
      step={0.1}
      disabled={totalSeconds <= 0}
      aria-label={`Seek ${label}`}
      aria-valuetext={`${formatTime(elapsedSeconds)} of ${formatTime(totalSeconds)}`}
      onValueChange={seek}
    />
  </div>
  <span class="audio-time" aria-label="Elapsed and total time">
    {formatTime(elapsedSeconds)} / {formatTime(totalSeconds)}
  </span>
  <audio
    bind:this={audioElement}
    src={source}
    preload="metadata"
    aria-label={label}
    onloadedmetadata={syncDuration}
    ondurationchange={syncDuration}
    ontimeupdate={syncElapsed}
    onplay={() => (playing = true)}
    onpause={() => (playing = false)}
    onended={() => (playing = false)}
    onerror={reportLoadFailure}
  >
    Your browser cannot play this audio.
  </audio>
  {#if playbackError}
    <p class="audio-error" role="alert">{playbackError}</p>
  {/if}
</div>

<style>
  .audio-control {
    display: grid;
    grid-template-columns: auto minmax(7rem, 1fr) auto;
    gap: 0.625rem;
    align-items: center;
    min-width: 0;
    padding: 0.625rem;
    border: 1px solid var(--border);
    border-radius: 0;
    color: var(--foreground);
    background: var(--background);
  }

  .audio-actions {
    display: flex;
    gap: 0.375rem;
  }

  .audio-actions :global(button) {
    border-radius: 0;
  }

  .audio-seek {
    min-width: 0;
  }

  .audio-time {
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  audio {
    display: none;
  }

  .audio-error {
    grid-column: 1 / -1;
    margin: 0;
    color: var(--destructive);
    font-size: var(--text-xs);
  }

  @media (max-width: 480px) {
    .audio-control {
      grid-template-columns: 1fr auto;
    }

    .audio-seek {
      grid-column: 1 / -1;
      grid-row: 2;
    }
  }
</style>
