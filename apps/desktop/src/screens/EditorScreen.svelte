<script lang="ts">
  import { onDestroy, onMount } from "svelte";

  import { api } from "../lib/api";
  import Button from "../lib/components/Button.svelte";
  import Dialog from "../lib/components/Dialog.svelte";
  import Feedback from "../lib/components/Feedback.svelte";
  import Field from "../lib/components/Field.svelte";
  import LimitedMarkdown from "../lib/components/LimitedMarkdown.svelte";
  import MediaFrame from "../lib/components/MediaFrame.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";
  import TextInput from "../lib/components/TextInput.svelte";
  import type { AnnotationDraftDto } from "../lib/generated/AnnotationDraftDto";
  import type { AuthoringClozeDto } from "../lib/generated/AuthoringClozeDto";
  import type { AuthoringDraftDto } from "../lib/generated/AuthoringDraftDto";
  import type { AuthoringPreviewDto } from "../lib/generated/AuthoringPreviewDto";
  import type { DirectionDto } from "../lib/generated/DirectionDto";
  import type { MatchingPolicyDto } from "../lib/generated/MatchingPolicyDto";
  import type { MediaRoleDto } from "../lib/generated/MediaRoleDto";
  import { mediaAssetSource } from "../lib/media";

  type Props = {
    cardId?: string | null;
    onReturn?: () => void;
  };

  let { cardId = null, onReturn }: Props = $props();

  type Selection = {
    segmentId: string;
    start: number;
    end: number;
  };

  let draft = $state<AuthoringDraftDto | null>(null);
  let selection = $state<Selection | null>(null);
  let previews = $state<AuthoringPreviewDto[]>([]);
  let previewIndex = $state(0);
  let previewOpen = $state(false);
  let dirty = $state(false);
  let composing = $state(false);
  let busy = $state(false);
  let error = $state("");
  let savedMessage = $state("");

  const shortcut = navigator.platform.includes("Mac") ? "⌘" : "Ctrl";

  onMount(() => {
    if (cardId) {
      void loadCardDraft(cardId);
    } else {
      void startNew(false);
    }
  });

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

  async function startNew(protect = true): Promise<void> {
    if (composing) return;
    if (
      protect &&
      dirty &&
      !window.confirm("Discard the unsaved source note and start a new one?")
    ) {
      return;
    }
    busy = true;
    try {
      draft = await api.newAuthoringDraft();
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
    busy = true;
    try {
      draft = await api.removeCloze({
        draft,
        cloze_id: draft.active_cloze_id,
      });
      changed();
    } catch (reason) {
      reportFailure(reason);
    } finally {
      busy = false;
    }
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

  async function save(next = false): Promise<void> {
    if (!draft || composing) return;
    busy = true;
    try {
      draft = await api.saveAuthoringDraft(draft);
      dirty = false;
      error = "";
      savedMessage = "Source note saved on this device.";
      if (next) await startNew(false);
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
      <span class="eyebrow">Source-first authoring</span>
      <h1 id="editor-title" class="screen-title">Add / Edit</h1>
      <p class="screen-description">
        Select complete text in any plain segment, make a cloze, add context,
        then preview and save.
      </p>
    </div>
    <div class="cluster">
      {#if cardId && onReturn}
        <Button variant="quiet" disabled={busy} onclick={onReturn}
          >Return to study</Button
        >
      {/if}
      <Button
        variant="quiet"
        shortcut={`${shortcut} N`}
        disabled={busy}
        onclick={() => startNew()}>New</Button
      >
      <Button
        variant="secondary"
        shortcut={`${shortcut} P`}
        disabled={busy || !draft?.clozes.length}
        onclick={openPreview}>Preview</Button
      >
      <Button
        variant="secondary"
        shortcut={`${shortcut} ⇧ S`}
        disabled={busy}
        onclick={() => save(true)}>Save & next</Button
      >
      <Button
        variant="primary"
        shortcut={`${shortcut} S`}
        data-primary-action
        disabled={busy}
        onclick={() => save(false)}>Save</Button
      >
    </div>
  </header>

  {#if error}
    <Feedback tone="error" title="The source note was not changed" compact>
      <p>{error}</p>
    </Feedback>
  {:else if savedMessage}
    <Feedback tone="success" title={savedMessage} compact />
  {/if}

  {#if draft}
    <div class="editor-grid">
      <div class="stack">
        <SurfaceCard>
          <div class="stack">
            <div class="source-heading">
              <div>
                <span class="eyebrow">Source content</span>
                <p>
                  Text and clozes are stable semantic segments. Use
                  <kbd>{shortcut} Enter</kbd> to cloze the current selection.
                </p>
              </div>
              <Button
                variant="primary"
                size="small"
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
              aria-label="Semantic source segments"
            >
              {#each draft.segments as segment, index (segment.id)}
                <div
                  class="segment"
                  class:cloze-segment={segment.kind === "cloze"}
                >
                  {#if segment.kind === "text"}
                    <label class="visually-hidden" for={`segment-${segment.id}`}
                      >Source text segment {index + 1}</label
                    >
                    <textarea
                      id={`segment-${segment.id}`}
                      class="segment-text content-text"
                      dir="auto"
                      rows="2"
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
                    ></textarea>
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
                      variant="quiet"
                      size="small"
                      aria-label={`Move segment ${index + 1} earlier`}
                      disabled={index === 0 || busy}
                      onclick={() => moveSegment(segment.id, -1)}>↑</Button
                    >
                    <Button
                      variant="quiet"
                      size="small"
                      aria-label={`Move segment ${index + 1} later`}
                      disabled={index === draft.segments.length - 1 || busy}
                      onclick={() => moveSegment(segment.id, 1)}>↓</Button
                    >
                  </div>
                </div>
              {/each}
            </div>
          </div>
        </SurfaceCard>

        {#if activeCloze()}
          {@const cloze = activeCloze()!}
          <SurfaceCard>
            <div class="stack metadata">
              <div class="metadata-heading">
                <div>
                  <span class="eyebrow">Active cloze</span>
                  <bdi>{cloze.answer}</bdi>
                </div>
                <Button
                  variant="danger"
                  size="small"
                  disabled={busy}
                  onclick={removeActiveCloze}>Convert to text</Button
                >
              </div>

              <Field
                id="surface-answer"
                label="Surface answer"
                description="Changing this preserves the cloze and card identities."
              >
                <TextInput
                  id="surface-answer"
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
              </Field>

              <Field
                id="accepted-answers"
                label="Accepted answers"
                description="Enter one explicit alternative per line."
                optional
              >
                <textarea
                  id="accepted-answers"
                  class="compact-textarea content-text"
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
                ></textarea>
              </Field>

              <Field id="cloze-hint" label="Hint" optional>
                <TextInput
                  id="cloze-hint"
                  value={cloze.hint}
                  dir="auto"
                  oninput={(event) =>
                    updateActiveCloze((value) => ({
                      ...value,
                      hint: event.currentTarget.value,
                    }))}
                />
              </Field>

              <div class="metadata-row">
                <Field id="cloze-language" label="Language" optional>
                  <TextInput
                    id="cloze-language"
                    placeholder={draft.language_tag ?? "Inherit deck / source"}
                    value={cloze.language_tag ?? ""}
                    oninput={(event) =>
                      updateActiveCloze((value) => ({
                        ...value,
                        language_tag: event.currentTarget.value.trim() || null,
                      }))}
                  />
                </Field>
                <Field id="cloze-direction" label="Direction">
                  <select
                    id="cloze-direction"
                    value={cloze.direction}
                    onchange={(event) =>
                      updateActiveCloze((value) => ({
                        ...value,
                        direction: event.currentTarget.value as DirectionDto,
                      }))}
                  >
                    <option value="auto">Auto / inherit</option>
                    <option value="ltr">Left to right</option>
                    <option value="rtl">Right to left</option>
                  </select>
                </Field>
              </div>

              <Field
                id="cloze-matching"
                label="Answer matching"
                description="Inherit the deck policy or override it for this cloze."
              >
                <select
                  id="cloze-matching"
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
              </Field>

              <Field
                id="cloze-explanation"
                label="Explanation"
                description="Limited Markdown is stored as text. Raw HTML and executable links are rejected."
                optional
              >
                <textarea
                  id="cloze-explanation"
                  class="compact-textarea content-text"
                  dir="auto"
                  value={cloze.explanation_markdown}
                  oninput={(event) =>
                    updateActiveCloze((value) => ({
                      ...value,
                      explanation_markdown: event.currentTarget.value,
                    }))}
                  oncompositionstart={() => (composing = true)}
                  oncompositionend={() => (composing = false)}
                ></textarea>
              </Field>

              <div class="annotation-heading">
                <div>
                  <span class="eyebrow">Ordered annotations</span>
                  <p>
                    Use any labels your material needs: reading, gloss, or note.
                  </p>
                </div>
                <Button variant="secondary" size="small" onclick={addAnnotation}
                  >Add annotation</Button
                >
              </div>
              {#each cloze.annotations as annotation, index (annotation.id)}
                <div class="annotation">
                  <TextInput
                    aria-label={`Annotation ${index + 1} label`}
                    placeholder="Label"
                    value={annotation.label}
                    oninput={(event) =>
                      updateAnnotation(annotation.id, (value) => ({
                        ...value,
                        label: event.currentTarget.value,
                      }))}
                  />
                  <TextInput
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
                      variant="quiet"
                      size="small"
                      aria-label={`Move annotation ${index + 1} earlier`}
                      disabled={index === 0}
                      onclick={() => moveAnnotation(annotation.id, -1)}
                      >↑</Button
                    >
                    <Button
                      variant="quiet"
                      size="small"
                      aria-label={`Move annotation ${index + 1} later`}
                      disabled={index === cloze.annotations.length - 1}
                      onclick={() => moveAnnotation(annotation.id, 1)}>↓</Button
                    >
                    <Button
                      variant="danger"
                      size="small"
                      aria-label={`Remove annotation ${index + 1}`}
                      onclick={() => removeAnnotation(annotation.id)}
                      >Remove</Button
                    >
                  </div>
                </div>
              {/each}
            </div>
          </SurfaceCard>
        {/if}
      </div>

      <aside class="stack" aria-label="Source defaults and optional media">
        <SurfaceCard padding="compact" tone="quiet">
          <div class="stack compact-stack">
            <span class="eyebrow">Deck defaults / source overrides</span>
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
            <Field id="source-language" label="Language" optional>
              <TextInput
                id="source-language"
                placeholder="BCP 47, for example ja"
                value={draft.language_tag ?? ""}
                oninput={(event) =>
                  updateSourceLanguage(event.currentTarget.value)}
              />
            </Field>
            <Field id="source-direction" label="Direction">
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
            </Field>
            <p class="default-note">
              Source language and direction override the deck. Matching and
              explicit accepted answers can be overridden per cloze.
            </p>
          </div>
        </SurfaceCard>
        <SurfaceCard padding="compact" tone="quiet">
          <div class="stack compact-stack">
            <span class="eyebrow">Local media</span>
            {#if activeCloze()}
              {@const mediaCloze = activeCloze()!}
              <div class="media-actions">
                <Button
                  variant="secondary"
                  size="small"
                  disabled={busy}
                  onclick={() => attachMedia("prompt_audio")}
                  >Add prompt audio</Button
                >
                <Button
                  variant="secondary"
                  size="small"
                  disabled={busy}
                  onclick={() => attachMedia("answer_audio")}
                  >Add answer audio</Button
                >
                <Button
                  variant="secondary"
                  size="small"
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
                    source={mediaAssetSource(media.asset_path)}
                    mediaType={media.media_type}
                    altText={media.alt_text}
                    width={media.width}
                    height={media.height}
                  />
                  <TextInput
                    aria-label={`${mediaRoleLabel(media.role)} alternative text`}
                    placeholder={media.kind === "image"
                      ? "Describe this image"
                      : "Optional audio label"}
                    value={media.alt_text ?? ""}
                    oninput={(event) =>
                      updateMediaAlt(media.id, event.currentTarget.value)}
                  />
                  <Button
                    variant="danger"
                    size="small"
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
          </div>
        </SurfaceCard>
      </aside>
    </div>
  {:else}
    <Feedback tone="info" title="Preparing a new source note…" />
  {/if}
</section>

<Dialog
  open={previewOpen}
  title="Card preview"
  description="Each cloze produces an independent card. Controls remain left to right."
  onClose={() => (previewOpen = false)}
>
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
  {#snippet actions()}
    <Button variant="primary" onclick={() => (previewOpen = false)}>Done</Button
    >
  {/snippet}
</Dialog>

<style>
  .editor-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(17rem, 0.65fr);
    gap: var(--space-5);
    margin-top: var(--space-5);
  }

  .source-heading,
  .metadata-heading,
  .annotation-heading {
    display: flex;
    gap: var(--space-4);
    align-items: center;
    justify-content: space-between;
  }

  .source-heading p,
  .annotation-heading p,
  .default-note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--text-xs);
    line-height: 1.5;
  }

  .source-heading kbd {
    white-space: nowrap;
  }

  .media-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .media-attachment {
    display: grid;
    gap: var(--space-2);
    padding-bottom: var(--space-3);
    border-bottom: var(--border-width) solid var(--color-border);
  }

  .semantic-source {
    display: grid;
    gap: var(--space-3);
  }

  .segment {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--space-2);
    align-items: center;
  }

  .segment-text {
    min-height: 5rem;
  }

  .segment-order {
    display: grid;
    gap: var(--space-1);
  }

  .cloze-chip {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    min-height: 4.5rem;
    padding: var(--space-3) var(--space-4);
    border: var(--border-width) solid var(--color-accent);
    border-radius: var(--radius-control);
    color: var(--color-text);
    background: var(--color-accent-soft);
    text-align: start;
    cursor: pointer;
  }

  .cloze-chip.active {
    box-shadow: 0 0 0 3px
      color-mix(in srgb, var(--color-accent) 24%, transparent);
  }

  .cloze-chip span {
    color: var(--color-accent);
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
    gap: var(--space-6);
  }

  .metadata-row {
    display: grid;
    grid-template-columns: 1fr minmax(10rem, 0.5fr);
    gap: var(--space-4);
  }

  .compact-textarea {
    min-height: 6rem;
  }

  .annotation {
    display: grid;
    grid-template-columns: minmax(8rem, 0.35fr) minmax(12rem, 1fr) auto;
    gap: var(--space-3);
    align-items: center;
    padding: var(--space-3);
    border: var(--border-width) solid var(--color-border);
    border-radius: var(--radius-control);
  }

  .compact-stack {
    gap: var(--space-4);
  }

  .deck-defaults {
    display: grid;
    gap: var(--space-2);
    margin: 0;
  }

  .deck-defaults div {
    display: flex;
    gap: var(--space-3);
    justify-content: space-between;
  }

  .deck-defaults dt {
    color: var(--color-text-muted);
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
    gap: var(--space-5);
  }

  .preview-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .preview-tabs button {
    min-height: var(--control-height-small);
    padding-inline: var(--space-3);
    border: var(--border-width) solid var(--color-border-strong);
    border-radius: var(--radius-control);
    color: var(--color-text);
    background: var(--color-surface);
    cursor: pointer;
  }

  .preview-tabs button[aria-selected="true"] {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-soft);
  }

  .dialog-prompt {
    margin: 0;
    font-size: var(--text-xl);
    line-height: 1.8;
    text-align: center;
  }

  .preview-hint {
    margin: 0;
    color: var(--color-text-muted);
    text-align: center;
  }

  .preview-annotations {
    display: grid;
    gap: var(--space-2);
    margin: 0;
  }

  .preview-annotations div {
    display: grid;
    grid-template-columns: minmax(6rem, 0.35fr) 1fr;
    gap: var(--space-3);
  }

  .preview-annotations dt {
    color: var(--color-text-muted);
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
