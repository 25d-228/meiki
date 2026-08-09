<script lang="ts">
  import RiCloseLine from "remixicon-svelte/icons/close-line";

  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import type { BundleImportProgressDto } from "../lib/generated/BundleImportProgressDto";
  import type { BundleImportResultDto } from "../lib/generated/BundleImportResultDto";
  import type { BundlePreviewDto } from "../lib/generated/BundlePreviewDto";

  type BundleImportStatus =
    "choosing" | "previewing" | "ready" | "running" | "success" | "failure";

  type BundleImportActivity = {
    status: BundleImportStatus;
    preview: BundlePreviewDto | null;
    progress: BundleImportProgressDto | null;
    result: BundleImportResultDto | null;
    error: string;
  };

  type Props = {
    activity: BundleImportActivity | null;
    dialogOpen?: boolean;
    onAdd: () => void;
    onAbandon: () => void;
    onDismiss: () => void;
  };

  let {
    activity,
    dialogOpen = $bindable(false),
    onAdd,
    onAbandon,
    onDismiss,
  }: Props = $props();

  $effect(() => {
    if (!dialogOpen && activity?.status === "ready") onAbandon();
  });

  function languageName(languageTag: string): string {
    try {
      const language = new Intl.Locale(languageTag).language;
      return (
        new Intl.DisplayNames(["en"], { type: "language" }).of(language) ??
        language
      );
    } catch {
      return languageTag;
    }
  }

  function activityLanguage(current: BundleImportActivity): string {
    const languageTag =
      current.result?.language_tag ?? current.preview?.language_tag;
    return languageTag ? languageName(languageTag) : "bundle";
  }

  function progressLabel(progress: BundleImportProgressDto): string {
    if (progress.stage === "preparing_decks") return "Preparing decks";
    if (progress.stage === "adding_cards") return "Adding cards";
    return "Adding audio";
  }

  function successMessage(current: BundleImportActivity): string {
    const result = current.result;
    if (!result) return "Bundle added.";
    const language = languageName(result.language_tag);
    if (result.added_decks === 0) return `${language} is now installed.`;
    return `Added ${language} with ${result.added_decks.toLocaleString()} ${result.added_decks === 1 ? "deck" : "decks"}.`;
  }

  function cardVisible(current: BundleImportActivity): boolean {
    return ["running", "success", "failure"].includes(current.status);
  }
</script>

