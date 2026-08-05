<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteDate } from "svelte/reactivity";

  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import type { DeckSummaryDto } from "../lib/generated/DeckSummaryDto";
  import type { BundleImportProgressDto } from "../lib/generated/BundleImportProgressDto";
  import type { BundlePreviewDto } from "../lib/generated/BundlePreviewDto";
  import type { BundleRemovalPreviewDto } from "../lib/generated/BundleRemovalPreviewDto";
  import type { BundleRemovalProgressDto } from "../lib/generated/BundleRemovalProgressDto";
  import { localDayBounds } from "../lib/local-day";
  import {
    clearStudyQueue,
    readStudyQueue,
    remainingStudyCards,
    startStudyQueue,
    type StudyQueueSession,
  } from "../lib/study-queue";

  type Props = {
    onStudy: (deckName: string) => void;
    onOpen: (deckId: string, deckName: string) => void;
    onDeckContextChange: (value: string) => void;
  };

  const selectedTodayDeckKey = "meiki-today-deck";
  const allDecksId = "__all_decks__";

  let { onStudy, onOpen, onDeckContextChange }: Props = $props();
  let decks = $state<DeckSummaryDto[]>([]);
  let activeQueue = $state<StudyQueueSession | null>(null);
  let newDeckName = $state("");
  let newDeckDialogOpen = $state(false);
  let loading = $state(true);
  let busyDeckId = $state("");
  let creating = $state(false);
  let bundleDialogOpen = $state(false);
  let bundlePath = $state("");
  let bundlePreview = $state<BundlePreviewDto | null>(null);
  let previewingBundle = $state(false);
  let importingBundle = $state(false);
  let bundleProgress = $state<BundleImportProgressDto | null>(null);
  let bundleError = $state("");
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

  onMount(() => {
    onDeckContextChange("All decks");
    const storedQueue = readStudyQueue();
    if (storedQueue && remainingStudyCards(storedQueue) > 0) {
      activeQueue = storedQueue;
    } else if (storedQueue) {
      clearStudyQueue();
    }
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

  async function chooseBundle(): Promise<void> {
    const path = await api.pickArchiveFile();
    if (!path) return;
    bundlePath = path;
    bundlePreview = null;
    bundleProgress = null;
    bundleError = "";
    bundleDialogOpen = true;
    previewingBundle = true;
    try {
      bundlePreview = await api.previewBundle(path);
    } catch (cause) {
      bundleError = message(cause);
    } finally {
      previewingBundle = false;
    }
  }

  async function addBundle(): Promise<void> {
    if (!bundlePreview?.can_import || importingBundle) return;
    importingBundle = true;
    bundleProgress = null;
    bundleError = "";
    notice = "";
    try {
      const result = await api.importBundle(
        { path: bundlePath, now_ms: Date.now() },
        (progress) => (bundleProgress = progress),
      );
      await loadDecks();
      bundleDialogOpen = false;
      notice =
        result.added_decks === 0
          ? `${languageName(result.language_tag)} is now installed.`
          : `Added ${languageName(result.language_tag)} with ${result.added_decks.toLocaleString()} ${result.added_decks === 1 ? "deck" : "decks"} and ${result.added_cards.toLocaleString()} ${result.added_cards === 1 ? "card" : "cards"}.`;
    } catch (cause) {
      bundleError = message(cause);
    } finally {
      importingBundle = false;
    }
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
      moved_cards: 0,
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
      notice = `Removed ${languageName(result.language_tag)}. ${result.removed_decks.toLocaleString()} ${result.removed_decks === 1 ? "deck" : "decks"} and ${result.moved_cards.toLocaleString()} ${result.moved_cards === 1 ? "card" : "cards"} moved to Trash.`;
    } catch (cause) {
      bundleRemovalError = message(cause);
    } finally {
      removingBundle = false;
    }
  }

  async function beginStudy(deck: DeckSummaryDto): Promise<void> {
    if (activeQueue && remainingStudyCards(activeQueue) > 0) {
      if (activeQueue.deckId === deck.id) onStudy(deck.name);
      return;
    }
    busyDeckId = deck.id;
    error = "";
    notice = "";
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
      activeQueue = startStudyQueue(plan.overview);
      onStudy(deck.name);
    } catch (cause) {
      error = message(cause);
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

  function progressLabel(progress: BundleImportProgressDto): string {
    if (progress.stage === "preparing_decks") return "Preparing decks";
    if (progress.stage === "adding_cards") return "Adding cards";
    return "Adding audio";
  }
</script>

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
      <Button variant="outline" onclick={() => void chooseBundle()}
        >Import bundle</Button
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
        <Button class="mt-3" variant="outline" onclick={loadDecks}
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
      A saved session is active. Resume its deck here, or return to Today for an
      all-decks session.
    </p>
  {/if}

  <div class="deck-grid" aria-busy={loading}>
    {#if loading && decks.length === 0}
      <Card.Root class="p-6">
        <p class="text-muted-foreground">Loading decks…</p>
      </Card.Root>
    {:else}
      {#each decks as deck (deck.id)}
        <Card.Root class="gap-5 p-5" data-testid={`deck-${deck.id}`}>
          <Card.Header class="p-0">
            <Card.Title>{deck.name}</Card.Title>
            <Card.Description>
              {deck.total_cards}
              {deck.total_cards === 1 ? "card" : "cards"}
            </Card.Description>
          </Card.Header>
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
          <Card.Footer class="justify-end p-0">
            <Button variant="outline" onclick={() => onOpen(deck.id, deck.name)}
              >Open</Button
            >
            <Button
              disabled={busyDeckId !== "" ||
                deck.total_cards === 0 ||
                Boolean(activeQueue && activeQueue.deckId !== deck.id)}
              onclick={() => void beginStudy(deck)}
            >
              {activeQueue && activeQueue.deckId === deck.id
                ? "Resume"
                : busyDeckId === deck.id
                  ? "Planning…"
                  : "Study"}
            </Button>
          </Card.Footer>
        </Card.Root>
      {/each}
    {/if}
  </div>

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
          {`This removes ${selectedBundle.decks.toLocaleString()} ${selectedBundle.decks === 1 ? "deck" : "decks"} and moves their ${selectedBundle.cards.toLocaleString()} ${selectedBundle.cards === 1 ? "card" : "cards"} to Trash.`}
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
        <strong>Moving cards to Trash</strong>
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
            value={bundleRemovalProgress.moved_cards}
          ></progress>
          <span>
            {bundleRemovalProgress.moved_cards.toLocaleString()} / {bundleRemovalProgress.total_cards.toLocaleString()}
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

<Dialog.Root bind:open={bundleDialogOpen}>
  <Dialog.Content class="sm:max-w-xl">
    <Dialog.Header>
      <Dialog.Title>Import bundle</Dialog.Title>
      <Dialog.Description>
        Add missing language decks without replacing your collection.
      </Dialog.Description>
    </Dialog.Header>

    {#if previewingBundle}
      <p role="status">Reading bundle details…</p>
    {:else if bundleError}
      <Alert.Root variant="destructive" role="alert">
        <Alert.Title>The bundle was not added</Alert.Title>
        <Alert.Description>{bundleError}</Alert.Description>
      </Alert.Root>
    {:else if bundlePreview}
      <div class="bundle-summary">
        <div>
          <span>Language</span>
          <strong>{languageName(bundlePreview.language_tag)}</strong>
        </div>
        <div>
          <span>Total cards</span>
          <strong>{bundlePreview.total_cards.toLocaleString()}</strong>
        </div>
        <div>
          <span>Audio</span>
          <strong>{bundlePreview.audio_objects.toLocaleString()}</strong>
        </div>
      </div>

      <ul class="bundle-decks" aria-label="Bundle decks">
        {#each bundlePreview.decks as deck (deck.id)}
          <li>
            <div>
              <strong>{deck.name}</strong>
              <span
                >{deck.cards.toLocaleString()}
                {deck.cards === 1 ? "card" : "cards"}</span
              >
            </div>
            <span class:installed={deck.status === "installed"}
              >{deck.status === "installed" ? "Installed" : "Missing"}</span
            >
          </li>
        {/each}
      </ul>

      {#if !bundlePreview.can_import}
        <p role="status">
          {languageName(bundlePreview.language_tag)} is already installed
        </p>
      {/if}
    {/if}

    {#if importingBundle && bundleProgress}
      <div class="bundle-progress" role="status" aria-live="polite">
        <strong>{progressLabel(bundleProgress)}</strong>
        {#if bundleProgress.stage !== "preparing_decks"}
          <progress
            max={Math.max(1, bundleProgress.total)}
            value={bundleProgress.current}
          ></progress>
          <span
            >{bundleProgress.current.toLocaleString()} / {bundleProgress.total.toLocaleString()}</span
          >
        {/if}
      </div>
    {/if}

    <Dialog.Footer>
      <Button
        type="button"
        variant="outline"
        disabled={importingBundle}
        onclick={() => (bundleDialogOpen = false)}>Close</Button
      >
      <Button
        type="button"
        disabled={!bundlePreview?.can_import || importingBundle}
        onclick={() => void addBundle()}
      >
        {importingBundle ? "Adding bundle…" : "Add bundle"}
      </Button>
    </Dialog.Footer>
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

  .saved-session-note {
    margin: 0;
  }

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

  @media (max-width: 36rem) {
    .bundle-summary {
      grid-template-columns: 1fr;
    }
  }
</style>
