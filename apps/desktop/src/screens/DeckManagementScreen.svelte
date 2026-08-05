<script lang="ts">
  import { onMount } from "svelte";

  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Collapsible from "$lib/components/ui/collapsible/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import type { LibraryBulkActionDto } from "../lib/generated/LibraryBulkActionDto";
  import type { LibraryBulkRequest } from "../lib/generated/LibraryBulkRequest";
  import type { LibraryDueFilterDto } from "../lib/generated/LibraryDueFilterDto";
  import type { LibraryMediaFilterDto } from "../lib/generated/LibraryMediaFilterDto";
  import type { LibraryNoteDto } from "../lib/generated/LibraryNoteDto";
  import type { LibraryOverviewDto } from "../lib/generated/LibraryOverviewDto";
  import type { LibrarySuspendedFilterDto } from "../lib/generated/LibrarySuspendedFilterDto";
  import type { LibraryTrashFilterDto } from "../lib/generated/LibraryTrashFilterDto";

  type Props = {
    selectedDeckId: string;
    deckName: string;
    onBack: () => void;
    onCreate: () => void;
    onEdit: (cardId: string) => void;
  };

  const pageSize = 25;

  let { selectedDeckId, deckName, onBack, onCreate, onEdit }: Props = $props();
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
  let bulkConfirmationOpen = $state(false);
  let bulkConfirmationMessage = $state("");
  let pendingBulkAction = $state<{
    action: LibraryBulkActionDto;
    options: {
      deckId?: string;
      tagId?: string;
      tagName?: string;
    };
  } | null>(null);

  onMount(() => {
    deckId = selectedDeckId;
    void loadDeckCards();
    return () => {
      if (searchTimer) clearTimeout(searchTimer);
    };
  });

  function scheduleSearch(): void {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      offset = 0;
      void loadDeckCards();
    }, 160);
  }

  async function loadDeckCards(): Promise<void> {
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
    void loadDeckCards();
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
    if (options.confirm) {
      bulkConfirmationMessage = options.confirm;
      pendingBulkAction = {
        action,
        options: {
          deckId: options.deckId,
          tagId: options.tagId,
          tagName: options.tagName,
        },
      };
      bulkConfirmationOpen = true;
      return;
    }
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
      await loadDeckCards();
    } catch (reason) {
      error = message(reason);
    } finally {
      busy = false;
    }
  }

  function confirmBulkAction(): void {
    const pending = pendingBulkAction;
    bulkConfirmationOpen = false;
    pendingBulkAction = null;
    if (pending) void runBulk(pending.action, pending.options);
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
      await loadDeckCards();
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
    void loadDeckCards();
  }
</script>

<section
  class="screen deck-management-screen"
  aria-labelledby="deck-management-title"