{#if activity && cardVisible(activity)}
  <Card.Root
    size="sm"
    class="fixed right-3 bottom-3 z-40 w-[min(16rem,calc(100vw-1.5rem))] gap-0 rounded-none py-0 shadow-lg sm:right-5 sm:bottom-5"
    data-testid="bundle-import-activity"
    role="status"
    aria-live="polite"
  >
    <div class="flex items-start">
      <Button
        class="h-auto min-w-0 flex-1 justify-start rounded-none px-4 py-3 text-left whitespace-normal"
        variant="ghost"
        onclick={() => (dialogOpen = true)}
        aria-label={`Open ${activityLanguage(activity)} import details`}
      >
        <span class="grid min-w-0 gap-1">
          {#if activity.status === "success"}
            <strong>{successMessage(activity)}</strong>
          {:else if activity.status === "failure"}
            <strong>Could not add {activityLanguage(activity)}.</strong>
            <span class="text-xs text-muted-foreground">Open for details</span>
          {:else}
            <strong>Adding {activityLanguage(activity)}</strong>
            {#if activity.progress}
              <span>{progressLabel(activity.progress)}</span>
              {#if activity.progress.stage !== "preparing_decks"}
                <span class="text-xs text-muted-foreground">
                  {activity.progress.current.toLocaleString()} / {activity.progress.total.toLocaleString()}
                </span>
              {/if}
            {/if}
          {/if}
        </span>
      </Button>
      {#if activity.status === "success" || activity.status === "failure"}
        <Button
          class="mt-2 mr-2 rounded-none"
          variant="ghost"
          size="icon-sm"
          aria-label="Dismiss bundle import status"
          onclick={onDismiss}
        >
          <RiCloseLine />
        </Button>
      {/if}
    </div>
  </Card.Root>
{/if}

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="rounded-none sm:max-w-xl">
    <Dialog.Header>
      <Dialog.Title>Import bundle</Dialog.Title>
      <Dialog.Description>
        Add missing language decks without replacing your collection.
      </Dialog.Description>
    </Dialog.Header>

    {#if activity?.status === "previewing"}
      <p role="status">Reading bundle details…</p>
    {:else if activity?.status === "failure"}
      <Alert.Root variant="destructive" role="alert">
        <Alert.Title>The bundle was not added</Alert.Title>
        <Alert.Description>{activity.error}</Alert.Description>
      </Alert.Root>
    {:else if activity?.status === "success" && dialogOpen}
      <Alert.Root role="status">
        <Alert.Title>{successMessage(activity)}</Alert.Title>
      </Alert.Root>
    {/if}

    {#if activity?.preview}
      <div class="bundle-summary">
        <div>
          <span>Language</span>
          <strong>{languageName(activity.preview.language_tag)}</strong>
        </div>
        <div>
          <span>Total cards</span>
          <strong>{activity.preview.total_cards.toLocaleString()}</strong>
        </div>
        <div>
          <span>Audio</span>
          <strong>{activity.preview.audio_objects.toLocaleString()}</strong>
        </div>
      </div>

      <ul class="bundle-decks" aria-label="Bundle decks">
        {#each activity.preview.decks as deck (deck.id)}
          <li>
            <div>
              <strong>{deck.name}</strong>
              <span
                >{deck.cards.toLocaleString()}
                {deck.cards === 1 ? "card" : "cards"}</span
              >
            </div>
            <span class:installed={deck.status === "installed"}
              >{deck.status === "installed" ? "Installed" : "Will add"}</span
            >
          </li>
        {/each}
      </ul>

      {#if !activity.preview.can_import}
        <p role="status">
          {languageName(activity.preview.language_tag)} is already installed
        </p>
      {/if}
    {/if}

    {#if activity?.status === "running" && activity.progress}
      <div class="bundle-progress" role="status" aria-live="polite">
        <strong>{progressLabel(activity.progress)}</strong>
        {#if activity.progress.stage !== "preparing_decks"}
          <progress
            max={Math.max(1, activity.progress.total)}
            value={activity.progress.current}
          ></progress>
          <span
            >{activity.progress.current.toLocaleString()} / {activity.progress.total.toLocaleString()}</span
          >
        {/if}
      </div>
    {/if}

    <Dialog.Footer>
      <Button
        type="button"
        variant="outline"
        onclick={() => (dialogOpen = false)}>Close</Button
      >
      {#if activity?.status === "ready" || activity?.status === "running"}
        <Button
          type="button"
          disabled={activity.status === "running" ||
            !activity.preview?.can_import}
          onclick={onAdd}
        >
          {activity.status === "running" ? "Adding bundle…" : "Add bundle"}
        </Button>
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<style>
  .bundle-summary {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.75rem;
  }

  .bundle-summary div,
  .bundle-decks li {
    border: 1px solid var(--border);
    padding: 0.75rem;
  }

  .bundle-summary div,
  .bundle-decks li > div,
  .bundle-progress {
    display: grid;
    gap: 0.25rem;
  }

  .bundle-summary span,
  .bundle-decks span,
  .bundle-progress span {
    color: var(--muted-foreground);
    font-size: 0.8rem;
  }

  .bundle-decks {
    display: grid;
    max-height: min(20rem, 45vh);
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }

  .bundle-decks li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .bundle-decks li + li {
    border-top: 0;
  }

  .bundle-decks li > span:not(.installed) {
    color: var(--foreground);
    font-weight: 700;
  }

  .bundle-progress progress {
    width: 100%;
  }

  @media (max-width: 30rem) {
    .bundle-summary {
      grid-template-columns: 1fr;
    }
  }
</style>
