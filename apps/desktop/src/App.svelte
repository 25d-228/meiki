<script lang="ts">
  import RiBookShelfLine from "remixicon-svelte/icons/book-shelf-line";
  import RiCalendarTodoLine from "remixicon-svelte/icons/calendar-todo-line";
  import RiEditLine from "remixicon-svelte/icons/edit-line";
  import RiKeyboardLine from "remixicon-svelte/icons/keyboard-line";
  import RiMenuLine from "remixicon-svelte/icons/menu-line";
  import RiSettings3Line from "remixicon-svelte/icons/settings-3-line";
  import { onMount, tick } from "svelte";
  import { SvelteMap } from "svelte/reactivity";

  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import * as Sheet from "$lib/components/ui/sheet/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import BundleImportActivity from "./components/BundleImportActivity.svelte";
  import DeletionActivity from "./components/DeletionActivity.svelte";
  import { api } from "./lib/api";
  import type {
    BundleDeletion,
    DeletionActivity as DeletionActivityState,
    DeletionProgress,
    MultipleDeckDeletion,
    SingleDeckDeletion,
  } from "./lib/deletion-activity";
  import type { BundleImportProgressDto } from "./lib/generated/BundleImportProgressDto";
  import type { BundleImportResultDto } from "./lib/generated/BundleImportResultDto";
  import type { BundleImportStageDto } from "./lib/generated/BundleImportStageDto";
  import type { BundlePreviewDto } from "./lib/generated/BundlePreviewDto";
  import type { BundleRemovalResultDto } from "./lib/generated/BundleRemovalResultDto";
  import type { DeleteDeckProgressDto } from "./lib/generated/DeleteDeckProgressDto";
  import type { DeleteDeckResultDto } from "./lib/generated/DeleteDeckResultDto";
  import type { DeleteDecksResultDto } from "./lib/generated/DeleteDecksResultDto";
  import { messages } from "./lib/messages";
  import {
    clearStudyQueue,
    clearStudySession,
    readStudyQueue,
  } from "./lib/study-queue";
  import type { TodayWarmData } from "./lib/today-warm-data";
  import { screens, type Screen, type ThemeMode } from "./lib/ui";
  import DeckManagementScreen from "./screens/DeckManagementScreen.svelte";
  import DecksScreen from "./screens/DecksScreen.svelte";
  import EditorScreen from "./screens/EditorScreen.svelte";
  import SettingsScreen from "./screens/SettingsScreen.svelte";
  import StudyScreen from "./screens/StudyScreen.svelte";
  import TodayScreen from "./screens/TodayScreen.svelte";
  import TypingScreen from "./screens/TypingScreen.svelte";

  const menuItems = [
    { id: "today", label: "Today", icon: RiCalendarTodoLine },
    { id: "decks", label: "Decks", icon: RiBookShelfLine },
    { id: "editor", label: "Add", icon: RiEditLine },
    { id: "typing", label: "Typing", icon: RiKeyboardLine },
    { id: "settings", label: "Settings", icon: RiSettings3Line },
  ];

  type BundleImportStatus =
    "choosing" | "previewing" | "ready" | "running" | "success" | "failure";

  type BundleImportActivity = {
    status: BundleImportStatus;
    path: string;
    preview: BundlePreviewDto | null;
    progress: BundleImportProgressDto | null;
    result: BundleImportResultDto | null;
    error: string;
  };

  let activeScreen: Screen = "today";
  let theme: ThemeMode = "system";
  let authoringDirty = false;
  let authoringComposing = false;
  let editingStudyCardId: string | null = null;
  let editingReturnScreen: "study" | "deck" | null = null;
  let studyReturnScreen: "today" | "decks" = "today";
  let selectedDeckId = "";
  let selectedDeckName = "";
  let selectedDeckIsBundleStage = false;
  let mainElement: HTMLElement;
  let mobileNavigationOpen = false;
  let discardDialogOpen = false;
  let pendingNavigation:
    { kind: "screen"; screen: Screen } | { kind: "return" } | null = null;
  let discardDescription = "";
  let announcement = "";
  let deckContext = "All decks";
  let bundleImportActivity: BundleImportActivity | null = null;
  let bundleImportDialogOpen = false;
  let bundleImportCardVisible = false;
  let bundleImportRefresh = 0;
  let deletionActivity: DeletionActivityState | null = null;
  let deletionDialogOpen = false;
  let deletionCardVisible = false;
  let deletionRefresh = 0;
  let todayRefresh = 0;
  const todayWarmData = new SvelteMap<string, TodayWarmData>();
  let nextDeletionOperationId = 1;
  $: bundleImportRunning =
    bundleImportActivity?.status === "choosing" ||
    bundleImportActivity?.status === "previewing" ||
    bundleImportActivity?.status === "running";
  $: deletionRunning = deletionActivity?.status === "running";

  function readTodayWarmData(
    deckId: string,
    nowMs: number,
  ): TodayWarmData | null {
    const warmData = todayWarmData.get(deckId);
    if (
      !warmData ||
      nowMs < warmData.dayStartMs ||
      nowMs >= warmData.dayEndMs
    ) {
      if (warmData) todayWarmData.delete(deckId);
      return null;
    }
    return warmData;
  }

  function writeTodayWarmData(warmData: TodayWarmData): void {
    todayWarmData.set(warmData.deckId, warmData);
  }

  function invalidateTodayWarmData(): void {
    todayWarmData.clear();
    todayRefresh += 1;
  }

  onMount(() => {
    const savedTheme = localStorage.getItem("meiki-theme");
    if (isTheme(savedTheme)) theme = savedTheme;
    applyTheme(theme);

    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
    const syncSystemTheme = () => {
      if (theme === "system") setDarkClass(systemTheme.matches);
    };
    systemTheme.addEventListener("change", syncSystemTheme);

    const trackAuthoring = (event: Event) => {
      const detail = (
        event as CustomEvent<{ dirty: boolean; composing: boolean }>
      ).detail;
      authoringDirty = detail.dirty;
      authoringComposing = detail.composing;
    };
    window.addEventListener("meiki-authoring-state", trackAuthoring);
    return () => {
      systemTheme.removeEventListener("change", syncSystemTheme);
      window.removeEventListener("meiki-authoring-state", trackAuthoring);
    };
  });

  function isTheme(value: string | null): value is ThemeMode {
    return value === "system" || value === "light" || value === "dark";
  }

  function isScreen(value: string): value is Screen {
    return screens.includes(value as Screen);
  }

  function screenLabel(screen: Screen): string {
    if (screen === "study") return "Study";
    if (screen === "deck") return selectedDeckName || "Deck";
    if (screen === "editor") return editingStudyCardId ? "Edit" : "Add";
    return menuItems.find((item) => item.id === screen)?.label ?? "Today";
  }

  function applyTheme(nextTheme: ThemeMode): void {
    theme = nextTheme;
    document.documentElement.dataset.theme = nextTheme;
    const prefersDark = window.matchMedia(
      "(prefers-color-scheme: dark)",
    ).matches;
    setDarkClass(
      nextTheme === "dark" || (nextTheme === "system" && prefersDark),
    );
    localStorage.setItem("meiki-theme", nextTheme);
  }

  function setDarkClass(enabled: boolean): void {
    document.documentElement.classList.toggle("dark", enabled);
    document.documentElement.style.colorScheme = enabled ? "dark" : "light";
  }

  async function chooseBundle(): Promise<void> {
    if (bundleImportRunning) return;
    if (bundleImportActivity?.status === "ready") {
      bundleImportDialogOpen = true;
      return;
    }
    bundleImportCardVisible = false;
    bundleImportActivity = {
      status: "choosing",
      path: "",
      preview: null,
      progress: null,
      result: null,
      error: "",
    };
    try {
      const path = await api.pickArchiveFile();
      if (!path) {
        bundleImportActivity = null;
        return;
      }
      bundleImportActivity = {
        status: "previewing",
        path,
        preview: null,
        progress: null,
        result: null,
        error: "",
      };
      bundleImportDialogOpen = true;
      try {
        const preview = await api.previewBundle(path);
        if (bundleImportActivity?.path !== path) return;
        bundleImportActivity.preview = preview;
        bundleImportActivity.status = "ready";
      } catch (cause) {
        if (bundleImportActivity?.path !== path) return;
        bundleImportActivity.error = message(cause);
        bundleImportActivity.status = "failure";
        bundleImportCardVisible = true;
      }
    } catch (cause) {
      bundleImportActivity = {
        status: "failure",
        path: "",
        preview: null,
        progress: null,
        result: null,
        error: message(cause),
      };
      bundleImportCardVisible = true;
      bundleImportDialogOpen = true;
    }
  }

  async function addBundle(): Promise<void> {
    if (
      bundleImportActivity?.status !== "ready" ||
      !bundleImportActivity.preview?.can_import
    )
      return;
    const path = bundleImportActivity.path;
    bundleImportActivity.status = "running";
    bundleImportCardVisible = true;
    bundleImportActivity.error = "";
    bundleImportActivity.progress = {
      stage: "preparing_decks",
      current: 0,
      total: bundleImportActivity.preview.decks.length,
    };
    try {
      const result = await api.importBundle(
        { path, now_ms: Date.now() },
        updateBundleImportProgress,
      );
      if (bundleImportActivity?.path !== path) return;
      bundleImportActivity.result = result;
      bundleImportActivity.status = "success";
      bundleImportCardVisible = true;
      bundleImportDialogOpen = false;
      bundleImportRefresh += 1;
      invalidateTodayWarmData();
    } catch (cause) {
      if (bundleImportActivity?.path !== path) return;
      bundleImportActivity.error = message(cause);
      bundleImportActivity.status = "failure";
      bundleImportCardVisible = true;
    }
  }

  function updateBundleImportProgress(progress: BundleImportProgressDto): void {
    if (bundleImportActivity?.status !== "running") return;
    const previous = bundleImportActivity.progress;
    if (
      previous &&
      (bundleImportStageIndex(progress.stage) <
        bundleImportStageIndex(previous.stage) ||
        (progress.stage === previous.stage &&
          progress.current < previous.current))
    ) {
      return;
    }
    bundleImportActivity.progress = progress;
  }

  function bundleImportStageIndex(stage: BundleImportStageDto): number {
    if (stage === "preparing_decks") return 0;
    if (stage === "adding_cards") return 1;
    return 2;
  }

  function hideBundleImportCard(): void {
    if (
      bundleImportActivity?.status !== "success" &&
      bundleImportActivity?.status !== "failure"
    )
      return;
    bundleImportCardVisible = false;
  }

  function abandonBundlePreview(): void {
    if (bundleImportActivity?.status !== "ready") return;
    bundleImportActivity = null;
    bundleImportCardVisible = false;
  }

  function beginDeletion(
    kind: DeletionActivityState["kind"],
    name: string,
  ): number | null {
    if (deletionRunning) {
      announcement = "Another deletion is already running.";
      return null;
    }
    const operationId = nextDeletionOperationId;
    nextDeletionOperationId += 1;
    deletionActivity = {
      operationId,
      kind,
      status: "running",
      name,
      progress: { phase: "preparing", current: null, total: null },
      message: "",
    };
    deletionCardVisible = true;
    deletionDialogOpen = true;
    return operationId;
  }

  function updateDeletionProgress(
    operationId: number,
    progress: DeletionProgress,
  ): void {
    if (
      deletionActivity?.operationId !== operationId ||
      deletionActivity.status !== "running"
    )
      return;
    const previous = deletionActivity.progress;
    const previousPhase = deletionPhaseIndex(previous.phase);
    const nextPhase = deletionPhaseIndex(progress.phase);
    if (
      nextPhase < previousPhase ||
      (nextPhase === previousPhase &&
        previous.current !== null &&
        progress.current !== null &&
        progress.current < previous.current)
    )
      return;
    deletionActivity.progress = progress;
  }

  function deletionPhaseIndex(phase: DeletionProgress["phase"]): number {
    if (phase === "preparing") return 0;
    if (phase === "removing_cards") return 1;
    if (phase === "cleaning_audio") return 2;
    return 3;
  }

  function finishDeletion(
    operationId: number,
    status: "success" | "warning" | "failure",
    message: string,
  ): void {
    if (deletionActivity?.operationId !== operationId) return;
    deletionActivity.status = status;
    deletionActivity.message = message;
    deletionCardVisible = true;
  }

  function dismissDeletion(): void {
    if (deletionActivity?.status === "running") return;
    deletionCardVisible = false;
  }

  async function applyDeletedDeckCleanup(
    deletedDeckIds: string[],
  ): Promise<void> {
    const deletedIds = new Set(deletedDeckIds);
    const savedQueue = readStudyQueue();
    if (
      savedQueue &&
      savedQueue.deckId !== "__all_decks__" &&
      deletedIds.has(savedQueue.deckId)
    ) {
      clearStudyQueue();
      clearStudySession();
    }
    const selectedTodayDeck = localStorage.getItem("meiki-today-deck");
    if (selectedTodayDeck && deletedIds.has(selectedTodayDeck)) {
      localStorage.setItem("meiki-today-deck", "__all_decks__");
    }
    deletionRefresh += 1;
    if (activeScreen === "deck" && deletedIds.has(selectedDeckId)) {
      const deletedDeckName = selectedDeckName;
      selectedDeckId = "";
      selectedDeckName = "";
      selectedDeckIsBundleStage = false;
      announcement = `Deleted ${deletedDeckName}. Returning to Decks.`;
      await performNavigation("decks");
    }
  }

  async function deleteSingleDeck(deletion: SingleDeckDeletion): Promise<void> {
    const operationId = beginDeletion(
      "deck",
      `Deleting “${deletion.deckName}”`,
    );
    if (operationId === null) return;
    let result: DeleteDeckResultDto;
    try {
      result = await api.deleteDeck(
        {
          deck_id: deletion.deckId,
          move_cards_to_deck_id: deletion.moveCardsToDeckId,
          confirmation: deletion.deckName,
          now_ms: Date.now(),
        },
        (progress: DeleteDeckProgressDto) =>
          updateDeletionProgress(operationId, progress),
      );
    } catch {
      finishDeletion(
        operationId,
        "failure",
        "Could not delete the deck. Try again.",
      );
      return;
    }
    invalidateTodayWarmData();
    try {
      await applyDeletedDeckCleanup([result.deleted_deck_id]);
    } catch {
      deletionRefresh += 1;
      finishDeletion(
        operationId,
        "warning",
        "Deck deleted, but the collection view could not be refreshed.",
      );
      return;
    }
    if (result.media_cleanup_warning) {
      finishDeletion(
        operationId,
        "warning",
        "Deck deleted, but some unused audio could not be cleaned up.",
      );
      return;
    }
    finishDeletion(operationId, "success", `Deleted ${deletion.deckName}.`);
  }

  async function deleteMultipleDecks(
    deletion: MultipleDeckDeletion,
  ): Promise<void> {
    const count = deletion.deckIds.length;
    const operationId = beginDeletion(
      "decks",
      `Deleting ${count.toLocaleString()} ${count === 1 ? "deck" : "decks"}`,
    );
    if (operationId === null) return;
    let result: DeleteDecksResultDto;
    try {
      result = await api.deleteDecks(
        { deck_ids: deletion.deckIds, now_ms: Date.now() },
        (progress: DeleteDeckProgressDto) =>
          updateDeletionProgress(operationId, progress),
      );
    } catch {
      finishDeletion(
        operationId,
        "failure",
        "Could not delete the selected decks. Try again.",
      );
      return;
    }
    invalidateTodayWarmData();
    try {
      await applyDeletedDeckCleanup(result.deleted_deck_ids);
    } catch {
      deletionRefresh += 1;
      finishDeletion(
        operationId,
        "warning",
        "Decks deleted, but the collection view could not be refreshed.",
      );
      return;
    }
    if (result.media_cleanup_warning) {
      finishDeletion(
        operationId,
        "warning",
        "Decks deleted, but some unused audio could not be cleaned up.",
      );
      return;
    }
    finishDeletion(
      operationId,
      "success",
      `Deleted ${result.deleted_deck_ids.length.toLocaleString()} ${result.deleted_deck_ids.length === 1 ? "deck" : "decks"}.`,
    );
  }

  async function removeBundle(deletion: BundleDeletion): Promise<void> {
    const bundle = deletion.bundle;
    const language = languageName(bundle.language_tag);
    const operationId = beginDeletion("bundle", `Removing ${language}`);
    if (operationId === null) return;
    let result: BundleRemovalResultDto;
    try {
      result = await api.removeBundle(
        {
          language_tag: bundle.language_tag,
          expected_decks: bundle.decks,
          expected_cards: bundle.cards,
          now_ms: Date.now(),
        },
        (progress) =>
          updateDeletionProgress(operationId, {
            phase: "removing_cards",
            current: progress.processed_cards,
            total: progress.total_cards,
          }),
      );
    } catch {
      finishDeletion(
        operationId,
        "failure",
        `Could not remove ${language}. Try again.`,
      );
      return;
    }
    invalidateTodayWarmData();
    try {
      const remainingDecks = await api.listDeckSummaries(Date.now());
      const remainingDeckIds = new Set(remainingDecks.map((deck) => deck.id));
      await applyDeletedDeckCleanup(
        deletion.deckIdsBeforeRemoval.filter(
          (deckId) => !remainingDeckIds.has(deckId),
        ),
      );
    } catch {
      deletionRefresh += 1;
      finishDeletion(
        operationId,
        "warning",
        `${language} was removed, but the collection view could not be refreshed.`,
      );
      return;
    }
    if (result.media_cleanup_warning) {
      finishDeletion(
        operationId,
        "warning",
        `${language} was removed, but some unused audio could not be cleaned up.`,
      );
      return;
    }
    finishDeletion(
      operationId,
      "success",
      `Removed ${language} with ${result.removed_decks.toLocaleString()} ${result.removed_decks === 1 ? "deck" : "decks"}.`,
    );
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

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }

  async function performNavigation(value: Screen): Promise<void> {
    discardDialogOpen = false;
    pendingNavigation = null;
    mobileNavigationOpen = false;
    authoringDirty = false;
    authoringComposing = false;
    activeScreen = value;
    editingStudyCardId = null;
    editingReturnScreen = null;
    await tick();
    mainElement.focus();
  }

  async function navigate(value: string): Promise<void> {
    if (!isScreen(value)) return;
    if (activeScreen === "editor" && value !== "editor" && authoringDirty) {
      if (authoringComposing) return;
      pendingNavigation = { kind: "screen", screen: value };
      discardDescription =
        "Your unsaved card changes will be lost when you leave the editor.";
      discardDialogOpen = true;
      return;
    }
    await performNavigation(value);
  }

  async function editStudyCard(cardId: string): Promise<void> {
    editingReturnScreen = "study";
    editingStudyCardId = cardId;
    activeScreen = "editor";
    await tick();
    mainElement.focus();
  }

  async function editDeckCard(cardId: string): Promise<void> {
    editingReturnScreen = "deck";
    editingStudyCardId = cardId;
    activeScreen = "editor";
    await tick();
    mainElement.focus();
  }

  async function returnFromEditor(): Promise<void> {
    if (authoringDirty) {
      if (authoringComposing) return;
      const destination =
        editingReturnScreen === "deck" ? selectedDeckName : "study";
      pendingNavigation = { kind: "return" };
      discardDescription = `Your unsaved card changes will be lost when you return to ${destination}.`;
      discardDialogOpen = true;
      return;
    }
    await performEditorReturn();
  }

  async function performEditorReturn(): Promise<void> {
    if (!editingReturnScreen) return;
    discardDialogOpen = false;
    pendingNavigation = null;
    authoringDirty = false;
    authoringComposing = false;
    activeScreen = editingReturnScreen;
    editingStudyCardId = null;
    await tick();
    mainElement.focus();
  }

  async function handleEditorSaved(): Promise<void> {
    invalidateTodayWarmData();
    if (editingReturnScreen) await performEditorReturn();
  }

  async function confirmDiscard(): Promise<void> {
    if (pendingNavigation?.kind === "screen") {
      await performNavigation(pendingNavigation.screen);
      return;
    }
    if (pendingNavigation?.kind === "return") await performEditorReturn();
  }

  async function startStudy(
    returnScreen: "today" | "decks",
    deckName: string,
  ): Promise<void> {
    studyReturnScreen = returnScreen;
    deckContext = deckName;
    await performNavigation("study");
  }

  async function openDeck(
    deckId: string,
    deckName: string,
    isBundleStage: boolean,
  ): Promise<void> {
    selectedDeckId = deckId;
    selectedDeckName = deckName;
    selectedDeckIsBundleStage = isBundleStage;
    deckContext = deckName;
    await performNavigation("deck");
  }

  function renameSelectedDeck(deckName: string): void {
    selectedDeckName = deckName;
    deckContext = deckName;
  }

  async function addDeckNote(): Promise<void> {
    editingReturnScreen = "deck";
    editingStudyCardId = null;
    activeScreen = "editor";
    await tick();
    mainElement.focus();
  }

  async function finishStudyQueue(): Promise<void> {
    const destination = studyReturnScreen === "decks" ? "Decks" : "Today";
    announcement = `Study queue complete. Returning to ${destination}.`;
    activeScreen = studyReturnScreen;
    editingStudyCardId = null;
    await tick();
    mainElement.focus();
  }
