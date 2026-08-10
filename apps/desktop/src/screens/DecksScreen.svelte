<script lang="ts">
  import { DropdownMenu } from "bits-ui";
  import RiDeleteBin6Line from "remixicon-svelte/icons/delete-bin-6-line";
  import RiCheckboxBlankLine from "remixicon-svelte/icons/checkbox-blank-line";
  import RiCheckboxLine from "remixicon-svelte/icons/checkbox-line";
  import RiGridLine from "remixicon-svelte/icons/grid-line";
  import RiListUnordered from "remixicon-svelte/icons/list-unordered";
  import RiMore2Line from "remixicon-svelte/icons/more-2-line";
  import { onMount } from "svelte";
  import { SvelteDate } from "svelte/reactivity";

  import DeckBatchDeletionFlow from "../components/DeckBatchDeletionFlow.svelte";
  import DeckDeletionFlow from "../components/DeckDeletionFlow.svelte";
  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import type { DeckSummaryDto } from "../lib/generated/DeckSummaryDto";
  import type { DeleteDeckResultDto } from "../lib/generated/DeleteDeckResultDto";
  import type { DeleteDecksResultDto } from "../lib/generated/DeleteDecksResultDto";
  import type { BundleRemovalPreviewDto } from "../lib/generated/BundleRemovalPreviewDto";
  import type { BundleRemovalProgressDto } from "../lib/generated/BundleRemovalProgressDto";
  import { localDayBounds } from "../lib/local-day";
  import {
    clearStudyQueue,
    clearStudySession,
    readStudyQueue,
    replaceStudyQueue,
    remainingStudyCards,
    type StudyQueueSession,
  } from "../lib/study-queue";

  type Props = {
    onStudy: (deckName: string) => void;
    onOpen: (deckId: string, deckName: string, isBundleStage: boolean) => void;
    onDeckContextChange: (value: string) => void;
    onChooseBundle: () => void;
    bundleImportRefresh: number;
    bundleImportRunning: boolean;
  };

  type DeckView = "grid" | "list";

  const selectedTodayDeckKey = "meiki-today-deck";
  const deckViewPreferenceKey = "meiki-decks-view";
  const allDecksId = "__all_decks__";
  const defaultDeckId = "default-deck";

  let {
    onStudy,
    onOpen,
    onDeckContextChange,
    onChooseBundle,
    bundleImportRefresh,
    bundleImportRunning,
  }: Props = $props();
  let decks = $state<DeckSummaryDto[]>([]);
  let activeQueue = $state<StudyQueueSession | null>(null);
  let newDeckName = $state("");
  let newDeckDialogOpen = $state(false);
  let loading = $state(true);
  let busyDeckId = $state("");
  let creating = $state(false);
  let installedBundles = $state<BundleRemovalPreviewDto[]>([]);
  let bundleActionsDialogOpen = $state(false);
  let exportingBundleLanguage = $state("");
  let bundleActionError = $state("");
  let bundleRemovalConfirmationOpen = $state(false);
  let bundleRemovalProgressDialogOpen = $state(false);
  let selectedBundle = $state<BundleRemovalPreviewDto | null>(null);
  let removingBundle = $state(false);
  let bundleRemovalProgress = $state<BundleRemovalProgressDto | null>(null);
  let bundleRemovalError = $state("");
  let error = $state("");
  let notice = $state("");
  let retryStudyDeck = $state<DeckSummaryDto | null>(null);
  let deleteTarget = $state<DeckSummaryDto | null>(null);
  let deleteFlowOpen = $state(false);
  let deckView = $state<DeckView>("grid");
  let selectionMode = $state(false);
  let selectedDeckIds = $state<string[]>([]);
  let batchDeleteDeckIds = $state<string[]>([]);
  let batchDeleteTargets = $state<DeckSummaryDto[]>([]);
  let batchDeleteFlowOpen = $state(false);
  let loadedBundleImportRefresh = $state<number | null>(null);

  onMount(() => {
    onDeckContextChange("All decks");
    if (localStorage.getItem(deckViewPreferenceKey) === "list") {
      deckView = "list";
    }
    const storedQueue = readStudyQueue();
    if (storedQueue && remainingStudyCards(storedQueue) > 0) {
      activeQueue = storedQueue;
    } else if (storedQueue) {
      clearStudyQueue();
    }
    void loadDecks();
  });

  $effect(() => {
    if (loadedBundleImportRefresh === null) {
      loadedBundleImportRefresh = bundleImportRefresh;
      return;
    }
    if (bundleImportRefresh === loadedBundleImportRefresh) return;
    loadedBundleImportRefresh = bundleImportRefresh;
    void loadDecks();
  });

  async function loadDecks(): Promise<void> {
    loading = true;
    error = "";
    try {
      const [loadedDecks, loadedBundles] = await Promise.all([
        api.listDeckSummaries(Date.now()),
        api.listInstalledBundles(),
      ]);
      decks = loadedDecks;
      installedBundles = loadedBundles;
      if (
        activeQueue &&
        activeQueue.deckId !== allDecksId &&
        !loadedDecks.some((deck) => deck.id === activeQueue?.deckId)
      ) {
        clearStudyQueue();
        clearStudySession();
        activeQueue = null;
      }
    } catch (cause) {
      error = message(cause);
    } finally {
      loading = false;
    }
  }

  async function createDeck(): Promise<void> {
    if (!newDeckName.trim() || creating) return;
    creating = true;
    error = "";
    notice = "";
    try {
      const created = await api.createDeck({
        name: newDeckName,
        now_ms: Date.now(),
      });
      newDeckName = "";
      newDeckDialogOpen = false;
      await loadDecks();
      notice = `Created deck “${created.name}”.`;
    } catch (cause) {
      error = message(cause);
    } finally {
      creating = false;
    }
  }

  function openDeleteDeck(deck: DeckSummaryDto): void {
    if (deck.id === defaultDeckId) return;
    deleteTarget = deck;
    deleteFlowOpen = true;
  }

  function selectDeckView(view: DeckView): void {
    deckView = view;
    localStorage.setItem(deckViewPreferenceKey, view);
  }

  function enterSelectionMode(): void {
    selectedDeckIds = [];
    selectionMode = true;
  }

  function clearSelection(): void {
    selectedDeckIds = [];
    selectionMode = false;
  }

  function toggleDeckSelection(deckId: string): void {
    if (deckId === defaultDeckId) return;
    selectedDeckIds = selectedDeckIds.includes(deckId)
      ? selectedDeckIds.filter((selectedDeckId) => selectedDeckId !== deckId)
      : [...selectedDeckIds, deckId];
  }

  function confirmBatchDeletion(): void {
    if (selectedDeckIds.length === 0) return;
    batchDeleteDeckIds = [...selectedDeckIds];
    batchDeleteTargets = decks.filter((deck) =>
      selectedDeckIds.includes(deck.id),
    );
    batchDeleteFlowOpen = true;
  }

  async function handleDeckDeletionCommitted(
    result: DeleteDeckResultDto,
  ): Promise<void> {
    const savedQueue = readStudyQueue();
    if (savedQueue?.deckId === result.deleted_deck_id) {
      clearStudyQueue();
      clearStudySession();
      activeQueue = null;
    }
    if (localStorage.getItem(selectedTodayDeckKey) === result.deleted_deck_id) {
      localStorage.setItem(selectedTodayDeckKey, allDecksId);
    }
    await loadDecks();
  }

  function finishDeckDeletion(result: DeleteDeckResultDto): void {
    const deletedName =
      deleteTarget?.id === result.deleted_deck_id ? deleteTarget.name : "deck";
    deleteTarget = null;
    notice = `Deleted ${deletedName}.`;
  }

  async function handleBatchDeletionCommitted(
    result: DeleteDecksResultDto,
  ): Promise<void> {
    const deletedDeckIds = new Set(result.deleted_deck_ids);
    const savedQueue = readStudyQueue();
    if (savedQueue && deletedDeckIds.has(savedQueue.deckId)) {
      clearStudyQueue();
      clearStudySession();
      activeQueue = null;
    }
    const selectedTodayDeck = localStorage.getItem(selectedTodayDeckKey);
    if (selectedTodayDeck && deletedDeckIds.has(selectedTodayDeck)) {
      localStorage.setItem(selectedTodayDeckKey, allDecksId);
    }
    clearSelection();
    await loadDecks();
  }

  function finishBatchDeletion(result: DeleteDecksResultDto): void {
    notice = `Deleted ${result.deleted_deck_ids.length.toLocaleString()} ${result.deleted_deck_ids.length === 1 ? "deck" : "decks"}.`;
    batchDeleteDeckIds = [];
    batchDeleteTargets = [];
  }

  function confirmBundleRemoval(bundle: BundleRemovalPreviewDto): void {
    selectedBundle = bundle;
    bundleActionsDialogOpen = false;
    bundleRemovalConfirmationOpen = true;
  }

  async function exportBundle(bundle: BundleRemovalPreviewDto): Promise<void> {
    if (exportingBundleLanguage) return;
    exportingBundleLanguage = bundle.language_tag;
    bundleActionError = "";
    notice = "";
    try {
      const result = await api.exportBundle({
        language_tag: bundle.language_tag,
        now_ms: Date.now(),
      });
      bundleActionsDialogOpen = false;
      notice = `Exported ${languageName(bundle.language_tag)} with ${result.decks.toLocaleString()} ${result.decks === 1 ? "deck" : "decks"} and ${result.cards.toLocaleString()} ${result.cards === 1 ? "card" : "cards"} to ${result.path}.`;
    } catch (cause) {
      bundleActionError = message(cause);
    } finally {
      exportingBundleLanguage = "";
    }
  }

  async function removeBundle(): Promise<void> {
    if (!selectedBundle || removingBundle) return;
    const bundle = selectedBundle;
    bundleRemovalConfirmationOpen = false;
    bundleRemovalProgressDialogOpen = true;
    removingBundle = true;
    bundleRemovalError = "";
    notice = "";
    const deckIdsBeforeRemoval = new Set(decks.map((deck) => deck.id));
    bundleRemovalProgress = {
      removed_decks: 0,
      total_decks: bundle.decks,
      processed_cards: 0,
      total_cards: bundle.cards,
    };
    try {
      const result = await api.removeBundle(
        {
          language_tag: bundle.language_tag,
          expected_decks: bundle.decks,
          expected_cards: bundle.cards,
          now_ms: Date.now(),
        },
        (progress) => (bundleRemovalProgress = progress),
      );
      await loadDecks();
      const selectedTodayDeck = localStorage.getItem(selectedTodayDeckKey);
      if (
        selectedTodayDeck &&
        deckIdsBeforeRemoval.has(selectedTodayDeck) &&
        !decks.some((deck) => deck.id === selectedTodayDeck)
      ) {
        localStorage.setItem(selectedTodayDeckKey, allDecksId);
      }
      bundleRemovalProgressDialogOpen = false;
      selectedBundle = null;
      notice = `Removed ${languageName(result.language_tag)} with ${result.removed_decks.toLocaleString()} ${result.removed_decks === 1 ? "deck" : "decks"}.`;
    } catch (cause) {
      bundleRemovalError = message(cause);
    } finally {
      removingBundle = false;
    }
  }

  async function beginStudy(deck: DeckSummaryDto): Promise<void> {
    if (activeQueue && remainingStudyCards(activeQueue) > 0) {
      if (activeQueue.deckId === deck.id) {
        onStudy(deck.name);
        return;
      }
    }
    busyDeckId = deck.id;
    error = "";
    notice = "";
    retryStudyDeck = null;
    try {
      const settings = await api.getSchedulerSettings(deck.id);
      const now = new SvelteDate();
      const { start, end } = localDayBounds(now, settings.day_boundary_minutes);
      const plan = await api.prepareStudy({
        deck_id: deck.id,
        now_ms: now.getTime(),
        day_start_ms: start.getTime(),
        day_end_ms: end.getTime(),
      });
      if (plan.availability !== "ready" || plan.overview.queue.length === 0) {
        notice = `${deck.name} has no cards ready to study.`;
        return;
      }
      activeQueue = await replaceStudyQueue(
        activeQueue,
        plan.overview,
        api.gradeReview,
      );
      onStudy(deck.name);
    } catch (cause) {
      error = message(cause);
      retryStudyDeck = deck;
    } finally {
      busyDeckId = "";
    }
  }

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }

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

  function studyActionLabel(deck: DeckSummaryDto): string {
    if (activeQueue?.deckId === deck.id) return "Resume";
    if (busyDeckId === deck.id) return "Planning…";
    return "Study";
  }
