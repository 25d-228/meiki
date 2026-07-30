<script lang="ts">
  import { onMount, tick } from "svelte";

  import Feedback from "./lib/components/Feedback.svelte";
  import Menu, { type MenuItem } from "./lib/components/Menu.svelte";
  import { messages } from "./lib/messages";
  import { screens, type Screen, type ThemeMode } from "./lib/ui";
  import EditorScreen from "./screens/EditorScreen.svelte";
  import LibraryScreen from "./screens/LibraryScreen.svelte";
  import SettingsScreen from "./screens/SettingsScreen.svelte";
  import StudyScreen from "./screens/StudyScreen.svelte";
  import TodayScreen from "./screens/TodayScreen.svelte";

  const menuItems: MenuItem[] = [
    { id: "today", label: "Today", shortLabel: "TD" },
    { id: "study", label: "Study", shortLabel: "ST" },
    { id: "library", label: "Library", shortLabel: "LB" },
    { id: "editor", label: "Add / Edit", shortLabel: "AD" },
    { id: "settings", label: "Settings", shortLabel: "SE" },
  ];

  let activeScreen: Screen = "today";
  let theme: ThemeMode = "system";
  let online = true;
  let authoringDirty = false;
  let authoringComposing = false;
  let editingStudyCardId: string | null = null;
  let editingReturnScreen: "study" | "library" = "study";
  let mainElement: HTMLElement;

  onMount(() => {
    const savedTheme = localStorage.getItem("meiki-theme");
    if (isTheme(savedTheme)) theme = savedTheme;
    applyTheme(theme);
    online = navigator.onLine;

    const markOnline = () => (online = true);
    const markOffline = () => (online = false);
    const trackAuthoring = (event: Event) => {
      const detail = (
        event as CustomEvent<{ dirty: boolean; composing: boolean }>
      ).detail;
      authoringDirty = detail.dirty;
      authoringComposing = detail.composing;
    };
    window.addEventListener("online", markOnline);
    window.addEventListener("offline", markOffline);
    window.addEventListener("meiki-authoring-state", trackAuthoring);
    return () => {
      window.removeEventListener("online", markOnline);
      window.removeEventListener("offline", markOffline);
      window.removeEventListener("meiki-authoring-state", trackAuthoring);
    };
  });

  function isTheme(value: string | null): value is ThemeMode {
    return value === "system" || value === "light" || value === "dark";
  }

  function isScreen(value: string): value is Screen {
    return screens.includes(value as Screen);
  }

  function applyTheme(nextTheme: ThemeMode): void {
    theme = nextTheme;
    document.documentElement.dataset.theme = nextTheme;
    localStorage.setItem("meiki-theme", nextTheme);
  }

  async function navigate(value: string): Promise<void> {
    if (!isScreen(value)) return;
    if (activeScreen === "editor" && value !== "editor" && authoringDirty) {
      if (authoringComposing) return;
      if (!window.confirm("Leave the editor and discard unsaved changes?"))
        return;
    }
    authoringDirty = false;
    authoringComposing = false;
    activeScreen = value;
    if (value !== "editor") editingStudyCardId = null;
    await tick();
    mainElement.focus();
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
      if (
        !window.confirm(`Return to ${destination} and discard unsaved changes?`)
      )
        return;
    }
    authoringDirty = false;
    authoringComposing = false;
    activeScreen = editingReturnScreen;
    editingStudyCardId = null;
    await tick();
    mainElement.focus();
  }

  async function finishStudyQueue(): Promise<void> {
    activeScreen = "today";
    editingStudyCardId = null;
    await tick();
    mainElement.focus();
  }
</script>

<svelte:head>
  <title>{messages.appName} · {messages.appTagline}</title>
</svelte:head>

<a class="skip-link" href="#main-content">Skip to content</a>

<div class="app-frame" dir="ltr">
  <header class="app-header">
    <button
      class="brand"
      type="button"
      aria-label="Go to Today"
      onclick={() => navigate("today")}
    >
      <span class="wordmark" lang="ja">{messages.appName}</span>
      <span class="tagline">{messages.appTagline}</span>
    </button>

    <div class="header-actions">
      <span class="local-status">
        <span aria-hidden="true" class="status-dot"></span>
        {messages.localOnly}
      </span>
      <label class="theme-select">
        <span class="visually-hidden">Theme</span>
        <select
          aria-label="Theme"
          value={theme}
          onchange={(event) =>
            applyTheme(event.currentTarget.value as ThemeMode)}
        >
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </label>
    </div>
  </header>

  <div class="shell-body">
    <Menu
      label="Primary navigation"
      items={menuItems}
      active={activeScreen}
      onSelect={navigate}
    />

    <main id="main-content" bind:this={mainElement} tabindex="-1">
      {#if !online}
        <div class="offline-state">
          <Feedback tone="warning" title="You are offline" compact>
            <p>Local creation and study remain available.</p>
          </Feedback>
        </div>
      {/if}

      {#if activeScreen === "today"}
        <TodayScreen
          onStart={() => void navigate("study")}
          onSettings={() => void navigate("settings")}
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

<style>
  .app-frame {
    min-height: 100vh;
    background:
      radial-gradient(
        circle at 15% 0%,
        color-mix(in srgb, var(--color-accent-soft) 58%, transparent),
        transparent 28rem
      ),
      var(--color-canvas);
  }

  .app-header {
    position: sticky;
    z-index: var(--z-header);
    top: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: var(--header-height);
    padding-inline: max(var(--space-6), calc((100% - 82rem) / 2));
    border-bottom: var(--border-width) solid var(--color-border);
    background: color-mix(in srgb, var(--color-canvas) 92%, transparent);
    backdrop-filter: blur(16px);
  }

  .brand {
    display: flex;
    gap: var(--space-3);
    align-items: baseline;
    padding: var(--space-2);
    border: 0;
    color: var(--color-text);
    background: transparent;
    cursor: pointer;
  }

  .wordmark {
    font-family: var(--font-display);
    font-size: var(--text-lg);
    font-weight: 800;
    letter-spacing: 0.08em;
  }

  .tagline,
  .local-status {
    color: var(--color-text-muted);
    font-size: var(--text-xs);
  }

  .header-actions,
  .local-status {
    display: flex;
    gap: var(--space-2);
    align-items: center;
  }

  .header-actions {
    gap: var(--space-4);
  }

  .status-dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
    background: var(--color-success);
    box-shadow: 0 0 0 3px var(--color-success-soft);
  }

  .shell-body {
    display: grid;
    grid-template-columns: 12rem minmax(0, 1fr);
    gap: clamp(var(--space-6), 4vw, var(--space-10));
    width: min(calc(100% - var(--space-8)), 82rem);
    margin-inline: auto;
  }

  main {
    min-width: 0;
    min-height: calc(100vh - var(--header-height));
    padding: var(--space-8) 0 var(--space-10);
    outline: 0;
  }

  .offline-state {
    width: min(100%, var(--content-width));
    margin-bottom: var(--space-5);
  }

  @media (max-width: 760px) {
    .app-header {
      padding-inline: var(--space-3);
    }

    .tagline,
    .local-status {
      display: none;
    }

    .header-actions {
      gap: var(--space-2);
    }

    .shell-body {
      display: block;
      width: min(calc(100% - var(--space-6)), 44rem);
    }

    main {
      padding: var(--space-6) 0 calc(6rem + env(safe-area-inset-bottom));
    }
  }
</style>