>
  <header class="screen-header">
    <div>
      <span class="eyebrow">Deck management</span>
      <h1 id="deck-management-title" class="screen-title">{deckName}</h1>
      <p class="screen-description">
        Search and manage this deck without changing review history.
      </p>
    </div>
    <div class="flex flex-wrap gap-2">
      <Button variant="ghost" onclick={onBack}>Back to decks</Button>
      <Button variant="default" data-primary-action onclick={onCreate}
        >Add a source note</Button
      >
    </div>
  </header>

  <Collapsible.Root bind:open={filtersOpen}>
    <div
      class="flex flex-wrap items-end gap-3 rounded-lg border bg-card p-3"
      role="search"
      aria-label="Deck tools"
    >
      <div class="field min-w-60 flex-1">
        <Label for="library-search">Search</Label>
        <Input
          id="library-search"
          type="search"
          bind:value={query}
          placeholder="Source, answer, deck, tag…"
          aria-label="Search library"
          dir="auto"
          oninput={scheduleSearch}
        />
      </div>
      <Collapsible.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            variant={filtersOpen ? "default" : "outline"}
            aria-expanded={filtersOpen}>Filters</Button
          >
        {/snippet}
      </Collapsible.Trigger>
    </div>

    <Collapsible.Content>
      <Card.Root class="mt-4 bg-muted/40 p-4 shadow-none">
        <div class="filter-grid" aria-label="Deck filters">
          <div class="field">
            <Label for="filter-tag">Tag</Label>
            <select id="filter-tag" bind:value={tagId} onchange={applyFilter}>
              <option value="">All tags</option>
              {#each overview?.tags ?? [] as tag (tag.id)}
                <option value={tag.id}>{tag.name}</option>
              {/each}
            </select>
          </div>
          <div class="field">
            <Label for="filter-due">Due state</Label>
            <select id="filter-due" bind:value={due} onchange={applyFilter}>
              <option value="all">Any due state</option>
              <option value="due">Due</option>
              <option value="new">New</option>
              <option value="scheduled">Scheduled later</option>
            </select>
          </div>
          <div class="field">
            <Label for="filter-suspended">Card state</Label>
            <select
              id="filter-suspended"
              bind:value={suspended}
              onchange={applyFilter}
            >
              <option value="all">Active or suspended</option>
              <option value="active">Has active cards</option>
              <option value="suspended">Has suspended cards</option>
            </select>
          </div>
          <div class="field">
            <Label for="filter-language">Language metadata</Label>
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
          </div>
          <div class="field">
            <Label for="filter-media">Media</Label>
            <select id="filter-media" bind:value={media} onchange={applyFilter}>
              <option value="all">With or without media</option>
              <option value="with_media">Has media</option>
              <option value="without_media">No media</option>
            </select>
          </div>
          <div class="field">
            <Label for="filter-trash">Location</Label>
            <select id="filter-trash" bind:value={trash} onchange={applyFilter}>
              <option value="active">Active</option>
              <option value="deleted">Trash</option>
              <option value="all">Active and Trash</option>
            </select>
          </div>
        </div>
      </Card.Root>
    </Collapsible.Content>
  </Collapsible.Root>

  {#if error}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>The deck action was not completed</Alert.Title>
      <Alert.Description>
        <p>{error}</p>
        <Button class="mt-3" variant="outline" onclick={loadDeckCards}
          >Try again</Button
        >
      </Alert.Description>
    </Alert.Root>
  {:else if notice}
    <Alert.Root role="status">
      <Alert.Title>{notice}</Alert.Title>
      {#if undoRequest}<Alert.Action>
          <Button variant="ghost" size="sm" onclick={undoLastAction}
            >Undo</Button
          >
        </Alert.Action>{/if}
    </Alert.Root>
  {/if}

  {#if selectedIds.length}
    <Card.Root class="bg-muted/40 p-4 shadow-none">
      <div class="bulk-panel" aria-label="Selected note actions">
        <strong>
          {selectedIds.length}
          {selectedIds.length === 1 ? "note" : "notes"} selected
        </strong>
        <div class="bulk-actions">
          <Button
            size="sm"
            variant="outline"
            disabled={busy}
            onclick={() => runBulk("suspend")}>Suspend</Button
          >
          <Button
            size="sm"
            variant="outline"
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
            size="sm"
            variant="outline"
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
            size="sm"
            variant="outline"
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
              size="sm"
              variant="outline"
              disabled={busy || !removeTagId}
              onclick={() => runBulk("remove_tag", { tagId: removeTagId })}
              >Remove tag</Button
            >
          {/if}
          {#if trash === "deleted"}
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onclick={() => runBulk("restore")}>Restore</Button
            >
          {:else if trash === "active"}
            <Button
              size="sm"
              variant="destructive"
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
    </Card.Root>
  {/if}

  <Card.Root class="overflow-hidden p-0">
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
                  <Badge>Due</Badge>
                {:else if note.cards.some((card) => card.is_new)}
                  <Badge variant="secondary">New</Badge>
                {/if}
                {#if note.cards.every((card) => card.suspended)}
                  <Badge variant="outline">Suspended</Badge>
                {/if}
                {#if note.media_count}<span>{note.media_count} media</span>{/if}
                {#if note.deleted}<Badge variant="destructive">Trash</Badge
                  >{/if}
              </div>
              {#if note.tags.length}
                <div class="tags" aria-label="Tags">
                  {#each note.tags as tag (tag.id)}
                    <Badge variant="outline">{tag.name}</Badge>
                  {/each}
                </div>
              {/if}
            </div>
            <div class="note-actions">
              <Button
                size="sm"
                variant="ghost"
                onclick={() => (previewNote = note)}>Preview</Button
              >
              {#if !note.deleted && note.cards[0]}
                <Button
                  size="sm"
                  variant="ghost"
                  onclick={() => onEdit(note.cards[0].card_id)}>Edit</Button
                >
              {/if}
            </div>
          </li>
        {/each}
      </ul>
      <div class="pagination">
        <Button
          size="sm"
          variant="outline"
          disabled={offset === 0 || loading}
          onclick={() => changePage(offset - pageSize)}>Previous</Button
        >
        <span>
          {offset + 1}–{offset + overview.notes.length} of {overview.total_matches}
        </span>
        <Button
          size="sm"
          variant="outline"
          disabled={!hasNextPage() || loading}
          onclick={() => changePage(offset + pageSize)}>Next</Button
        >
      </div>
    {:else}
      <div class="empty-state">
        <span class="empty-mark" aria-hidden="true">＋</span>
        <h2>{query ? "No matching notes" : "Your deck is ready"}</h2>
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
        <Button variant="outline" onclick={onCreate}>Add a source note</Button>
      </div>
    {/if}
  </Card.Root>
</section>

<Dialog.Root
  open={Boolean(previewNote)}
  onOpenChange={(open) => {
    if (!open) previewNote = null;
  }}
>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Generated cards</Dialog.Title>
      <Dialog.Description>
        {previewNote?.cards.length ?? 0}
        {previewNote?.cards.length === 1 ? "card" : "cards"} from this source note
      </Dialog.Description>
    </Dialog.Header>
    {#if previewNote}
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
    {/if}
  </Dialog.Content>
</Dialog.Root>

<AlertDialog.Root bind:open={bulkConfirmationOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Move selected notes to Trash?</AlertDialog.Title>
      <AlertDialog.Description>
        {bulkConfirmationMessage}
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        class="bg-destructive/10 text-destructive hover:bg-destructive/20"
        onclick={confirmBulkAction}>Move to Trash</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>

<style>
  .deck-management-screen {
    width: min(100%, 76rem);
  }

  .filter-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
    gap: 0.75rem;
  }

  select,
  .tag-entry input {
    min-height: 2.75rem;
    padding-inline: 0.75rem;
    border: 1px solid var(--input);
    border-radius: var(--radius-lg);
    color: var(--foreground);
    background: var(--card);
    font: inherit;
  }

  .bulk-panel {
    display: grid;
    gap: 0.75rem;
  }

  .bulk-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }

  .bulk-actions select,
  .bulk-actions input {
    min-height: 2rem;
  }

  .results-heading,
  .pagination {
    display: flex;
    gap: 1rem;
    align-items: center;
    justify-content: space-between;
    padding: 1rem;
    color: var(--muted-foreground);
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
    gap: 0.75rem;
    align-items: start;
    padding: 1rem;
    border-top: 1px solid var(--border);
  }

  .note-list li.deleted {
    background: var(--muted);
    opacity: 0.82;
  }

  .note-select {
    padding-top: 0.25rem;
  }

  .note-main {
    display: grid;
    min-width: 0;
    gap: 0.5rem;
  }

  .source-text {
    overflow-wrap: anywhere;
    font-family: var(--font-content);
    font-size: 1rem;
    line-height: 1.5;
  }

  .note-meta,
  .tags,
  .note-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }

  .note-meta {
    color: var(--muted-foreground);
    font-size: var(--text-xs);
  }

  .note-meta span + span::before {
    margin-right: 0.5rem;
    content: "·";
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
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    color: var(--primary);
    background: var(--accent);
    font-size: var(--text-xl);
    place-items: center;
  }

  .empty-state h2 {
    margin: 1rem 0 0.5rem;
    font-family: var(--font-sans);
    font-size: var(--text-xl);
  }

  .empty-state p {
    max-width: 34rem;
    margin: 0 0 1.5rem;
    color: var(--muted-foreground);
    line-height: 1.6;
  }

  .preview-list {
    display: grid;
    gap: 0.75rem;
  }

  .preview-list article {
    display: grid;
    gap: 0.5rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  .preview-list p {
    margin: 0;
    overflow-wrap: anywhere;
    font-family: var(--font-content);
    line-height: 1.6;
  }

  .preview-list small {
    color: var(--muted-foreground);
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