</script>

{#snippet deckCounts(deck: DeckSummaryDto)}
  <dl class="deck-counts">
    <div>
      <dt>Total</dt>
      <dd>{deck.total_cards}</dd>
    </div>
    <div>
      <dt>Due</dt>
      <dd>{deck.due_cards}</dd>
    </div>
    <div>
      <dt>New</dt>
      <dd>{deck.new_cards}</dd>
    </div>
  </dl>
{/snippet}

{#snippet deckNavigationActions(deck: DeckSummaryDto)}
  <Button
    variant="outline"
    onclick={() => onOpen(deck.id, deck.name, deck.is_bundle_stage)}
    >Open</Button
  >
  <Button
    disabled={busyDeckId !== "" || deck.total_cards === 0}
    onclick={() => void beginStudy(deck)}
  >
    {studyActionLabel(deck)}
  </Button>
{/snippet}

{#snippet deckActionsMenu(deck: DeckSummaryDto)}
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button
          {...props}
          size="icon-sm"
          variant="ghost"
          aria-label={`Actions for ${deck.name}`}
        >
          <RiMore2Line aria-hidden="true" />
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Portal>
      <DropdownMenu.Content
        align="end"
        sideOffset={4}
        class="z-50 min-w-36 rounded-none bg-popover p-1 text-popover-foreground shadow-md ring-1 ring-foreground/10"
      >
        <DropdownMenu.Item
          textValue="Delete deck"
          class="flex cursor-default items-center gap-2 rounded-none px-2 py-1.5 text-sm text-destructive outline-none select-none data-highlighted:bg-destructive/10"
          onSelect={() => openDeleteDeck(deck)}
        >
          <RiDeleteBin6Line class="size-4" aria-hidden="true" />
          Delete deck
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Portal>
  </DropdownMenu.Root>
{/snippet}

{#snippet deckSelectionControl(deck: DeckSummaryDto)}
  {@const selected = selectedDeckIds.includes(deck.id)}
  <Button
    size="icon-sm"
    variant={selected ? "default" : "outline"}
    role="checkbox"
    aria-checked={selected}
    aria-label={`Select ${deck.name}`}
    onclick={() => toggleDeckSelection(deck.id)}
  >
    {#if selected}
      <RiCheckboxLine aria-hidden="true" />
    {:else}
      <RiCheckboxBlankLine aria-hidden="true" />
    {/if}
  </Button>
{/snippet}

<section class="screen decks-screen" aria-labelledby="decks-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Your collection</span>
      <h1 id="decks-title" class="screen-title">Decks</h1>
      <p class="screen-description">
        Open a deck to manage its cards, or begin a focused study session.
      </p>
    </div>
    <div class="screen-actions">
      {#if installedBundles.length > 0}
        <Button
          variant="outline"
          onclick={() => {
            bundleActionError = "";
            bundleActionsDialogOpen = true;
          }}>Bundle actions</Button
        >
      {/if}
      <Button
        variant="outline"
        disabled={bundleImportRunning}
        onclick={onChooseBundle}>Import bundle</Button
      >
      <Button data-primary-action onclick={() => (newDeckDialogOpen = true)}
        >New deck</Button
      >
    </div>
  </header>

  {#if error}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>The deck action was not completed</Alert.Title>
      <Alert.Description>
        <p>{error}</p>
        <Button
          class="mt-3"
          variant="outline"
          onclick={() =>
            retryStudyDeck ? void beginStudy(retryStudyDeck) : void loadDecks()}
          >Try again</Button
        >
      </Alert.Description>
    </Alert.Root>
  {:else if notice}
    <Alert.Root role="status">
      <Alert.Title>{notice}</Alert.Title>
    </Alert.Root>
  {/if}

  {#if activeQueue && remainingStudyCards(activeQueue) > 0}
    <p class="saved-session-note">
      A saved session is active. Resume it, or start another deck to replace it.
    </p>
  {/if}

  <div class="deck-toolbar">
    <div class="deck-view-toolbar" role="group" aria-label="Deck view">
      <Button
        size="sm"
        variant={deckView === "grid" ? "default" : "outline"}
        aria-pressed={deckView === "grid"}
        onclick={() => selectDeckView("grid")}
      >
        <RiGridLine data-icon="inline-start" aria-hidden="true" />
        Grid
      </Button>
      <Button
        size="sm"
        variant={deckView === "list" ? "default" : "outline"}
        aria-pressed={deckView === "list"}
        onclick={() => selectDeckView("list")}
      >
        <RiListUnordered data-icon="inline-start" aria-hidden="true" />
        List
      </Button>
    </div>
    {#if selectionMode}
      <div class="deck-selection-toolbar" aria-label="Deck selection actions">
        <span
          role="status"
          aria-live="polite"
          data-testid="deck-selection-count"
        >
          {selectedDeckIds.length.toLocaleString()}
          {selectedDeckIds.length === 1 ? "deck" : "decks"} selected
        </span>
        <Button
          size="sm"
          variant="destructive"
          disabled={selectedDeckIds.length === 0}
          onclick={confirmBatchDeletion}>Delete selected</Button
        >
        <Button size="sm" variant="outline" onclick={clearSelection}
          >Clear selection</Button
        >
      </div>
    {:else}
      <Button
        size="sm"
        variant="outline"
        disabled={!decks.some((deck) => deck.id !== defaultDeckId)}
        onclick={enterSelectionMode}>Select</Button
      >
    {/if}
  </div>

  {#if deckView === "grid"}
    <div class="deck-grid" data-testid="deck-grid" aria-busy={loading}>
      {#if loading && decks.length === 0}
        <Card.Root class="p-6">
          <p class="text-muted-foreground">Loading decks…</p>
        </Card.Root>
      {:else}
        {#each decks as deck (deck.id)}
          <Card.Root
            class={selectionMode && selectedDeckIds.includes(deck.id)
              ? "gap-5 p-5 ring-2 ring-primary"
              : "gap-5 p-5"}
            data-testid={`deck-${deck.id}`}
          >
            <Card.Header class="p-0">
              <Card.Title class="[overflow-wrap:anywhere]" data-deck-name
                >{deck.name}</Card.Title
              >
              <Card.Description>
                {deck.total_cards}
                {deck.total_cards === 1 ? "card" : "cards"}
              </Card.Description>
              {#if deck.id !== defaultDeckId}
                <Card.Action>
                  <div class="deck-card-actions">
                    {#if selectionMode}
                      {@render deckSelectionControl(deck)}
                    {/if}
                    {@render deckActionsMenu(deck)}
                  </div>
                </Card.Action>
              {/if}
            </Card.Header>
            {@render deckCounts(deck)}
            <Card.Footer class="justify-end p-0">
              {@render deckNavigationActions(deck)}
            </Card.Footer>
          </Card.Root>
        {/each}
      {/if}
    </div>
  {:else}
    <div class="deck-list" data-testid="deck-list" aria-busy={loading}>
      {#if loading && decks.length === 0}
        <Card.Root class="p-6">
          <p class="text-muted-foreground">Loading decks…</p>
        </Card.Root>
      {:else}
        {#each decks as deck (deck.id)}
          <article
            class="deck-list-row"
            data-selected={selectionMode && selectedDeckIds.includes(deck.id)}
            data-testid={`deck-${deck.id}`}
          >
            <h2 class="deck-list-name" data-deck-name>{deck.name}</h2>
            {@render deckCounts(deck)}
            <div class="deck-list-actions">
              {@render deckNavigationActions(deck)}
              {#if deck.id !== defaultDeckId}
                {#if selectionMode}
                  {@render deckSelectionControl(deck)}
                {/if}
                {@render deckActionsMenu(deck)}
              {/if}
            </div>
          </article>
        {/each}
      {/if}
    </div>
  {/if}

  {#if !loading && decks.length === 0}
    <div class="empty-state">
      <span class="empty-mark" aria-hidden="true">＋</span>
      <h2>Create your first deck</h2>
      <p>Name a deck now, then add cards when you are ready.</p>
      <Button variant="outline" onclick={() => (newDeckDialogOpen = true)}
        >New deck</Button
      >
    </div>
  {/if}
</section>

{#if deleteTarget}
  <DeckDeletionFlow
    bind:open={deleteFlowOpen}
    deckId={deleteTarget.id}
    deckName={deleteTarget.name}
    isBundleStage={deleteTarget.is_bundle_stage}
    cardCount={deleteTarget.total_cards}
    destinationDecks={[
      ...(deleteTarget.id !== defaultDeckId &&
      !decks.some((deck) => deck.id === defaultDeckId)
        ? [{ id: defaultDeckId, name: "Unsorted" }]
        : []),
      ...decks
        .filter((deck) => deck.id !== deleteTarget?.id)
        .map(({ id, name }) => ({ id, name })),
    ]}
    onCommitted={handleDeckDeletionCommitted}
    onFinished={finishDeckDeletion}
  />
{/if}

{#if batchDeleteTargets.length > 0}
  <DeckBatchDeletionFlow
    bind:open={batchDeleteFlowOpen}
    deckIds={batchDeleteDeckIds}
    decks={batchDeleteTargets}
    onCommitted={handleBatchDeletionCommitted}
    onFinished={finishBatchDeletion}
  />
{/if}

<Dialog.Root bind:open={newDeckDialogOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>New deck</Dialog.Title>
      <Dialog.Description>
        Choose a name. You can set its daily time after opening it.
      </Dialog.Description>
    </Dialog.Header>
    <form
      class="grid gap-4"
      onsubmit={(event) => {
        event.preventDefault();
        void createDeck();
      }}
    >
      <div class="grid gap-2">
        <Label for="new-deck-name">Name</Label>
        <Input
          id="new-deck-name"
          bind:value={newDeckName}
          maxlength={80}
          autocomplete="off"
        />
      </div>
      <Dialog.Footer>
        <Button
          type="button"
          variant="outline"
          disabled={creating}
          onclick={() => (newDeckDialogOpen = false)}>Cancel</Button
        >
        <Button type="submit" disabled={creating || !newDeckName.trim()}>
          {creating ? "Creating…" : "Create deck"}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={bundleActionsDialogOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Bundle actions</Dialog.Title>
      <Dialog.Description>
        Manage language bundles installed in this collection.
      </Dialog.Description>
    </Dialog.Header>
    {#if bundleActionError}
      <Alert.Root variant="destructive" role="alert">
        <Alert.Title>The bundle was not exported</Alert.Title>
        <Alert.Description>{bundleActionError}</Alert.Description>
      </Alert.Root>
    {/if}
    <div class="bundle-action-list">
      {#each installedBundles as bundle (bundle.language_tag)}
        <div>
          <Button
            variant="outline"
            disabled={Boolean(exportingBundleLanguage)}
            onclick={() => void exportBundle(bundle)}
          >
            {exportingBundleLanguage === bundle.language_tag
              ? `Exporting ${languageName(bundle.language_tag)}…`
              : `Export ${languageName(bundle.language_tag)}`}
          </Button>
          <Button
            variant="outline"
            disabled={Boolean(exportingBundleLanguage)}
            onclick={() => confirmBundleRemoval(bundle)}
          >
            Remove {languageName(bundle.language_tag)}
            <span>
              {bundle.decks.toLocaleString()}
              {bundle.decks === 1 ? "deck" : "decks"}, {bundle.cards.toLocaleString()}
              {bundle.cards === 1 ? "card" : "cards"}
            </span>
          </Button>
        </div>
      {/each}
    </div>
    <Dialog.Footer>
      <Button
        variant="outline"
        disabled={Boolean(exportingBundleLanguage)}
        onclick={() => (bundleActionsDialogOpen = false)}>Close</Button
      >
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<AlertDialog.Root bind:open={bundleRemovalConfirmationOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>
        Remove {selectedBundle
          ? languageName(selectedBundle.language_tag)
          : "bundle"}?
      </AlertDialog.Title>
      <AlertDialog.Description>
        {#if selectedBundle}
          {`This permanently removes bundled content from ${selectedBundle.decks.toLocaleString()} ${selectedBundle.decks === 1 ? "deck" : "decks"}. Personal cards in those decks move to Trash.`}
        {/if}
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        class="bg-destructive/10 text-destructive hover:bg-destructive/20"
        onclick={() => void removeBundle()}>Remove bundle</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>

<Dialog.Root bind:open={bundleRemovalProgressDialogOpen}>
  <Dialog.Content
    class="sm:max-w-xl"
    showCloseButton={!removingBundle}
    onEscapeKeydown={(event) => removingBundle && event.preventDefault()}
    onInteractOutside={(event) => removingBundle && event.preventDefault()}
  >
    <Dialog.Header>
      <Dialog.Title>Removing bundle</Dialog.Title>
      <Dialog.Description>
        The collection is updated only after every deck is ready.
      </Dialog.Description>
    </Dialog.Header>
    {#if bundleRemovalError}
      <Alert.Root variant="destructive" role="alert">
        <Alert.Title>The bundle was not removed</Alert.Title>
        <Alert.Description>{bundleRemovalError}</Alert.Description>
      </Alert.Root>
    {:else if bundleRemovalProgress}
      <div class="bundle-removal-progress" role="status" aria-live="polite">
        <strong>Removing bundle content</strong>
        <label>
          Decks
          <progress
            max={Math.max(1, bundleRemovalProgress.total_decks)}
            value={bundleRemovalProgress.removed_decks}
          ></progress>
          <span>
            {bundleRemovalProgress.removed_decks.toLocaleString()} / {bundleRemovalProgress.total_decks.toLocaleString()}
          </span>
        </label>
        <label>
          Cards
          <progress
            max={Math.max(1, bundleRemovalProgress.total_cards)}
            value={bundleRemovalProgress.processed_cards}
          ></progress>
          <span>
            {bundleRemovalProgress.processed_cards.toLocaleString()} / {bundleRemovalProgress.total_cards.toLocaleString()}
          </span>
        </label>
      </div>
    {/if}
    {#if !removingBundle}
      <Dialog.Footer>
        <Button
          variant="outline"
          onclick={() => (bundleRemovalProgressDialogOpen = false)}
          >Close</Button
        >
      </Dialog.Footer>
    {/if}
  </Dialog.Content>
</Dialog.Root>

<style>
  .screen-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .deck-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 18rem), 1fr));
    gap: 1rem;
  }

  .deck-toolbar,
  .deck-view-toolbar,
  .deck-selection-toolbar,
  .deck-card-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }

  .deck-toolbar {
    justify-content: space-between;
    margin-bottom: 1rem;
  }

  .deck-selection-toolbar {
    justify-content: flex-end;
  }

  .deck-selection-toolbar span {
    color: var(--muted-foreground);
    font-size: 0.875rem;
  }

  .deck-card-actions {
    justify-content: flex-end;
  }

  .deck-list {
    display: grid;
    gap: 0.5rem;
    min-width: 0;
  }

  .deck-list-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(13rem, 18rem) auto;
    gap: 0.75rem;
    align-items: center;
    min-width: 0;
    padding: 0.875rem;
    border: 1px solid var(--border);
    background: var(--card);
  }

  .deck-list-row[data-selected="true"] {
    box-shadow: 0 0 0 2px var(--primary);
  }

  .deck-list-name {
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
    font-size: 1rem;
    line-height: 1.35;
  }

  .deck-list-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    justify-content: flex-end;
    min-width: 0;
  }

  .deck-counts {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.75rem;
  }

  .deck-counts div {
    display: grid;
    gap: 0.15rem;
    border: 1px solid var(--border);
    padding: 0.75rem;
  }

  .deck-counts dt,
  .saved-session-note {
    color: var(--muted-foreground);
    font-size: 0.8rem;
  }

  .deck-counts dd {
    font-size: 1.25rem;
    font-weight: 700;
  }

  .deck-list .deck-counts {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.5rem;
  }

  .deck-list .deck-counts div {
    gap: 0.1rem;
    padding: 0;
    border: 0;
  }

  .deck-list .deck-counts dd {
    font-size: 1rem;
  }

  .saved-session-note {
    margin: 0;
  }

  .bundle-action-list,
  .bundle-action-list > div,
  .bundle-removal-progress,
  .bundle-removal-progress label {
    display: grid;
    gap: 0.5rem;
  }

  .bundle-action-list :global(button) {
    height: auto;
    justify-content: space-between;
    text-align: left;
  }

  .bundle-action-list span,
  .bundle-removal-progress span {
    color: var(--muted-foreground);
    font-size: 0.8rem;
  }

  .bundle-removal-progress progress {
    width: 100%;
  }

  @media (max-width: 760px) {
    .deck-toolbar,
    .deck-selection-toolbar {
      justify-content: flex-start;
    }

    .deck-toolbar {
      align-items: flex-start;
      flex-direction: column;
    }

    .deck-list-row {
      grid-template-columns: minmax(0, 1fr);
    }

    .deck-list-actions {
      justify-content: flex-start;
    }
  }
</style>
