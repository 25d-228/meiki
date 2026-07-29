<script lang="ts">
  import { onMount } from "svelte";

  import { api } from "../lib/api";
  import Button from "../lib/components/Button.svelte";
  import Dialog from "../lib/components/Dialog.svelte";
  import Feedback from "../lib/components/Feedback.svelte";
  import Field from "../lib/components/Field.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";
  import TextInput from "../lib/components/TextInput.svelte";
  import Toolbar from "../lib/components/Toolbar.svelte";
  import type { LibraryBulkActionDto } from "../lib/generated/LibraryBulkActionDto";
  import type { LibraryBulkRequest } from "../lib/generated/LibraryBulkRequest";
  import type { LibraryDueFilterDto } from "../lib/generated/LibraryDueFilterDto";
  import type { LibraryMediaFilterDto } from "../lib/generated/LibraryMediaFilterDto";
  import type { LibraryNoteDto } from "../lib/generated/LibraryNoteDto";
  import type { LibraryOverviewDto } from "../lib/generated/LibraryOverviewDto";
  import type { LibrarySuspendedFilterDto } from "../lib/generated/LibrarySuspendedFilterDto";
  import type { LibraryTrashFilterDto } from "../lib/generated/LibraryTrashFilterDto";

  type Props = {
    onNavigate: (screen: string) => void;
    onEdit: (cardId: string) => void;
  };

  const pageSize = 25;

  let { onNavigate, onEdit }: Props = $props();
  let query = $state("");
  let deckId = $state("");
  let tagId = $state("");
  let due = $state<LibraryDueFilterDto>("all");
  let suspended = $state<LibrarySuspendedFilterDto>("all");
  let languageTag = $state("");
  let media = $state<LibraryMediaFilterDto>("all");
  let trash = $state<LibraryTrashFilterDto>("active");
  let filtersOpen = $state(false);
  let overview = $state<LibraryOverviewDto | null>(null);
  let selectedIds = $state<string[]>([]);
  let previewNote = $state<LibraryNoteDto | null>(null);
  let destinationDeckId = $state("");
  let tagName = $state("");
  let removeTagId = $state("");
  let offset = $state(0);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state("");
  let notice = $state("");
  let undoRequest = $state<LibraryBulkRequest | null>(null);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    void loadLibrary();
    return () => {
      if (searchTimer) clearTimeout(searchTimer);
    };
  });

  function scheduleSearch(): void {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      offset = 0;
      void loadLibrary();
    }, 160);
  }

  async function loadLibrary(): Promise<void> {
    loading = true;
    error = "";
    try {
      overview = await api.getLibrary({
        query,
        deck_id: deckId || null,
        tag_id: tagId || null,
        due,
        suspended,
        language_tag: languageTag || null,
        media,
        trash,
        now_ms: Date.now(),
        offset,
        limit: pageSize,
      });
      selectedIds = selectedIds.filter((id) =>
        overview?.notes.some((note) => note.source_id === id),
      );
      destinationDeckId ||= overview.decks[0]?.id ?? "";
      removeTagId ||= overview.tags[0]?.id ?? "";
    } catch (reason) {
      error = message(reason);
    } finally {
      loading = false;
    }
  }

  function applyFilter(): void {
    offset = 0;
    void loadLibrary();
  }

  function toggleSelection(sourceId: string): void {
    selectedIds = selectedIds.includes(sourceId)
      ? selectedIds.filter((id) => id !== sourceId)
      : [...selectedIds, sourceId];
  }

  function togglePageSelection(): void {
    const pageIds = overview?.notes.map((note) => note.source_id) ?? [];
    const allSelected =
      pageIds.length > 0 && pageIds.every((id) => selectedIds.includes(id));
    selectedIds = allSelected
      ? selectedIds.filter((id) => !pageIds.includes(id))
      : [...new Set([...selectedIds, ...pageIds])];
  }

  async function runBulk(
    action: LibraryBulkActionDto,
    options: {
      deckId?: string;
      tagId?: string;
      tagName?: string;
      confirm?: string;
    } = {},
  ): Promise<void> {
    if (!selectedIds.length || busy) return;
    if (options.confirm && !window.confirm(options.confirm)) return;
    const request: LibraryBulkRequest = {
      source_ids: selectedIds,
      action,
      deck_id: options.deckId ?? null,
      tag_id: options.tagId ?? null,
      tag_name: options.tagName ?? null,
      now_ms: Date.now(),
    };
    busy = true;
    error = "";
    notice = "";
    try {
      const result = await api.applyLibraryBulkAction(request);
      notice = actionNotice(action, result.affected_notes);
      undoRequest = result.undo_action
        ? {
            ...request,
            action: result.undo_action,
            now_ms: Date.now(),
          }
        : null;
      selectedIds = [];
      tagName = "";
      await loadLibrary();
    } catch (reason) {
      error = message(reason);
    } finally {
      busy = false;
    }
  }

  async function undoLastAction(): Promise<void> {
    if (!undoRequest || busy) return;
    const request = { ...undoRequest, now_ms: Date.now() };
    busy = true;
    error = "";
    try {
      const result = await api.applyLibraryBulkAction(request);
      notice = `Undid the last action for ${result.affected_notes} ${
        result.affected_notes === 1 ? "note" : "notes"
      }.`;
      undoRequest = null;
      await loadLibrary();
    } catch (reason) {
      error = message(reason);
    } finally {
      busy = false;
    }
  }

  async function exportSelection(): Promise<void> {
    if (!selectedIds.length || busy) return;
    busy = true;
    error = "";
    try {
      const result = await api.exportLibrarySelection({
        source_ids: selectedIds,
        now_ms: Date.now(),
      });
      notice = `Exported ${result.exported_notes} ${
        result.exported_notes === 1 ? "note" : "notes"
      } to ${result.path}`;
    } catch (reason) {
      error = message(reason);
    } finally {
      busy = false;
    }
  }

  async function exportPortableSelection(): Promise<void> {
    if (!selectedIds.length || busy) return;
    busy = true;
    error = "";
    try {
      const result = await api.exportArchive({
        scope: "selected_notes",
        selected_ids: selectedIds,
        now_ms: Date.now(),
      });
      notice = `Exported ${result.notes} ${
        result.notes === 1 ? "note" : "notes"
      } with complete history to ${result.path}`;
    } catch (reason) {
      error = message(reason);
    } finally {
      busy = false;
    }
  }

  function actionNotice(action: LibraryBulkActionDto, count: number): string {
    const noun = count === 1 ? "note" : "notes";
    if (action === "delete") return `Moved ${count} ${noun} to Trash.`;
    if (action === "restore") return `Restored ${count} ${noun}.`;
    if (action === "suspend") return `Suspended cards in ${count} ${noun}.`;
    if (action === "unsuspend") return `Unsuspended cards in ${count} ${noun}.`;
    if (action === "move") return `Moved ${count} ${noun}.`;
    if (action === "add_tag") return `Tagged ${count} ${noun}.`;
    return `Removed the tag from ${count} ${noun}.`;
  }

  function message(reason: unknown): string {
    return reason instanceof Error ? reason.message : String(reason);
  }

  function hasNextPage(): boolean {
    return Boolean(
      overview && offset + overview.notes.length < overview.total_matches,
    );
  }

  function changePage(nextOffset: number): void {
    offset = Math.max(0, nextOffset);
    selectedIds = [];
    void loadLibrary();
  }
