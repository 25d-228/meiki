<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";

  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Collapsible from "$lib/components/ui/collapsible/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";
  import LimitedMarkdown from "../lib/components/LimitedMarkdown.svelte";
  import MediaFrame from "../lib/components/MediaFrame.svelte";
  import type { AnnotationDraftDto } from "../lib/generated/AnnotationDraftDto";
  import type { AuthoringClozeDto } from "../lib/generated/AuthoringClozeDto";
  import type { AuthoringDraftDto } from "../lib/generated/AuthoringDraftDto";
  import type { AuthoringPreviewDto } from "../lib/generated/AuthoringPreviewDto";
  import type { DirectionDto } from "../lib/generated/DirectionDto";
  import type { DeckDto } from "../lib/generated/DeckDto";
  import type { MatchingPolicyDto } from "../lib/generated/MatchingPolicyDto";
  import type { MediaRoleDto } from "../lib/generated/MediaRoleDto";
  import { mediaAssetSource } from "../lib/media";

  type Props = {
    cardId?: string | null;
    preferredDeckId?: string;
    onReturn?: () => void;
    onSaved?: () => void | Promise<void>;
    returnLabel?: string;
  };

  let {
    cardId = null,
    preferredDeckId,
    onReturn,
    onSaved,
    returnLabel = "Return to study",
  }: Props = $props();

  type Selection = {
    segmentId: string;
    start: number;
    end: number;
  };

  let draft = $state<AuthoringDraftDto | null>(null);
  let decks = $state<DeckDto[]>([]);
  let selection = $state<Selection | null>(null);
  let previews = $state<AuthoringPreviewDto[]>([]);
  let previewIndex = $state(0);
  let previewOpen = $state(false);
  let dirty = $state(false);
  let composing = $state(false);
  let busy = $state(false);
  let error = $state("");
  let savedMessage = $state("");
  let confirmationOpen = $state(false);
  let confirmationKind = $state<"new" | "remove-cloze" | null>(null);
  let confirmationDescription = $state("");
  let previewTrigger = $state<HTMLElement | null>(null);

  const shortcut = navigator.platform.includes("Mac") ? "⌘" : "Ctrl";

  onMount(() => {
    void initialize();
  });

  async function initialize(): Promise<void> {
    try {
      decks = await api.listDecks();
      if (cardId) {
        await loadCardDraft(cardId);
      } else {
        await startNew(false);
      }
    } catch (reason) {
      reportFailure(reason);
    }
  }

  onDestroy(() => {
    publishAuthoringState(false, false);
  });

  $effect(() => {
    publishAuthoringState(dirty, composing);
  });

  function publishAuthoringState(isDirty: boolean, isComposing: boolean) {
    window.dispatchEvent(
      new CustomEvent("meiki-authoring-state", {
        detail: { dirty: isDirty, composing: isComposing },
      }),
    );
  }

  function activeCloze(): AuthoringClozeDto | undefined {
    return draft?.clozes.find((cloze) => cloze.id === draft?.active_cloze_id);
  }

  function activePreview(): AuthoringPreviewDto | undefined {
    return previews[previewIndex];
  }

  function displayDirection(direction: DirectionDto): "auto" | "ltr" | "rtl" {
    return direction;
  }

  function reportFailure(reason: unknown): void {
    error = reason instanceof Error ? reason.message : String(reason);
    savedMessage = "";
  }

  function changed(): void {
    dirty = true;
    error = "";
    savedMessage = "";
  }

  function chooseDeck(deckId: string): void {
    const deck = decks.find((candidate) => candidate.id === deckId);
    if (!draft || !deck) return;
    draft = {
      ...draft,
      deck_id: deck.id,
      deck_language_tag: deck.language_tag,
      deck_direction: deck.direction,
      deck_matching_policy: deck.matching_policy,
    };
    changed();
  }

  async function startNew(protect = true): Promise<void> {
    if (composing) return;
    if (protect && dirty) {
      confirmationKind = "new";
      confirmationDescription = "Discard the unsaved card and start a new one?";
      confirmationOpen = true;
      return;
    }
    busy = true;
    try {
      const nextDraft = await api.newAuthoringDraft();
      const preferredDeck = decks.find((deck) => deck.id === preferredDeckId);
      draft = preferredDeck
        ? {
            ...nextDraft,
            deck_id: preferredDeck.id,
            deck_language_tag: preferredDeck.language_tag,
            deck_direction: preferredDeck.direction,
            deck_matching_policy: preferredDeck.matching_policy,
          }
        : nextDraft;
      selection = null;
      previews = [];
      previewOpen = false;
      dirty = false;
      error = "";
      savedMessage = "";
    } catch (reason) {
      reportFailure(reason);
    } finally {
      busy = false;
    }
  }

  async function loadCardDraft(activeCardId: string): Promise<void> {
    busy = true;
    try {
      draft = await api.getAuthoringDraftForCard(activeCardId);
      selection = null;
      previews = [];
      previewOpen = false;
      dirty = false;
      error = "";
      savedMessage = "";
    } catch (reason) {
      reportFailure(reason);
    } finally {
      busy = false;
    }
  }

  function updateSourceLanguage(value: string): void {
    if (!draft) return;
    draft = { ...draft, language_tag: value.trim() || null };
    changed();
  }

  function updateSourceDirection(value: string): void {
    if (!draft) return;
    draft = { ...draft, direction: value as DirectionDto };
    changed();
  }

  function updateSegmentText(segmentId: string, text: string): void {
    if (!draft) return;
    draft = {
      ...draft,
      segments: draft.segments.map((segment) =>
        segment.id === segmentId ? { ...segment, text } : segment,
      ),
    };
    changed();
  }

  function rememberSelection(event: Event, segmentId: string): void {
    const textarea = event.currentTarget as HTMLTextAreaElement;
    selection = {
      segmentId,
      start: textarea.selectionStart,
      end: textarea.selectionEnd,
    };
  }

  async function makeCloze(): Promise<void> {
    if (!draft || !selection || selection.start === selection.end || composing)
      return;
    busy = true;
    try {
      draft = await api.makeCloze({
        draft,
        segment_id: selection.segmentId,
        selection_start_utf16: selection.start,
        selection_end_utf16: selection.end,
      });
      selection = null;
      changed();
    } catch (reason) {
      reportFailure(reason);
    } finally {
      busy = false;
    }
  }

  function selectCloze(clozeId: string): void {
    if (!draft) return;
    draft = { ...draft, active_cloze_id: clozeId };
  }

  function updateActiveCloze(
    update: (cloze: AuthoringClozeDto) => AuthoringClozeDto,
  ): void {
    if (!draft?.active_cloze_id) return;
    const activeId = draft.active_cloze_id;
    const current = draft.clozes.find((cloze) => cloze.id === activeId);
    if (!current) return;
    const next = update(current);
    draft = {
      ...draft,
      clozes: draft.clozes.map((cloze) =>
        cloze.id === activeId ? next : cloze,
      ),
      segments: draft.segments.map((segment) =>
        segment.cloze_id === activeId
          ? { ...segment, text: next.answer }
          : segment,
      ),
    };
    changed();
  }

  function addAnnotation(): void {
    updateActiveCloze((cloze) => ({
      ...cloze,
      annotations: [
        ...cloze.annotations,
        {
          id: crypto.randomUUID(),
          label: "",
          value: "",
          language_tag: cloze.language_tag,
          direction: cloze.direction,
        },
      ],
    }));
  }

  function updateAnnotation(
    annotationId: string,
    update: (annotation: AnnotationDraftDto) => AnnotationDraftDto,
  ): void {
    updateActiveCloze((cloze) => ({
      ...cloze,
      annotations: cloze.annotations.map((annotation) =>
        annotation.id === annotationId ? update(annotation) : annotation,
      ),
    }));
  }

  function moveAnnotation(annotationId: string, offset: number): void {
    updateActiveCloze((cloze) => {
      const annotations = [...cloze.annotations];
      const index = annotations.findIndex(
        (annotation) => annotation.id === annotationId,
      );
      const next = index + offset;
      if (index < 0 || next < 0 || next >= annotations.length) return cloze;
      [annotations[index], annotations[next]] = [
        annotations[next],
        annotations[index],
      ];
      return { ...cloze, annotations };
    });
  }

  function removeAnnotation(annotationId: string): void {
    updateActiveCloze((cloze) => ({
      ...cloze,
      annotations: cloze.annotations.filter(
        (annotation) => annotation.id !== annotationId,
      ),
    }));
  }

  async function attachMedia(role: MediaRoleDto): Promise<void> {
    const cloze = activeCloze();
    if (!cloze || busy) return;
    busy = true;
    try {
      const path = await api.pickMediaFile(role);
      if (!path) return;
      const media = await api.importMedia(
        path,
        role,
        cloze.language_tag ?? draft?.language_tag ?? null,
        cloze.direction,
      );
      updateActiveCloze((value) => ({
        ...value,
        media: [...value.media, media],
      }));
    } catch (reason) {
      reportFailure(reason);
    } finally {
      busy = false;
    }
  }

  function updateMediaAlt(mediaId: string, altText: string): void {
    updateActiveCloze((cloze) => ({
      ...cloze,
      media: cloze.media.map((media) =>
        media.id === mediaId
          ? { ...media, alt_text: altText.trim() || null }
          : media,
      ),
    }));
  }

  function removeMedia(mediaId: string): void {
    updateActiveCloze((cloze) => ({
      ...cloze,
      media: cloze.media.filter((media) => media.id !== mediaId),
    }));
  }

  function mediaRoleLabel(role: MediaRoleDto): string {
    if (role === "prompt_audio") return "Prompt audio";
    if (role === "answer_audio") return "Answer audio";
    return "Reveal image";
  }

  async function removeActiveCloze(): Promise<void> {
    if (!draft?.active_cloze_id || composing) return;
    if (draft.persisted) {
      confirmationKind = "remove-cloze";
      confirmationDescription =
        "Saving will remove this card. Existing review history prevents unsafe deletion and will be reported.";
      confirmationOpen = true;
      return;
    }
    await performRemoveActiveCloze(false);
  }

  async function performRemoveActiveCloze(confirmed: boolean): Promise<void> {
    if (!draft?.active_cloze_id) return;
    busy = true;
    try {
      draft = await api.removeCloze({
        draft,
        cloze_id: draft.active_cloze_id,
        confirm_card_deletion: confirmed,
      });
      changed();
    } catch (reason) {
      reportFailure(reason);
    } finally {
      busy = false;
    }
  }

  async function confirmEditorAction(): Promise<void> {
    const action = confirmationKind;
    confirmationOpen = false;
    confirmationKind = null;
    if (action === "new") await startNew(false);
    if (action === "remove-cloze") await performRemoveActiveCloze(true);
  }

  async function moveSegment(segmentId: string, offset: number): Promise<void> {
    if (!draft || composing) return;
    const order = draft.segments.map((segment) => segment.id);
    const index = order.indexOf(segmentId);
    const next = index + offset;
    if (index < 0 || next < 0 || next >= order.length) return;
    [order[index], order[next]] = [order[next], order[index]];
    busy = true;
    try {
      draft = await api.reorderSegments({ draft, segment_ids: order });
      changed();
    } catch (reason) {
      reportFailure(reason);
    } finally {
      busy = false;
    }
  }

  async function openPreview(): Promise<void> {
    if (!draft || draft.clozes.length === 0 || composing) return;
    busy = true;
    try {
      previews = await api.previewAuthoringDraft(draft);
      const activeIndex = previews.findIndex(
        (preview) => preview.cloze_id === draft?.active_cloze_id,
      );
      previewIndex = activeIndex < 0 ? 0 : activeIndex;
      previewOpen = true;
      error = "";
    } catch (reason) {
      reportFailure(reason);
    } finally {
      busy = false;
    }
  }

  async function handlePreviewOpenChange(open: boolean): Promise<void> {
    previewOpen = open;
    if (!open) {
      await tick();
      previewTrigger?.focus();
    }
  }

  async function save(next = false): Promise<void> {
    if (!draft || composing) return;
    busy = true;
    try {
      draft = await api.saveAuthoringDraft(draft);
      dirty = false;
      error = "";
      savedMessage = "Card saved on this device.";
      if (onSaved) {
        await onSaved();
      } else if (next) {
        await startNew(false);
      }
    } catch (reason) {
      reportFailure(reason);
    } finally {
      busy = false;
    }
  }

  function handleShortcut(event: KeyboardEvent): void {
    if ((!event.metaKey && !event.ctrlKey) || event.isComposing || composing)
      return;
    const key = event.key.toLowerCase();
    if (key === "s") {
      event.preventDefault();
      void save(event.shiftKey);
    } else if (key === "p") {
      event.preventDefault();
      void openPreview();
    } else if (key === "enter") {
      event.preventDefault();
      void makeCloze();
    } else if (key === "n") {
      event.preventDefault();
      void startNew();
    }
  }

  function protectUnload(event: BeforeUnloadEvent): void {
    if (!dirty) return;
    event.preventDefault();
    event.returnValue = "";
  }