</script>

<svelte:head>
  <title>{screenLabel(activeScreen)} · {messages.appName}</title>
</svelte:head>
<a class="skip-link" href="#main-content">Skip to content</a>
<p
  class="visually-hidden"
  role="status"
  aria-live="polite"
  data-testid="app-announcement"
>
  {announcement}
</p>

<Tooltip.Provider>
  <div class="min-h-screen bg-background" data-testid="app-shell" dir="ltr">
    <header
      class="sticky top-0 z-40 flex h-12 items-center justify-between border-b bg-background/95 px-3 backdrop-blur sm:px-5"
    >
      <div class="flex min-w-0 items-center gap-2">
        <Sheet.Root bind:open={mobileNavigationOpen}>
          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Sheet.Trigger
                  {...props}
                  class="inline-flex size-8 items-center justify-center rounded-lg hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none md:hidden"
                  aria-label="Open navigation"
                >
                  <RiMenuLine />
                </Sheet.Trigger>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content>Open navigation</Tooltip.Content>
          </Tooltip.Root>
          <Sheet.Content side="left" class="w-72">
            <Sheet.Header>
              <Sheet.Title>
                <span lang="ja">{messages.appName}</span>
              </Sheet.Title>
              <Sheet.Description>{messages.appTagline}</Sheet.Description>
            </Sheet.Header>
            <nav class="mt-5 grid gap-1" aria-label="Primary navigation">
              {#each menuItems as item (item.id)}
                {@const Icon = item.icon}
                <Button
                  class="w-full justify-start"
                  variant={activeScreen === item.id ? "secondary" : "ghost"}
                  aria-current={activeScreen === item.id ? "page" : undefined}
                  onclick={() => void navigate(item.id)}
                >
                  <Icon data-icon="inline-start" />
                  {item.label}
                </Button>
              {/each}
            </nav>
          </Sheet.Content>
        </Sheet.Root>

        <Button
          variant="ghost"
          class="min-w-0 justify-start px-1.5"
          aria-label="Go to Today"
          onclick={() => void navigate("today")}
        >
          <span
            class="shrink-0 text-sm font-extrabold tracking-[0.08em]"
            lang="ja">{messages.appName}</span
          >
          <span class="hidden truncate text-xs text-muted-foreground sm:inline">
            {screenLabel(activeScreen)}
            {screenLabel(activeScreen) === deckContext
              ? ""
              : ` · ${deckContext}`}
          </span>
        </Button>
      </div>

      <div class="flex items-center">
        <Select.Root
          type="single"
          value={theme}
          onValueChange={(value) => applyTheme(value as ThemeMode)}
        >
          <Select.Trigger aria-label="Theme" class="w-24 capitalize">
            {theme}
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="system" label="System">System</Select.Item>
            <Select.Item value="light" label="Light">Light</Select.Item>
            <Select.Item value="dark" label="Dark">Dark</Select.Item>
          </Select.Content>
        </Select.Root>
      </div>
    </header>

    <div
      class="mx-auto grid w-full max-w-7xl grid-cols-1 md:grid-cols-[12rem_minmax(0,1fr)] md:gap-8 md:px-5"
    >
      <aside class="hidden py-6 md:block">
        <nav class="sticky top-18 grid gap-1" aria-label="Primary navigation">
          {#each menuItems as item (item.id)}
            {@const Icon = item.icon}
            <Button
              class="w-full justify-start"
              variant={activeScreen === item.id ? "secondary" : "ghost"}
              aria-current={activeScreen === item.id ? "page" : undefined}
              onclick={() => void navigate(item.id)}
            >
              <Icon data-icon="inline-start" />
              {item.label}
            </Button>
          {/each}
        </nav>
      </aside>

      <main
        id="main-content"
        class="min-h-[calc(100vh-3rem)] min-w-0 px-3 py-6 outline-none sm:px-5 md:px-0 md:py-8"
        bind:this={mainElement}
        tabindex="-1"
      >
        {#if activeScreen === "today"}
          <TodayScreen
            onStart={() => void startStudy("today", deckContext)}
            onSettings={() => void navigate("settings")}
            onDeckContextChange={(value) => (deckContext = value)}
            {todayRefresh}
            readWarmData={readTodayWarmData}
            writeWarmData={writeTodayWarmData}
            onTodayMutation={invalidateTodayWarmData}
          />
        {:else if activeScreen === "decks"}
          <DecksScreen
            onStudy={(deckName) => void startStudy("decks", deckName)}
            onOpen={(deckId, deckName, isBundleStage) =>
              void openDeck(deckId, deckName, isBundleStage)}
            onDeckContextChange={(value) => (deckContext = value)}
            onChooseBundle={() => void chooseBundle()}
            {bundleImportRefresh}
            {bundleImportRunning}
            {deletionRefresh}
            {deletionRunning}
            onDeleteDeck={(deletion) => void deleteSingleDeck(deletion)}
            onDeleteDecks={(deletion) => void deleteMultipleDecks(deletion)}
            onRemoveBundle={(deletion) => void removeBundle(deletion)}
            onProgressReset={() => {
              invalidateTodayWarmData();
            }}
            onTodayMutation={invalidateTodayWarmData}
          />
        {:else if activeScreen === "study"}
          <StudyScreen
            onCreate={() => void navigate("editor")}
            onEdit={editStudyCard}
            onQueueComplete={finishStudyQueue}
            onTodayMutation={invalidateTodayWarmData}
          />
        {:else if activeScreen === "deck"}
          <DeckManagementScreen
            {selectedDeckId}
            deckName={selectedDeckName}
            isBundleStage={selectedDeckIsBundleStage}
            onBack={() => void navigate("decks")}
            onCreate={() => void addDeckNote()}
            {deletionRunning}
            onDeleteDeck={(deletion) => void deleteSingleDeck(deletion)}
            onEdit={editDeckCard}
            onRename={renameSelectedDeck}
            onTodayMutation={invalidateTodayWarmData}
          />
        {:else if activeScreen === "editor"}
          <EditorScreen
            cardId={editingStudyCardId}
            preferredDeckId={editingReturnScreen === "deck"
              ? selectedDeckId
              : undefined}
            onReturn={editingReturnScreen ? returnFromEditor : undefined}
            onSaved={handleEditorSaved}
            returnLabel={editingReturnScreen === "deck"
              ? "Cancel"
              : "Return to study"}
          />
        {:else if activeScreen === "typing"}
          <TypingScreen />
        {:else}
          <SettingsScreen
            {theme}
            onThemeChange={applyTheme}
            onTodayMutation={invalidateTodayWarmData}
          />
        {/if}
      </main>
    </div>
  </div>

  <div
    class="pointer-events-none fixed right-3 bottom-3 z-40 grid w-[min(16rem,calc(100vw-1.5rem))] gap-3 sm:right-5 sm:bottom-5"
    data-testid="app-activity-stack"
  >
    <DeletionActivity
      activity={deletionActivity}
      cardVisible={deletionCardVisible}
      bind:dialogOpen={deletionDialogOpen}
      onDismiss={dismissDeletion}
    />
    <BundleImportActivity
      activity={bundleImportActivity}
      cardVisible={bundleImportCardVisible}
      bind:dialogOpen={bundleImportDialogOpen}
      onAdd={() => void addBundle()}
      onAbandon={abandonBundlePreview}
      onDismiss={hideBundleImportCard}
    />
  </div>
</Tooltip.Provider>

<AlertDialog.Root bind:open={discardDialogOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Discard unsaved changes?</AlertDialog.Title>
      <AlertDialog.Description>{discardDescription}</AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Keep editing</AlertDialog.Cancel>
      <AlertDialog.Action
        class="bg-destructive/10 text-destructive hover:bg-destructive/20"
        onclick={() => void confirmDiscard()}
      >
        Discard changes
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