</script>

<section class="screen library-screen" aria-labelledby="library-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Your collection</span>
      <h1 id="library-title" class="screen-title">Library</h1>
      <p class="screen-description">
        Search every script and manage selected notes without changing review
        history.
      </p>
    </div>
    <Button
      variant="primary"
      data-primary-action
      onclick={() => onNavigate("editor")}>Add a source note</Button
    >
  </header>

  <Toolbar label="Library tools">
    <div class="toolbar-grow">
      <Field id="library-search" label="Search">
        <TextInput
          id="library-search"
          type="search"
          bind:value={query}
          placeholder="Source, answer, deck, tag…"
          aria-label="Search library"
          dir="auto"
          oninput={scheduleSearch}
        />
      </Field>
    </div>
    <Button
      variant={filtersOpen ? "primary" : "secondary"}
      aria-expanded={filtersOpen}
      onclick={() => (filtersOpen = !filtersOpen)}>Filters</Button
    >
  </Toolbar>

  {#if filtersOpen}
    <SurfaceCard padding="compact" tone="quiet">
      <div class="filter-grid" aria-label="Library filters">
        <Field id="filter-deck" label="Deck">
          <select id="filter-deck" bind:value={deckId} onchange={applyFilter}>
            <option value="">All decks</option>
            {#each overview?.decks ?? [] as deck (deck.id)}
              <option value={deck.id}>{deck.name}</option>
            {/each}
          </select>
        </Field>
        <Field id="filter-tag" label="Tag">
          <select id="filter-tag" bind:value={tagId} onchange={applyFilter}>
            <option value="">All tags</option>
            {#each overview?.tags ?? [] as tag (tag.id)}
              <option value={tag.id}>{tag.name}</option>
            {/each}
          </select>
        </Field>
        <Field id="filter-due" label="Due state">
          <select id="filter-due" bind:value={due} onchange={applyFilter}>
            <option value="all">Any due state</option>
            <option value="due">Due</option>
            <option value="new">New</option>
            <option value="scheduled">Scheduled later</option>
          </select>
        </Field>
        <Field id="filter-suspended" label="Card state">
          <select
            id="filter-suspended"
            bind:value={suspended}
            onchange={applyFilter}
          >
            <option value="all">Active or suspended</option>
            <option value="active">Has active cards</option>
            <option value="suspended">Has suspended cards</option>
          </select>
        </Field>
        <Field id="filter-language" label="Language metadata">
          <select
            id="filter-language"
            bind:value={languageTag}
            onchange={applyFilter}
          >
            <option value="">Any or unknown</option>
            {#each overview?.languages ?? [] as language (language)}
              <option value={language}>{language}</option>
            {/each}
          </select>
        </Field>
        <Field id="filter-media" label="Media">
          <select id="filter-media" bind:value={media} onchange={applyFilter}>
            <option value="all">With or without media</option>
            <option value="with_media">Has media</option>
            <option value="without_media">No media</option>
          </select>
        </Field>
        <Field id="filter-trash" label="Location">
          <select id="filter-trash" bind:value={trash} onchange={applyFilter}>
            <option value="active">Library</option>
            <option value="deleted">Trash</option>
            <option value="all">Library and Trash</option>
          </select>
        </Field>
      </div>
    </SurfaceCard>
  {/if}

  {#if error}
    <Feedback tone="error" title="The Library action was not completed">
      <p>{error}</p>
      <Button variant="secondary" onclick={loadLibrary}>Try again</Button>
    </Feedback>
  {:else if notice}
    <Feedback tone="success" title={notice} compact>
      {#if undoRequest}
        <Button variant="quiet" size="small" onclick={undoLastAction}
          >Undo</Button
        >
      {/if}
    </Feedback>
  {/if}

  {#if selectedIds.length}
    <SurfaceCard padding="compact" tone="quiet">
      <div class="bulk-panel" aria-label="Selected note actions">
        <strong>
          {selectedIds.length}
          {selectedIds.length === 1 ? "note" : "notes"} selected
        </strong>
        <div class="bulk-actions">
          <Button
            size="small"
            variant="secondary"
            disabled={busy}
            onclick={() => runBulk("suspend")}>Suspend</Button
          >
          <Button
            size="small"
            variant="secondary"
            disabled={busy}
            onclick={() => runBulk("unsuspend")}>Unsuspend</Button
          >
          <label>
            <span class="visually-hidden">Destination deck</span>
            <select
              bind:value={destinationDeckId}
              aria-label="Destination deck"
            >
              {#each overview?.decks ?? [] as deck (deck.id)}
                <option value={deck.id}>{deck.name}</option>
              {/each}
            </select>
          </label>
          <Button
            size="small"
            variant="secondary"
            disabled={busy || !destinationDeckId}
            onclick={() => runBulk("move", { deckId: destinationDeckId })}
            >Move</Button
          >
          <label class="tag-entry">
            <span class="visually-hidden">Tag name</span>
            <input
              bind:value={tagName}
              aria-label="Tag name"
              placeholder="Tag"
            />
          </label>
          <Button
            size="small"
            variant="secondary"
            disabled={busy || !tagName.trim()}
            onclick={() => runBulk("add_tag", { tagName })}>Add tag</Button
          >
          {#if overview?.tags.length}
            <label>
              <span class="visually-hidden">Tag to remove</span>
              <select bind:value={removeTagId} aria-label="Tag to remove">
                {#each overview.tags as tag (tag.id)}
                  <option value={tag.id}>{tag.name}</option>
                {/each}
              </select>
            </label>
            <Button
              size="small"
              variant="secondary"
              disabled={busy || !removeTagId}
              onclick={() => runBulk("remove_tag", { tagId: removeTagId })}
              >Remove tag</Button
            >
          {/if}
          <Button
            size="small"
            variant="secondary"
            disabled={busy}
            onclick={exportSelection}>Export</Button
          >
          <Button
            size="small"
            variant="secondary"
            disabled={busy}
            onclick={exportPortableSelection}>Create .meiki archive</Button
          >
          {#if trash === "deleted"}
            <Button
              size="small"
              variant="secondary"
              disabled={busy}
              onclick={() => runBulk("restore")}>Restore</Button
            >
          {:else if trash === "active"}
            <Button
              size="small"
              variant="danger"
              disabled={busy}
              onclick={() =>
                runBulk("delete", {
                  confirm: `Move ${selectedIds.length} selected ${
                    selectedIds.length === 1 ? "note" : "notes"
                  } to Trash? Review history and media stay intact, and the selection can be restored.`,
                })}>Move to Trash</Button
            >
          {/if}
        </div>
      </div>
    </SurfaceCard>
  {/if}

  <SurfaceCard padding="none">
    {#if loading && !overview}
      <div class="state-card" aria-live="polite" aria-busy="true">
        <span class="spinner" aria-hidden="true"></span>
        <p>Searching your local collection…</p>
      </div>
    {:else if overview?.notes.length}
      <div class="results-heading">
        <label>
          <input
            type="checkbox"
            checked={overview.notes.every((note) =>
              selectedIds.includes(note.source_id),
            )}
            onchange={togglePageSelection}
          />
          Select this page
        </label>
        <span>{overview.total_matches} matching notes</span>
      </div>
      <ul class="note-list">
        {#each overview.notes as note (note.source_id)}
          <li class:deleted={note.deleted}>
            <label class="note-select">
              <input
                type="checkbox"
                aria-label={`Select ${note.source_text}`}
                checked={selectedIds.includes(note.source_id)}
                onchange={() => toggleSelection(note.source_id)}
              />
            </label>
            <div class="note-main">
              <bdi
                class="source-text"
                lang={note.language_tag ?? undefined}
                dir={note.direction}>{note.source_text}</bdi
              >
              <div class="note-meta">
                <span>{note.deck_name}</span>
                <span>{note.cards.length} cards</span>
                {#if note.cards.some((card) => card.is_due)}
                  <span class="due">Due</span>
                {:else if note.cards.some((card) => card.is_new)}
                  <span>New</span>
                {/if}
                {#if note.cards.every((card) => card.suspended)}
                  <span>Suspended</span>
                {/if}
                {#if note.media_count}<span>{note.media_count} media</span>{/if}
                {#if note.deleted}<span>Trash</span>{/if}
              </div>
              {#if note.tags.length}
                <div class="tags" aria-label="Tags">
                  {#each note.tags as tag (tag.id)}
                    <span>{tag.name}</span>
                  {/each}
                </div>
              {/if}
            </div>
            <div class="note-actions">
              <Button
                size="small"
                variant="quiet"
                onclick={() => (previewNote = note)}>Preview</Button
              >
              {#if !note.deleted && note.cards[0]}
                <Button
                  size="small"
                  variant="quiet"
                  onclick={() => onEdit(note.cards[0].card_id)}>Edit</Button
                >
              {/if}
            </div>
          </li>
        {/each}
      </ul>
      <div class="pagination">
        <Button
          size="small"
          variant="secondary"
          disabled={offset === 0 || loading}
          onclick={() => changePage(offset - pageSize)}>Previous</Button
        >
        <span>
          {offset + 1}–{offset + overview.notes.length} of {overview.total_matches}
        </span>
        <Button
          size="small"
          variant="secondary"
          disabled={!hasNextPage() || loading}
          onclick={() => changePage(offset + pageSize)}>Next</Button
        >
      </div>
    {:else}
      <div class="empty-state">
        <span class="empty-mark" aria-hidden="true">＋</span>
        <h2>{query ? "No matching notes" : "Your library is ready"}</h2>
        <p>
          {#if trash === "deleted"}
            Trash is empty. Deleted notes stay recoverable here.
          {:else if query}
            Nothing matches “{query}”. Try another script or clear a filter.
          {:else}
            Create a source note and turn one or more semantic spans into
            clozes.
          {/if}
        </p>
        <Button variant="secondary" onclick={() => onNavigate("editor")}
          >Add a source note</Button
        >
      </div>
    {/if}
  </SurfaceCard>
</section>

{#if previewNote}
  <Dialog
    open
    title="Generated cards"
    description={`${previewNote.cards.length} ${
      previewNote.cards.length === 1 ? "card" : "cards"
    } from this source note`}
    onClose={() => (previewNote = null)}
  >
    <div class="preview-list">
      {#each previewNote.cards as card (card.card_id)}
        <article>
          <p lang={card.language_tag ?? undefined} dir={card.direction}>
            {card.prompt}
          </p>
          <strong dir="auto">{card.answer}</strong>
          <small>
            {card.suspended
              ? "Suspended"
              : card.is_due
                ? "Due"
                : card.is_new
                  ? "New"
                  : "Scheduled"}
          </small>
        </article>
      {/each}
    </div>
  </Dialog>
{/if}

<style>
  .library-screen {
    width: min(100%, 76rem);
  }

  .filter-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
    gap: var(--space-3);
  }

  select,
  .tag-entry input {
    min-height: var(--control-height);
    padding-inline: var(--space-3);
    border: var(--border-width) solid var(--color-border-strong);
    border-radius: var(--radius-control);
    color: var(--color-text);
    background: var(--color-surface);
    font: inherit;
  }

  .bulk-panel {
    display: grid;
    gap: var(--space-3);
  }

  .bulk-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
  }

  .bulk-actions select,
  .bulk-actions input {
    min-height: 2rem;
  }

  .results-heading,
  .pagination {
    display: flex;
    gap: var(--space-4);
    align-items: center;
    justify-content: space-between;
    padding: var(--space-4);
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .note-list {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .note-list li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: start;
    padding: var(--space-4);
    border-top: var(--border-width) solid var(--color-border);
  }

  .note-list li.deleted {
    background: var(--color-surface-muted);
    opacity: 0.82;
  }

  .note-select {
    padding-top: var(--space-1);
  }

  .note-main {
    display: grid;
    min-width: 0;
    gap: var(--space-2);
  }

  .source-text {
    overflow-wrap: anywhere;
    font-family: var(--font-content);
    font-size: var(--text-md);
    line-height: 1.5;
  }

  .note-meta,
  .tags,
  .note-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
  }

  .note-meta {
    color: var(--color-text-muted);
    font-size: var(--text-xs);
  }

  .note-meta span + span::before {
    margin-right: var(--space-2);
    content: "·";
  }

  .note-meta .due {
    color: var(--color-warning);
    font-weight: 700;
  }

  .tags span {
    padding: 0.15rem var(--space-2);
    border-radius: 999px;
    color: var(--color-info);
    background: var(--color-info-soft);
    font-size: var(--text-xs);
  }

  .empty-state,
  .state-card {
    display: grid;
    justify-items: center;
    min-height: 20rem;
    text-align: center;
    place-content: center;
  }

  .empty-mark {
    display: inline-grid;
    width: 3rem;
    height: 3rem;
    border: var(--border-width) solid var(--color-accent-border);
    border-radius: 50%;
    color: var(--color-accent);
    background: var(--color-accent-soft);
    font-size: var(--text-xl);
    place-items: center;
  }

  .empty-state h2 {
    margin: var(--space-4) 0 var(--space-2);
    font-family: var(--font-display);
    font-size: var(--text-xl);
  }

  .empty-state p {
    max-width: 34rem;
    margin: 0 0 var(--space-6);
    color: var(--color-text-muted);
    line-height: 1.6;
  }

  .preview-list {
    display: grid;
    gap: var(--space-3);
  }

  .preview-list article {
    display: grid;
    gap: var(--space-2);
    padding: var(--space-4);
    border: var(--border-width) solid var(--color-border);
    border-radius: var(--radius-control);
  }

  .preview-list p {
    margin: 0;
    overflow-wrap: anywhere;
    font-family: var(--font-content);
    line-height: 1.6;
  }

  .preview-list small {
    color: var(--color-text-muted);
  }

  @media (max-width: 44rem) {
    .note-list li {
      grid-template-columns: auto minmax(0, 1fr);
    }

    .note-actions {
      grid-column: 2;
    }

    .pagination {
      flex-wrap: wrap;
    }
  }
</style>
