<script lang="ts">
  import CalendarDaysIcon from "@lucide/svelte/icons/calendar-days";
  import LibraryIcon from "@lucide/svelte/icons/library";
  import MenuIcon from "@lucide/svelte/icons/menu";
  import PenLineIcon from "@lucide/svelte/icons/pen-line";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import SparklesIcon from "@lucide/svelte/icons/sparkles";
  import { onMount, tick } from "svelte";

  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import * as Sheet from "$lib/components/ui/sheet/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import { messages } from "./lib/messages";
  import { screens, type Screen, type ThemeMode } from "./lib/ui";
  import EditorScreen from "./screens/EditorScreen.svelte";
  import LibraryScreen from "./screens/LibraryScreen.svelte";
  import SettingsScreen from "./screens/SettingsScreen.svelte";
  import StudyScreen from "./screens/StudyScreen.svelte";
  import TodayScreen from "./screens/TodayScreen.svelte";

  const menuItems = [
    { id: "today", label: "Today", icon: CalendarDaysIcon },
    { id: "study", label: "Study", icon: SparklesIcon },
    { id: "library", label: "Library", icon: LibraryIcon },
    { id: "editor", label: "Add / Edit", icon: PenLineIcon },
    { id: "settings", label: "Settings", icon: SettingsIcon },
  ];

  let activeScreen: Screen = "today";
  let theme: ThemeMode = "system";
  let authoringDirty = false;
  let authoringComposing = false;
  let editingStudyCardId: string | null = null;
  let editingReturnScreen: "study" | "library" = "study";
  let mainElement: HTMLElement;
  let mobileNavigationOpen = false;
  let discardDialogOpen = false;
  let pendingNavigation:
    { kind: "screen"; screen: Screen } | { kind: "return" } | null = null;
  let discardDescription = "";
  let announcement = "";
  let deckContext = "All decks";

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

  async function performNavigation(value: Screen): Promise<void> {
    discardDialogOpen = false;
    pendingNavigation = null;
    mobileNavigationOpen = false;
    authoringDirty = false;
    authoringComposing = false;
    activeScreen = value;
    if (value !== "editor") editingStudyCardId = null;
    await tick();
    mainElement.focus();
  }

  async function navigate(value: string): Promise<void> {
    if (!isScreen(value)) return;
    if (activeScreen === "editor" && value !== "editor" && authoringDirty) {
      if (authoringComposing) return;
      pendingNavigation = { kind: "screen", screen: value };
      discardDescription =
        "Your unsaved source note changes will be lost when you leave the editor.";
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

  async function editLibraryCard(cardId: string): Promise<void> {
    editingReturnScreen = "library";
    editingStudyCardId = cardId;
    activeScreen = "editor";
    await tick();
    mainElement.focus();
  }

  async function returnFromEditor(): Promise<void> {
    if (authoringDirty) {
      if (authoringComposing) return;
      const destination =
        editingReturnScreen === "library" ? "Library" : "study";
      pendingNavigation = { kind: "return" };
      discardDescription = `Your unsaved source note changes will be lost when you return to ${destination}.`;
      discardDialogOpen = true;
      return;
    }
    await performEditorReturn();
  }

  async function performEditorReturn(): Promise<void> {
    discardDialogOpen = false;
    pendingNavigation = null;
    authoringDirty = false;
    authoringComposing = false;
    activeScreen = editingReturnScreen;
    editingStudyCardId = null;
    await tick();
    mainElement.focus();
  }

  async function confirmDiscard(): Promise<void> {
    if (pendingNavigation?.kind === "screen") {
      await performNavigation(pendingNavigation.screen);
      return;
    }
    if (pendingNavigation?.kind === "return") await performEditorReturn();
  }

  async function finishStudyQueue(): Promise<void> {
    announcement = "Study queue complete. Returning to Today.";
    activeScreen = "today";
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
                  <MenuIcon />
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
            {screenLabel(activeScreen)} · {deckContext}
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
            onStart={() => void navigate("study")}
            onSettings={() => void navigate("settings")}
            onDeckContextChange={(value) => (deckContext = value)}
          />
        {:else if activeScreen === "study"}
          <StudyScreen
            onCreate={() => void navigate("editor")}
            onEdit={editStudyCard}
            onQueueComplete={finishStudyQueue}
          />
        {:else if activeScreen === "library"}
          <LibraryScreen onNavigate={navigate} onEdit={editLibraryCard} />
        {:else if activeScreen === "editor"}
          <EditorScreen
            cardId={editingStudyCardId}
            onReturn={editingStudyCardId ? returnFromEditor : undefined}
            returnLabel={editingReturnScreen === "library"
              ? "Return to Library"
              : "Return to study"}
          />
        {:else}
          <SettingsScreen {theme} onThemeChange={applyTheme} />
        {/if}
      </main>
    </div>
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