</script>

<svelte:window onkeydown={handleShortcut} onbeforeunload={protectUnload} />

<section class="screen" aria-labelledby="editor-title" aria-busy={busy}>
  <header class="screen-header">
    <div>
      <span class="eyebrow">Card authoring</span>
      <h1 id="editor-title" class="screen-title">Add / Edit card</h1>
      <p class="screen-description">
        Select complete text in any plain segment, make a cloze, add context,
        then preview and save.
      </p>
    </div>
    <div class="cluster">
      {#if onReturn}
        <Button variant="ghost" disabled={busy} onclick={onReturn}
          >{returnLabel}</Button
        >
      {/if}
      {#if !onReturn}
        <Button variant="ghost" disabled={busy} onclick={() => startNew()}
          >New</Button
        >
      {/if}
      <Button
        bind:ref={previewTrigger}
        variant="outline"
        disabled={busy || !draft?.clozes.length}
        onclick={openPreview}>Preview</Button
      >
      {#if !onReturn}
        <Button variant="outline" disabled={busy} onclick={() => save(true)}
          >Save & next</Button
        >
      {/if}
      <Button
        variant="default"
        data-primary-action
        disabled={busy}
        onclick={() => save(false)}>Save</Button
      >
    </div>
  </header>

  {#if error}
    <Alert.Root variant="destructive" role="alert" class="mb-5">
      <Alert.Title>The card was not changed</Alert.Title>
      <Alert.Description>{error}</Alert.Description>
    </Alert.Root>
  {:else if savedMessage}
    <Alert.Root role="status" class="mb-5">
      <Alert.Title>{savedMessage}</Alert.Title>
    </Alert.Root>
  {/if}

  {#if draft}
    <div class="editor-grid">
      <div class="stack">
        <Card.Root class="p-6">
          <div class="stack">
            <div class="source-heading">
              <div>
                <span class="eyebrow">Sentence</span>
                <p>
                  Text and clozes are stable semantic segments. Use
                  <kbd>{shortcut} Enter</kbd> to cloze the current selection.
                </p>
              </div>
              <Button
                variant="default"
                size="sm"
                disabled={busy ||
                  !selection ||
                  selection.start === selection.end}
                onclick={makeCloze}>Make cloze</Button
              >
            </div>

            <div
              class="semantic-source content-text"
              dir={displayDirection(draft.direction)}
              lang={draft.language_tag ?? undefined}
              aria-label="Sentence segments"
            >
              {#each draft.segments as segment, index (segment.id)}
                <div
                  class="segment"
                  class:cloze-segment={segment.kind === "cloze"}
                >
                  {#if segment.kind === "text"}
                    <label class="visually-hidden" for={`segment-${segment.id}`}
                      >Sentence text segment {index + 1}</label
                    >
                    <Textarea
                      id={`segment-${segment.id}`}
                      class="segment-text min-h-20 content-text"
                      dir="auto"
                      rows={2}
                      value={segment.text}
                      oninput={(event) =>
                        updateSegmentText(
                          segment.id,
                          event.currentTarget.value,
                        )}
                      onselect={(event) => rememberSelection(event, segment.id)}
                      onkeyup={(event) => rememberSelection(event, segment.id)}
                      oncompositionstart={() => (composing = true)}
                      oncompositionend={() => (composing = false)}
                    ></Textarea>
                  {:else}
                    <button
                      type="button"
                      class="cloze-chip"
                      class:active={segment.cloze_id === draft.active_cloze_id}
                      aria-pressed={segment.cloze_id === draft.active_cloze_id}
                      onclick={() =>
                        segment.cloze_id && selectCloze(segment.cloze_id)}
                    >
                      <span
                        >Cloze {draft.clozes.findIndex(
                          (cloze) => cloze.id === segment.cloze_id,
                        ) + 1}</span
                      >
                      <bdi>{segment.text}</bdi>
                    </button>
                  {/if}
                  <div
                    class="segment-order"
                    aria-label={`Reorder segment ${index + 1}`}
                  >
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={`Move segment ${index + 1} earlier`}
                      disabled={index === 0 || busy}
                      onclick={() => moveSegment(segment.id, -1)}
                      >Earlier</Button
                    >
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={`Move segment ${index + 1} later`}
                      disabled={index === draft.segments.length - 1 || busy}
                      onclick={() => moveSegment(segment.id, 1)}>Later</Button
                    >
                  </div>
                </div>
              {/each}
            </div>
          </div>
        </Card.Root>

        {#if activeCloze()}
          {@const cloze = activeCloze()!}
          <Card.Root class="p-6">
            <div class="stack metadata">
              <div class="metadata-heading">
                <div>
                  <span class="eyebrow">Active cloze</span>
                  <bdi>{cloze.answer}</bdi>
                </div>
                <Button
                  variant="destructive"
                  size="sm"
                  disabled={busy}
                  onclick={removeActiveCloze}>Convert to text</Button
                >
              </div>

              <div class="field">
                <Label for="surface-answer">Surface answer</Label>
                <Input
                  id="surface-answer"
                  aria-describedby="surface-answer-description"
                  value={cloze.answer}
                  dir="auto"
                  oninput={(event) =>
                    updateActiveCloze((value) => ({
                      ...value,
                      answer: event.currentTarget.value,
                    }))}
                  oncompositionstart={() => (composing = true)}
                  oncompositionend={() => (composing = false)}
                />
                <p id="surface-answer-description" class="field-description">
                  Changing this preserves the cloze and card identities.
                </p>
              </div>

              <div class="field">
                <div class="label-row">
                  <Label for="accepted-answers">Accepted answers</Label>
                  <span>Optional</span>
                </div>
                <Textarea
                  id="accepted-answers"
                  class="min-h-24 content-text"
                  aria-describedby="accepted-answers-description"
                  dir="auto"
                  value={cloze.accepted_answers.join("\n")}
                  oninput={(event) =>
                    updateActiveCloze((value) => ({
                      ...value,
                      accepted_answers: event.currentTarget.value
                        .split("\n")
                        .map((answer) => answer.trim())
                        .filter(Boolean),
                    }))}
                  oncompositionstart={() => (composing = true)}
                  oncompositionend={() => (composing = false)}
                ></Textarea>
                <p id="accepted-answers-description" class="field-description">
                  Enter one explicit alternative per line.
                </p>
              </div>

              <Collapsible.Root>
                <Collapsible.Trigger
                  class="w-full rounded-lg border px-3 py-2 text-left text-sm font-semibold hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
                >
                  Optional cloze details
                </Collapsible.Trigger>
                <Collapsible.Content class="grid gap-6 pt-4">
                  <div class="field">
                    <div class="label-row">
                      <Label for="cloze-hint">Hint</Label>
                      <span>Optional</span>
                    </div>
                    <Input
                      id="cloze-hint"
                      value={cloze.hint}
                      dir="auto"
                      oninput={(event) =>
                        updateActiveCloze((value) => ({
                          ...value,
                          hint: event.currentTarget.value,
                        }))}
                    />
                  </div>

                  <div class="metadata-row">
                    <div class="field">
                      <div class="label-row">
                        <Label for="cloze-language">Language</Label>
                        <span>Optional</span>
                      </div>
                      <Input
                        id="cloze-language"
                        placeholder={draft.language_tag ?? "Inherit card"}
                        value={cloze.language_tag ?? ""}
                        oninput={(event) =>
                          updateActiveCloze((value) => ({
                            ...value,
                            language_tag:
                              event.currentTarget.value.trim() || null,
                          }))}
                      />
                    </div>
                    <div class="field">
                      <Label for="cloze-direction">Direction</Label>
                      <select
                        id="cloze-direction"
                        value={cloze.direction}
                        onchange={(event) =>
                          updateActiveCloze((value) => ({
                            ...value,
                            direction: event.currentTarget
                              .value as DirectionDto,
                          }))}
                      >
                        <option value="auto">Auto / inherit</option>
                        <option value="ltr">Left to right</option>
                        <option value="rtl">Right to left</option>
                      </select>
                    </div>
                  </div>

                  <div class="field">
                    <Label for="cloze-matching">Answer matching</Label>
                    <select
                      id="cloze-matching"
                      aria-describedby="cloze-matching-description"
                      value={cloze.matching_policy ?? ""}
                      onchange={(event) =>
                        updateActiveCloze((value) => ({
                          ...value,
                          matching_policy:
                            (event.currentTarget.value as MatchingPolicyDto) ||
                            null,
                        }))}
                    >
                      <option value=""
                        >Inherit deck ({draft.deck_matching_policy})</option
                      >
                      <option value="strict">Strict Unicode</option>
                      <option value="forgiving"
                        >Forgiving case, accents, width, and punctuation</option
                      >
                    </select>
                    <p
                      id="cloze-matching-description"
                      class="field-description"
                    >
                      Inherit the deck policy or override it for this cloze.
                    </p>
                  </div>

                  <div class="field">
                    <div class="label-row">
                      <Label for="cloze-explanation">Explanation</Label>
                      <span>Optional</span>
                    </div>
                    <Textarea
                      id="cloze-explanation"
                      class="min-h-24 content-text"
                      aria-describedby="cloze-explanation-description"
                      dir="auto"
                      value={cloze.explanation_markdown}
                      oninput={(event) =>
                        updateActiveCloze((value) => ({
                          ...value,
                          explanation_markdown: event.currentTarget.value,
                        }))}
                      oncompositionstart={() => (composing = true)}
                      oncompositionend={() => (composing = false)}
                    ></Textarea>
                    <p
                      id="cloze-explanation-description"
                      class="field-description"
                    >
                      Limited Markdown is stored as text. Raw HTML and
                      executable links are rejected.
                    </p>
                  </div>

                  <div class="annotation-heading">
                    <div>
                      <span class="eyebrow">Ordered annotations</span>
                      <p>
                        Use any labels your material needs: reading, gloss, or
                        note.
                      </p>
                    </div>
                    <Button variant="outline" size="sm" onclick={addAnnotation}
                      >Add annotation</Button
                    >
                  </div>
                  {#each cloze.annotations as annotation, index (annotation.id)}
                    <div class="annotation">
                      <Input
                        aria-label={`Annotation ${index + 1} label`}
                        placeholder="Label"
                        value={annotation.label}
                        oninput={(event) =>
                          updateAnnotation(annotation.id, (value) => ({
                            ...value,
                            label: event.currentTarget.value,
                          }))}
                      />
                      <Input
                        aria-label={`Annotation ${index + 1} value`}
                        placeholder="Value"
                        dir="auto"
                        value={annotation.value}
                        oninput={(event) =>
                          updateAnnotation(annotation.id, (value) => ({
                            ...value,
                            value: event.currentTarget.value,
                          }))}
                      />
                      <div class="cluster">
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`Move annotation ${index + 1} earlier`}
                          disabled={index === 0}
                          onclick={() => moveAnnotation(annotation.id, -1)}
                          >Earlier</Button
                        >
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`Move annotation ${index + 1} later`}
                          disabled={index === cloze.annotations.length - 1}
                          onclick={() => moveAnnotation(annotation.id, 1)}
                          >Later</Button
                        >
                        <Button
                          variant="destructive"
                          size="sm"
                          aria-label={`Remove annotation ${index + 1}`}
                          onclick={() => removeAnnotation(annotation.id)}
                          >Remove</Button
                        >
                      </div>
                    </div>
                  {/each}
                </Collapsible.Content>
              </Collapsible.Root>
            </div>
          </Card.Root>
        {/if}
      </div>

      <aside class="stack" aria-label="Card defaults and optional media">
        <Card.Root class="bg-muted/40 p-4 shadow-none">
          <div class="stack compact-stack">
            <span class="eyebrow">Deck defaults / card overrides</span>
            <div class="field">
              <Label for="authoring-deck">Deck</Label>
              <select
                id="authoring-deck"
                aria-label="Author in deck"
                value={draft.deck_id}
                disabled={busy || decks.length === 0}
                onchange={(event) => chooseDeck(event.currentTarget.value)}
              >
                {#each decks as deck (deck.id)}
                  <option value={deck.id}>{deck.name}</option>
                {/each}
              </select>
            </div>
            <dl class="deck-defaults">
              <div>
                <dt>Language</dt>
                <dd>{draft.deck_language_tag ?? "Automatic"}</dd>
              </div>
              <div>
                <dt>Direction</dt>
                <dd>{draft.deck_direction}</dd>
              </div>
              <div>
                <dt>Matching</dt>
                <dd>{draft.deck_matching_policy}</dd>
              </div>
            </dl>
            <div class="field">
              <div class="label-row">
                <Label for="source-language">Language</Label>
                <span>Optional</span>
              </div>
              <Input
                id="source-language"
                placeholder="BCP 47, for example ja"
                value={draft.language_tag ?? ""}
                oninput={(event) =>
                  updateSourceLanguage(event.currentTarget.value)}
              />
            </div>
            <div class="field">
              <Label for="source-direction">Direction</Label>
              <select
                id="source-direction"
                value={draft.direction}
                onchange={(event) =>
                  updateSourceDirection(event.currentTarget.value)}
              >
                <option value="auto">Automatic</option>
                <option value="ltr">Left to right</option>
                <option value="rtl">Right to left</option>
              </select>
            </div>
            <p class="default-note">
              Source language and direction override the deck. Matching and
              explicit accepted answers can be overridden per cloze.
            </p>
          </div>
        </Card.Root>
        <Card.Root class="bg-muted/40 p-4 shadow-none">
          <Collapsible.Root>
            <Collapsible.Trigger
              class="w-full rounded-lg px-2 py-1 text-left text-xs font-extrabold tracking-[0.09em] text-muted-foreground uppercase hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
            >
              Local media
            </Collapsible.Trigger>
            <Collapsible.Content class="grid gap-4 pt-4">
              {#if activeCloze()}
                {@const mediaCloze = activeCloze()!}
                <div class="media-actions">
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={busy}
                    onclick={() => attachMedia("prompt_audio")}
                    >Add prompt audio</Button
                  >
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={busy}
                    onclick={() => attachMedia("answer_audio")}
                    >Add answer audio</Button
                  >
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={busy}
                    onclick={() => attachMedia("reveal_image")}
                    >Add reveal image</Button
                  >
                </div>
                {#each mediaCloze.media as media (media.id)}
                  <div class="media-attachment">
                    <MediaFrame
                      kind={media.kind}
                      label={media.original_file_name ??
                        mediaRoleLabel(media.role)}
                      role={media.role}
                      availability={media.availability}
                      source={mediaAssetSource(media)}
                      contentHash={media.content_hash}
                      mediaType={media.media_type}
                      altText={media.alt_text}
                      width={media.width}
                      height={media.height}
                      durationMs={media.duration_ms}
                    />
                    <Input
                      aria-label={`${mediaRoleLabel(media.role)} alternative text`}
                      placeholder={media.kind === "image"
                        ? "Describe this image"
                        : "Optional audio label"}
                      value={media.alt_text ?? ""}
                      oninput={(event) =>
                        updateMediaAlt(media.id, event.currentTarget.value)}
                    />
                    <Button
                      variant="destructive"
                      size="sm"
                      disabled={busy}
                      onclick={() => removeMedia(media.id)}
                      >Remove {mediaRoleLabel(media.role).toLowerCase()}</Button
                    >
                  </div>
                {:else}
                  <MediaFrame kind="audio" label="Prompt or answer audio" />
                  <MediaFrame kind="image" label="Reveal image" />
                {/each}
                <p class="default-note">
                  Files stay on this device in a checksum-addressed store.
                  Identical files share one object.
                </p>
              {:else}
                <p class="default-note">
                  Select or create a cloze before attaching media.
                </p>
              {/if}
            </Collapsible.Content>
          </Collapsible.Root>
        </Card.Root>
      </aside>
    </div>
  {:else}
    <Alert.Root role="status">
      <Alert.Title>Preparing a new card…</Alert.Title>
    </Alert.Root>
  {/if}
</section>

<Dialog.Root
  open={previewOpen}
  onOpenChange={(open) => void handlePreviewOpenChange(open)}
>
  <Dialog.Content class="max-w-2xl">
    <Dialog.Header>
      <Dialog.Title>Card preview</Dialog.Title>
      <Dialog.Description>
        Each cloze produces an independent card. Controls remain left to right.
      </Dialog.Description>
    </Dialog.Header>
    {#if activePreview()}
      {@const preview = activePreview()!}
      <div class="preview-shell" dir="ltr">
        <div class="preview-tabs" role="tablist" aria-label="Cloze previews">
          {#each previews as item, index (item.cloze_id)}
            <button
              type="button"
              role="tab"
              aria-selected={index === previewIndex}
              onclick={() => (previewIndex = index)}>Card {index + 1}</button
            >
          {/each}
        </div>
        <p
          class="dialog-prompt content-text"
          dir={displayDirection(preview.direction)}
          lang={preview.language_tag ?? undefined}
        >
          {preview.prompt}
        </p>
        {#if preview.hint}<p class="preview-hint" dir="auto">
            {preview.hint}
          </p>{/if}
        {#if preview.annotations.length}
          <dl class="preview-annotations">
            {#each preview.annotations as annotation (annotation.id)}
              <div>
                <dt>{annotation.label}</dt>
                <dd dir={displayDirection(annotation.direction)}>
                  {annotation.value}
                </dd>
              </div>
            {/each}
          </dl>
        {/if}
        {#if preview.explanation_markdown}
          <div dir="auto">
            <LimitedMarkdown value={preview.explanation_markdown} />
          </div>
        {/if}
      </div>
    {/if}
    <Dialog.Footer>
      <Dialog.Close>
        {#snippet child({ props })}
          <Button {...props}>Done</Button>
        {/snippet}
      </Dialog.Close>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<AlertDialog.Root bind:open={confirmationOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>
        {confirmationKind === "remove-cloze"
          ? "Convert this cloze to text?"
          : "Discard unsaved changes?"}
      </AlertDialog.Title>
      <AlertDialog.Description>
        {confirmationDescription}
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        class="bg-destructive/10 text-destructive hover:bg-destructive/20"
        onclick={() => void confirmEditorAction()}
      >
        {confirmationKind === "remove-cloze"
          ? "Convert to text"
          : "Discard and start new"}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>

<style>
  .editor-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(17rem, 0.65fr);
    gap: 1.25rem;
    margin-top: 1.25rem;
  }

  .source-heading,
  .metadata-heading,
  .annotation-heading {
    display: flex;
    gap: 1rem;
    align-items: center;
    justify-content: space-between;
  }

  .source-heading p,
  .annotation-heading p,
  .default-note {
    margin: 0;
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    line-height: 1.5;
  }

  .source-heading kbd {
    white-space: nowrap;
  }

  .media-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .media-attachment {
    display: grid;
    gap: 0.5rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--border);
  }

  .semantic-source {
    display: grid;
    gap: 0.75rem;
  }

  .segment {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.5rem;
    align-items: center;
  }

  .segment-order {
    display: grid;
    gap: 0.5rem;
  }

  .cloze-chip {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    min-height: 4.5rem;
    padding: 0.75rem 1rem;
    border: 1px solid var(--primary);
    border-radius: var(--radius-lg);
    color: var(--foreground);
    background: var(--accent);
    text-align: start;
    cursor: pointer;
  }

  .cloze-chip.active {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 24%, transparent);
  }

  .cloze-chip span {
    color: var(--primary);
    font-size: var(--text-xs);
    font-weight: 800;
    text-transform: uppercase;
  }

  .cloze-chip bdi,
  .metadata-heading bdi {
    font-family: var(--font-content);
    font-size: var(--text-lg);
    font-weight: 700;
  }

  .metadata {
    gap: 1.5rem;
  }

  .metadata-row {
    display: grid;
    grid-template-columns: 1fr minmax(10rem, 0.5fr);
    gap: 1rem;
  }

  .annotation {
    display: grid;
    grid-template-columns: minmax(8rem, 0.35fr) minmax(12rem, 1fr) auto;
    gap: 0.75rem;
    align-items: center;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  .compact-stack {
    gap: 1rem;
  }

  .deck-defaults {
    display: grid;
    gap: 0.5rem;
    margin: 0;
  }

  .deck-defaults div {
    display: flex;
    gap: 0.75rem;
    justify-content: space-between;
  }

  .deck-defaults dt {
    color: var(--muted-foreground);
    font-size: var(--text-xs);
  }

  .deck-defaults dd {
    margin: 0;
    font-size: var(--text-xs);
    font-weight: 700;
    text-transform: capitalize;
  }

  .preview-shell {
    display: grid;
    gap: 1.25rem;
  }

  .preview-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .preview-tabs button {
    min-height: 2.25rem;
    padding-inline: 0.75rem;
    border: 1px solid var(--input);
    border-radius: var(--radius-lg);
    color: var(--foreground);
    background: var(--card);
    cursor: pointer;
  }

  .preview-tabs button[aria-selected="true"] {
    border-color: var(--primary);
    color: var(--primary);
    background: var(--accent);
  }

  .dialog-prompt {
    margin: 0;
    font-size: var(--text-xl);
    line-height: 1.8;
    text-align: center;
  }

  .preview-hint {
    margin: 0;
    color: var(--muted-foreground);
    text-align: center;
  }

  .preview-annotations {
    display: grid;
    gap: 0.5rem;
    margin: 0;
  }

  .preview-annotations div {
    display: grid;
    grid-template-columns: minmax(6rem, 0.35fr) 1fr;
    gap: 0.75rem;
  }

  .preview-annotations dt {
    color: var(--muted-foreground);
    font-weight: 700;
  }

  .preview-annotations dd {
    margin: 0;
    unicode-bidi: isolate;
  }

  @media (max-width: 900px) {
    .editor-grid {
      grid-template-columns: 1fr;
    }

    .annotation {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 620px) {
    .source-heading,
    .metadata-heading,
    .annotation-heading {
      align-items: stretch;
      flex-direction: column;
    }

    .metadata-row {
      grid-template-columns: 1fr;
    }
  }
</style>
