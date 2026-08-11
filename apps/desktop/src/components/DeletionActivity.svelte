<script lang="ts">
  import RiCloseLine from "remixicon-svelte/icons/close-line";

  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import type { DeletionActivity } from "../lib/deletion-activity";
  import type { DeleteDeckPhaseDto } from "../lib/generated/DeleteDeckPhaseDto";
  import ProgressBar from "./ProgressBar.svelte";

  type Props = {
    activity: DeletionActivity | null;
    cardVisible: boolean;
    dialogOpen?: boolean;
    onDismiss: () => void;
  };

  let {
    activity,
    cardVisible,
    dialogOpen = $bindable(false),
    onDismiss,
  }: Props = $props();

  $effect(() => {
    if (!cardVisible || !activity || activity.status === "running") return;
    const operationId = activity.operationId;
    const timer = window.setTimeout(() => {
      if (activity?.operationId === operationId) onDismiss();
    }, 3_000);
    return () => window.clearTimeout(timer);
  });

  function progressLabel(phase: DeleteDeckPhaseDto): string {
    if (phase === "preparing") return "Preparing";
    if (phase === "removing_cards") return "Removing cards";
    if (phase === "cleaning_audio") return "Cleaning audio";
    return "Finalizing";
  }

  function dialogTitle(current: DeletionActivity): string {
    if (current.status === "running") return current.name;
    if (current.status === "failure") {
      if (current.kind === "deck") return "Deck was not deleted";
      if (current.kind === "decks") return "Decks were not deleted";
      return "Bundle was not removed";
    }
    if (current.kind === "deck") return "Deck deleted";
    if (current.kind === "decks") return "Decks deleted";
    return "Bundle removed";
  }

  function dialogDescription(current: DeletionActivity): string {
    if (current.status === "running") {
      return "Keep Meiki open while this finishes.";
    }
    if (current.status === "failure")
      return "Your collection was left unchanged.";
    if (current.status === "warning") {
      return "The collection was updated, but a follow-up step needs attention.";
    }
    return "The collection was updated successfully.";
  }
</script>

{#if activity && cardVisible}
  <Card.Root
    size="sm"
    class="pointer-events-auto w-full gap-0 rounded-none py-0 shadow-lg"
    data-testid="deletion-activity"
    role="status"
    aria-live="polite"
  >
    <div class="flex items-start">
      <Button
        class="h-auto min-w-0 flex-1 justify-start rounded-none px-4 py-3 text-left whitespace-normal"
        variant="ghost"
        onclick={() => (dialogOpen = true)}
        aria-label="Open deletion details"
      >
        <span class="grid w-full min-w-0 gap-1 break-words">
          <strong>{activity.name}</strong>
          {#if activity.status === "running"}
            <span>{progressLabel(activity.progress.phase)}</span>
            <ProgressBar
              label={progressLabel(activity.progress.phase)}
              current={activity.progress.current}
              total={activity.progress.total}
            />
            {#if activity.progress.current !== null && activity.progress.total !== null}
              <span class="text-xs text-muted-foreground">
                {activity.progress.current.toLocaleString()} / {activity.progress.total.toLocaleString()}
              </span>
            {/if}
          {:else}
            <span class="text-xs text-muted-foreground">{activity.message}</span
            >
          {/if}
        </span>
      </Button>
      {#if activity.status !== "running"}
        <Button
          class="mt-2 mr-2 rounded-none"
          variant="ghost"
          size="icon-sm"
          aria-label="Dismiss deletion status"
          onclick={onDismiss}
        >
          <RiCloseLine />
        </Button>
      {/if}
    </div>
  </Card.Root>
{/if}

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="rounded-none sm:max-w-md">
    {#if activity}
      <Dialog.Header>
        <Dialog.Title>{dialogTitle(activity)}</Dialog.Title>
        <Dialog.Description>{dialogDescription(activity)}</Dialog.Description>
      </Dialog.Header>

      {#if activity.status === "warning"}
        <Alert.Root role="alert">
          <Alert.Title>{activity.message}</Alert.Title>
        </Alert.Root>
      {:else if activity.status === "failure"}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>{activity.message}</Alert.Title>
        </Alert.Root>
      {:else if activity.status === "success"}
        <Alert.Root role="status">
          <Alert.Title>{activity.message}</Alert.Title>
        </Alert.Root>
      {:else}
        <div class="grid gap-2" role="status" aria-live="polite">
          <strong>{progressLabel(activity.progress.phase)}</strong>
          <ProgressBar
            label={progressLabel(activity.progress.phase)}
            current={activity.progress.current}
            total={activity.progress.total}
          />
          {#if activity.progress.current !== null && activity.progress.total !== null}
            <span class="text-sm text-muted-foreground">
              {activity.progress.current.toLocaleString()} / {activity.progress.total.toLocaleString()}
            </span>
          {/if}
        </div>
      {/if}

      <Dialog.Footer>
        <Button
          type="button"
          variant="outline"
          onclick={() => (dialogOpen = false)}>Close</Button
        >
      </Dialog.Footer>
    {/if}
  </Dialog.Content>
</Dialog.Root>
