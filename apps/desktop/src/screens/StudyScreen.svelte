<script lang="ts">
  import { onMount, tick } from "svelte";
  import { SvelteDate } from "svelte/reactivity";

  import TypingKeyboard from "../components/TypingKeyboard.svelte";
  import { api } from "../lib/api";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import LimitedMarkdown from "../lib/components/LimitedMarkdown.svelte";
  import MediaFrame from "../lib/components/MediaFrame.svelte";
  import type { GradeDto } from "../lib/generated/GradeDto";
  import type { GradePreviewDto } from "../lib/generated/GradePreviewDto";
  import type { GradeReviewRequest } from "../lib/generated/GradeReviewRequest";
  import type { GradeReviewResultDto } from "../lib/generated/GradeReviewResultDto";
  import type { RevealDto } from "../lib/generated/RevealDto";
  import type { StudyCardDto } from "../lib/generated/StudyCardDto";
  import type { StudyAvailabilityDto } from "../lib/generated/StudyAvailabilityDto";
  import {
    mediaAssetSource,
    readPromptAudioAutoplay,
    writePromptAudioAutoplay,
  } from "../lib/media";
  import { messages } from "../lib/messages";
  import {
    readStudyFrontAnswerPreference,
    readStudyVisualKeyboardPreference,
  } from "../lib/study-preferences";
  import {
    clearStudyQueue,
    clearStudySession,
    completePendingReview,
    readStudyQueue,
    recoverPendingReview,
    remainingStudyCards,
    remainingQueueEntries,
    startStudyQueue,
    studySessionKey,
    writeStudyQueue,
    type StudyQueueSession,
  } from "../lib/study-queue";
  import {
    detectInstructionPlatform,
    instructionPlatformPreferenceKey,
    isInstructionPlatform,
    koreanKeyLegends,
    type InstructionPlatform,
  } from "../lib/typing-lessons";
  import {
    readVimKeybindings,
    type VimMode,
    vimCommandAllowed,
  } from "../lib/vim-keybindings";

  type Props = {
    onCreate: () => void;
    onEdit?: (cardId: string) => void;
    onQueueComplete?: () => void;
  };

  type ViewState =
    | "loading"
    | "empty"
    | "prompt"
    | "checking"
    | "revealed"
    | "committing"
    | "next"
    | "error";
  type StableView = "prompt" | "revealed" | "next";
  type RetryAction =
    "recover" | "load" | "check" | "grade" | "suspend" | "undo";
  type CompletionKind = "graded" | "suspended" | null;
  type StoredStudySession = {
    card: StudyCardDto;
    reveal: RevealDto | null;
    result: GradeReviewResultDto | null;
    response: string;
    view: StableView;
    responseDurationMs: number;
    completionKind: CompletionKind;
  };

  const selectedDeckKey = "meiki-today-deck";
  const defaultDeckId = "default-deck";
  const allDecksId = "__all_decks__";
  const grades: GradeDto[] = ["again", "hard", "good", "easy"];

  let { onCreate, onEdit, onQueueComplete }: Props = $props();
  let view = $state<ViewState>("loading");
  let recoveryView = $state<StableView>("prompt");
  let retryAction = $state<RetryAction>("load");
  let pendingGrade = $state<GradeDto | null>(null);
  let pendingUndoEventId = $state<string | null>(null);
  let completionKind = $state<CompletionKind>(null);
  let card = $state<StudyCardDto | null>(null);
  let reveal = $state<RevealDto | null>(null);
  let result = $state<GradeReviewResultDto | null>(null);
  let response = $state("");
  let responseDurationMs = $state(0);
  let errorMessage = $state("");
  let sessionNotice = $state("");
  let audioNotice = $state("");
  let undoNotice = $state("");
  let hintVisible = $state(false);
  let composing = $state(false);
  let answerInput = $state<HTMLInputElement | null>(null);
  let studyElement = $state<HTMLElement | undefined>();
  let promptStartedAt = $state(0);
  let autoplayPromptAudio = $state(true);
  let promptAudioAutoplayPending = $state(false);
  let queueSession = $state<StudyQueueSession | null>(null);
  let studyAvailability = $state<StudyAvailabilityDto | null>(null);
  let nextDueAt = $state<string | null>(null);
  let vimKeybindingsEnabled = $state(false);
  let vimMode = $state<VimMode>("normal");
  let studyFrontAnswerEnabled = $state(false);
  let studyVisualKeyboardEnabled = $state(false);
  let studyKeyboardPlatform = $state<InstructionPlatform | null>(null);
  let studyKeyboardPressedCodes = $state<string[]>([]);
  let studyKeyboardLanguage = $derived(primaryLanguage(card?.language_tag));
  let studyKeyboardLegends = $derived(
    studyKeyboardLanguage === "ko" && studyKeyboardPlatform
      ? koreanKeyLegends
      : {},
  );
  const firstReadyPromptAudioId = $derived(
    card?.prompt_media.find(
      (media) =>
        media.role === "prompt_audio" && media.availability === "ready",
    )?.id,
  );

  onMount(() => {
    autoplayPromptAudio = readPromptAudioAutoplay();
    studyFrontAnswerEnabled = readStudyFrontAnswerPreference();
    studyVisualKeyboardEnabled = readStudyVisualKeyboardPreference();
    const savedPlatform = localStorage.getItem(
      instructionPlatformPreferenceKey,
    );
    studyKeyboardPlatform = isInstructionPlatform(savedPlatform)
      ? savedPlatform
      : detectInstructionPlatform(navigator);
    vimKeybindingsEnabled = readVimKeybindings();
    void prepareStudy();
    return resetStudyKeyboardState;
  });

  async function prepareStudy(): Promise<void> {
    resetStudyKeyboardState();
    view = "loading";
    errorMessage = "";
    try {
      queueSession = readStudyQueue();
      if (!queueSession) {
        const deckId = localStorage.getItem(selectedDeckKey) ?? allDecksId;
        const plan = await currentStudyPlan(deckId);
        if (plan.availability !== "ready") {
          studyAvailability = plan.availability;
          nextDueAt = plan.overview.next_due_at;
          card = null;
          view = "empty";
          return;
        }
        queueSession = startStudyQueue(plan.overview);
      }
      if (queueSession.pendingReview) {
        queueSession = await recoverPendingReview(
          queueSession,
          api.gradeReview,
        );
        writeStudyQueue(queueSession);
      }
      if (!queueSession) return;
      await reconcileQueue();
      if (!queueSession) return;
      await restoreOrLoad();
    } catch (error) {
      fail(error, "prompt", "recover");
    }
  }

  async function currentStudyPlan(deckId: string) {
    const settings = await api.getSchedulerSettings(
      deckId === allDecksId ? defaultDeckId : deckId,
    );
    const now = new Date();
    const { start, end } = localDayBounds(now, settings.day_boundary_minutes);
    return api.prepareStudy({
      deck_id: deckId,
      now_ms: now.getTime(),
      day_start_ms: start.getTime(),
      day_end_ms: end.getTime(),
    });
  }

  async function reconcileQueue(): Promise<void> {
    if (!queueSession) return;
    if (queueSession.position >= queueSession.entries.length) {
      finishQueue();
      return;
    }
    const settings = await api.getSchedulerSettings(
      queueSession.deckId === allDecksId ? defaultDeckId : queueSession.deckId,
    );
    const now = new Date();
    const { start, end } = localDayBounds(now, settings.day_boundary_minutes);
    const entries = await api.reconcileStudyQueue({
      deck_id: queueSession.deckId,
      now_ms: now.getTime(),
      day_start_ms: start.getTime(),
      day_end_ms: end.getTime(),
      entries: remainingQueueEntries(queueSession),
    });
    if (!entries.length) {
      finishQueue();
      return;
    }
    queueSession = {
      ...queueSession,
      entries,
      position: 0,
      pendingReview: null,
    };
    writeStudyQueue(queueSession);
  }

  async function restoreOrLoad(): Promise<void> {
    promptAudioAutoplayPending = false;
    const stored = sessionStorage.getItem(studySessionKey);
    sessionStorage.removeItem(studySessionKey);
    if (!stored) {
      await loadCard();
      return;
    }

    view = "loading";
    try {
      const session = JSON.parse(stored) as StoredStudySession;
      const queued = queueSession?.entries[queueSession.position];
      if (!queued || queued.card_id !== session.card.card_id) {
        await loadCard();
        return;
      }
      const current = await api.getStudyCard(session.card.card_id);
      card = current;
      response = session.response;
      responseDurationMs = session.responseDurationMs;
      if (
        current.card_content_version === session.card.card_content_version &&
        current.schedule_version === session.card.schedule_version &&
        current.card_content_version === queued.card_content_version &&
        current.schedule_version === queued.schedule_version &&
        !current.suspended
      ) {
        reveal = session.reveal;
        result = session.result;
        completionKind = session.completionKind;
        view = session.view;
      } else {
        reveal = null;
        result = null;
        completionKind = null;
        await reconcileQueue();
        if (!queueSession) return;
        await loadCard();
        return;
      }
      promptStartedAt = performance.now();
      if (view === "prompt") await focusAnswer();
    } catch (error) {
      fail(error, "prompt", "load");
    }
  }

  async function loadCard(): Promise<void> {
    resetStudyKeyboardState();
    view = "loading";
    errorMessage = "";
    sessionNotice = "";
    audioNotice = "";
    undoNotice = "";
    reveal = null;
    result = null;
    completionKind = null;
    response = "";
    responseDurationMs = 0;
    pendingUndoEventId = null;
    hintVisible = false;
    try {
      await reconcileQueue();
      if (!queueSession) return;
      for (let attempt = 0; attempt < 2; attempt += 1) {
        const queued = queueSession?.entries[queueSession.position];
        if (!queued) {
          finishQueue();
          return;
        }
        try {
          const current = await api.getStudyCard(queued.card_id);
          if (
            current.card_content_version === queued.card_content_version &&
            current.schedule_version === queued.schedule_version &&
            !current.suspended
          ) {
            card = current;
            break;
          }
        } catch (error) {
          if (attempt > 0) throw error;
        }
        await reconcileQueue();
        if (!queueSession) return;
      }
      if (!card) {
        throw new Error("The study queue changed while it was loading.");
      }
      promptAudioAutoplayPending = true;
      view = "prompt";
      promptStartedAt = performance.now();
      await focusAnswer();
    } catch (error) {
      fail(error, "prompt", "load");
    }
  }

  async function focusAnswer(): Promise<void> {
    await tick();
    answerInput?.focus();
  }

  async function autoplayFirstPromptAudio(
    playAudio: () => Promise<void>,
  ): Promise<void> {
    if (!autoplayPromptAudio) return;
    try {
      await playAudio();
    } catch {
      audioNotice =
        "Prompt audio could not start automatically. Use Play to hear it.";
    }
  }

  function promptAudioReady(playAudio: () => Promise<void>): void {
    if (!promptAudioAutoplayPending) return;
    promptAudioAutoplayPending = false;
    void autoplayFirstPromptAudio(playAudio);
  }

  function togglePromptAudioAutoplay(): void {
    autoplayPromptAudio = !autoplayPromptAudio;
    writePromptAudioAutoplay(autoplayPromptAudio);
  }

  async function checkAnswer(): Promise<void> {
    if (!card || composing || view !== "prompt") return;
    responseDurationMs ||= Math.min(
      4_294_967_295,
      Math.max(0, Math.round(performance.now() - promptStartedAt)),
    );
    view = "checking";
    errorMessage = "";
    try {
      reveal = await api.checkAnswer({
        card_id: card.card_id,
        card_content_version: card.card_content_version,
        schedule_version: card.schedule_version,
        raw_response: response,
      });
      view = "revealed";
      vimMode = "normal";
    } catch (error) {
      fail(error, "prompt", "check");
    }
  }

  async function grade(chosenGrade: GradeDto): Promise<void> {
    if (!card || !reveal || !queueSession || view !== "revealed") return;
    pendingGrade = chosenGrade;
    const pendingReview: GradeReviewRequest = queueSession.pendingReview ?? {
      review_event_id: crypto.randomUUID(),
      card_id: card.card_id,
      card_content_version: card.card_content_version,
      schedule_version: card.schedule_version,
      raw_response: reveal.raw_response,
      chosen_grade: chosenGrade,
      response_duration_ms: responseDurationMs,
    };
    queueSession = { ...queueSession, pendingReview };
    writeStudyQueue(queueSession);
    view = "committing";
    errorMessage = "";
    try {
      result = await api.gradeReview(pendingReview);
      card = {
        ...card,
        schedule_version: result.schedule_version,
        due_at: result.due_at,
        completed_reviews: card.completed_reviews + 1,
      };
      completionKind = "graded";
      queueSession = completePendingReview(
        queueSession,
        result.review_event_id,
      );
      writeStudyQueue(queueSession);
      view = "next";
      vimMode = "normal";
    } catch (error) {
      fail(error, "revealed", "grade");
    }
  }

  async function suspendCard(): Promise<void> {
    if (!card || (view !== "prompt" && view !== "revealed")) return;
    const origin = view;
    view = "committing";
    errorMessage = "";
    try {
      card = await api.suspendCard({
        card_id: card.card_id,
        card_content_version: card.card_content_version,
        schedule_version: card.schedule_version,
      });
      result = null;
      completionKind = "suspended";
      advanceStudyQueue();
      view = "next";
      vimMode = "normal";
    } catch (error) {
      fail(error, origin, "suspend");
    }
  }

  async function undoReview(): Promise<void> {
    if (!card || !result || completionKind !== "graded" || view !== "next")
      return;
    view = "committing";
    pendingUndoEventId ??= crypto.randomUUID();
    errorMessage = "";
    try {
      const undone = await api.undoReview({
        undo_event_id: pendingUndoEventId,
        card_id: card.card_id,
        card_content_version: card.card_content_version,
        schedule_version: card.schedule_version,
        review_event_id: result.review_event_id,
      });
      card = {
        ...card,
        schedule_version: undone.schedule_version,
        due_at: undone.due_at,
        completed_reviews: undone.completed_reviews,
      };
      reveal = null;
      result = null;
      response = "";
      responseDurationMs = 0;
      pendingUndoEventId = null;
      completionKind = null;
      resetStudyKeyboardState();
      restoreStudyQueueCard();
      undoNotice = "Last review undone. The card is back in the queue.";
      view = "prompt";
      promptStartedAt = performance.now();
      await focusAnswer();
    } catch (error) {
      fail(error, "next", "undo");
    }
  }

  function advanceStudyQueue(): void {
    if (!queueSession || !card) return;
    if (queueSession.entries[queueSession.position]?.card_id !== card.card_id)
      return;
    queueSession = {
      ...queueSession,
      position: Math.min(
        queueSession.entries.length,
        queueSession.position + 1,
      ),
      pendingReview: null,
    };
    writeStudyQueue(queueSession);
  }

  function restoreStudyQueueCard(): void {
    if (!queueSession || !card || queueSession.position === 0) return;
    const previous = queueSession.position - 1;
    if (queueSession.entries[previous]?.card_id !== card.card_id) return;
    const entries = [...queueSession.entries];
    entries[previous] = {
      card_id: card.card_id,
      card_content_version: card.card_content_version,
      schedule_version: card.schedule_version,
    };
    queueSession = {
      ...queueSession,
      entries,
      position: previous,
      pendingReview: null,
    };
    writeStudyQueue(queueSession);
  }

  async function continueStudy(): Promise<void> {
    if (!queueSession && completionKind === "suspended") {
      onQueueComplete?.();
      return;
    }
    if (queueSession && queueSession.position >= queueSession.entries.length) {
      finishQueue();
      return;
    }
    await loadCard();
  }

  function beginEdit(): void {
    if (!card || !onEdit) return;
    const stableView: StableView =
      view === "revealed" ? "revealed" : view === "next" ? "next" : "prompt";
    const session: StoredStudySession = {
      card,
      reveal,
      result,
      response,
      view: stableView,
      responseDurationMs,
      completionKind,
    };
    sessionStorage.setItem(studySessionKey, JSON.stringify(session));
    onEdit(card.card_id);
  }

  function replayAudio(): void {
    const role = view === "revealed" ? "answer_audio" : "prompt_audio";
    const replay = studyElement?.querySelector<HTMLButtonElement>(
      `[data-media-role="${role}"] button[aria-label="Replay audio"]`,
    );
    if (!replay) {
      audioNotice = "No playable audio is attached to this side of the card.";
      return;
    }
    audioNotice = "";
    replay.click();
  }

  async function retry(): Promise<void> {
    const action = retryAction;
    resetStudyKeyboardState();
    view = recoveryView;
    if (action === "recover") {
      await prepareStudy();
    } else if (action === "load") {
      await loadCard();
    } else if (action === "check") {
      await checkAnswer();
    } else if (action === "grade" && pendingGrade) {
      await grade(pendingGrade);
    } else if (action === "suspend") {
      await suspendCard();
    } else if (action === "undo") {
      await undoReview();
    }
  }

  function finishQueue(): void {
    resetStudyKeyboardState();
    clearStudyQueue();
    clearStudySession();
    queueSession = null;
    onQueueComplete?.();
  }

  function localDayBounds(
    now: Date,
    boundaryMinutes: number,
  ): { start: SvelteDate; end: SvelteDate } {
    const start = new SvelteDate(now);
    start.setHours(0, boundaryMinutes, 0, 0);
    if (now.getTime() < start.getTime()) {
      start.setDate(start.getDate() - 1);
    }
    const end = new SvelteDate(start);
    end.setDate(end.getDate() + 1);
    return { start, end };
  }

  function handleAnswerKeydown(event: KeyboardEvent): void {
    trackStudyKeyboardKeydown(event);
    if (event.key === "Escape" && vimKeybindingsEnabled) {
      if (event.isComposing || composing) return;
      event.preventDefault();
      answerInput?.blur();
      vimMode = "normal";
      return;
    }
    if (event.key === "Enter" && !event.isComposing && !composing) {
      event.preventDefault();
      void checkAnswer();
    }
  }

  function trackStudyKeyboardKeydown(event: KeyboardEvent): void {
    if (!studyVisualKeyboardEnabled || !event.code) return;
    if (!studyKeyboardPressedCodes.includes(event.code)) {
      studyKeyboardPressedCodes = [...studyKeyboardPressedCodes, event.code];
    }
  }

  function releaseStudyKeyboardKey(event: KeyboardEvent): void {
    studyKeyboardPressedCodes = studyKeyboardPressedCodes.filter(
      (code) => code !== event.code,
    );
  }

  function startStudyKeyboardComposition(): void {
    composing = true;
  }

  function endStudyKeyboardComposition(): void {
    composing = false;
  }

  function resetStudyKeyboardState(): void {
    studyKeyboardPressedCodes = [];
    composing = false;
  }

  function primaryLanguage(languageTag: string | null | undefined): string {
    if (!languageTag?.trim()) return "";
    try {
      return new Intl.Locale(languageTag).language.toLowerCase();
    } catch {
      return "";
    }
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    const key = event.key.toLowerCase();
    if (
      (event.metaKey || event.ctrlKey) &&
      key === "z" &&
      view === "next" &&
      result
    ) {
      if (!vimCommandAllowed(event, true, composing, true)) return;
      event.preventDefault();
      void undoReview();
      return;
    }

    if (!vimCommandAllowed(event, true, composing)) return;
    if (
      key === "e" &&
      (view === "prompt" || view === "revealed" || view === "next")
    ) {
      event.preventDefault();
      beginEdit();
      return;
    }
    if (key === "s" && (view === "prompt" || view === "revealed")) {
      event.preventDefault();
      void suspendCard();
      return;
    }
    if (key === "r" && (view === "prompt" || view === "revealed")) {
      event.preventDefault();
      replayAudio();
      return;
    }
    if (event.key === "Enter" && view === "revealed" && reveal) {
      event.preventDefault();
      void grade(reveal.suggested_grade);
      return;
    }
    if (event.key === "Enter" && view === "next") {
      event.preventDefault();
      void continueStudy();
      return;
    }
    if (view === "revealed") {
      const chosenGrade = grades[Number(event.key) - 1];
      if (chosenGrade) {
        event.preventDefault();
        void grade(chosenGrade);
        return;
      }
    }

    if (!vimKeybindingsEnabled) return;
    if (key === "i" && view === "prompt") {
      event.preventDefault();
      void focusAnswer();
    } else if (
      key === "u" &&
      view === "next" &&
      completionKind === "graded" &&
      result
    ) {
      event.preventDefault();
      void undoReview();
    } else if (event.key === "Enter") {
      if (view === "prompt") {
        event.preventDefault();
        void checkAnswer();
      }
    }
  }

  function fail(error: unknown, resume: StableView, action: RetryAction): void {
    errorMessage = error instanceof Error ? error.message : String(error);
    recoveryView = resume;
    retryAction = action;
    view = "error";
    vimMode = "normal";
  }

  function previewFor(grade: GradeDto): GradePreviewDto | undefined {
    return reveal?.grade_previews.find((preview) => preview.grade === grade);
  }

  function formatInterval(seconds: number): string {
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3_600) return `${Math.max(1, Math.round(seconds / 60))}m`;
    if (seconds < 86_400) return `${Math.max(1, Math.round(seconds / 3_600))}h`;
    return `${Math.max(1, Math.round(seconds / 86_400))}d`;
  }

  function formatDueDate(value: string): string {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(value));
  }
</script>

<svelte:window
  onkeydown={handleWindowKeydown}
  onkeyup={releaseStudyKeyboardKey}
  onblur={() => (studyKeyboardPressedCodes = [])}
/>

<section
  bind:this={studyElement}
  class="screen study-screen"
  aria-labelledby="study-title"
>
  <header class="screen-header">
    <div>
      <span class="eyebrow">Focused recall</span>
      <h1 id="study-title" class="screen-title">{messages.study}</h1>
      <p class="screen-description">
        Type the missing text before revealing the answer.
      </p>
    </div>
    <div class="study-header-actions">
      {#if vimKeybindingsEnabled}
        <span
          class="vim-mode-indicator"
          role="status"
          aria-label={`Vim mode ${vimMode.toUpperCase()}`}
          >{vimMode.toUpperCase()}</span
        >
      {/if}
      <Button
        variant="outline"
        size="sm"
        aria-pressed={autoplayPromptAudio}
        onclick={togglePromptAudioAutoplay}
      >
        Autoplay {autoplayPromptAudio ? "on" : "off"}
      </Button>
      {#if card}
        <span class="review-count">
          {#if queueSession}
            {remainingStudyCards(queueSession)}
            {remainingStudyCards(queueSession) === 1 ? "card" : "cards"} remaining
          {:else}
            {card.completed_reviews}
            {card.completed_reviews === 1 ? "review" : "reviews"} saved
          {/if}
        </span>
      {/if}
    </div>
  </header>

  {#if sessionNotice}
    <Alert.Root role="status" class="mb-5 bg-muted/40">
      <Alert.Title>Study item refreshed</Alert.Title>
      <Alert.Description>{sessionNotice}</Alert.Description>
    </Alert.Root>
  {:else if undoNotice}
    <Alert.Root role="status" class="mb-5">
      <Alert.Title>{undoNotice}</Alert.Title>
    </Alert.Root>
  {:else if audioNotice}
    <Alert.Root role="status" class="mb-5">
      <Alert.Title>{audioNotice}</Alert.Title>
    </Alert.Root>
  {/if}

  {#if view === "loading"}
    <Card.Root class="p-6">
      <div class="state-card" aria-live="polite" aria-busy="true">
        <span class="spinner" aria-hidden="true"></span>
        <p>{messages.loading}</p>
      </div>
    </Card.Root>
  {:else if view === "empty"}
    <Card.Root class="p-6">
      <div class="state-card" aria-live="polite">
        {#if studyAvailability === "empty_collection"}
          <span class="eyebrow">Start your collection</span>
          <h2>Your collection is empty</h2>
          <p>Create a typed cloze from any language or script to begin.</p>
          <Button variant="default" data-primary-action onclick={onCreate}
            >Create a cloze</Button
          >
        {:else}
          <span class="eyebrow">Study complete</span>
          <h2>Nothing is due</h2>
          <p>
            {#if nextDueAt}
              Your next review is due {formatDueDate(nextDueAt)}.
            {:else}
              There are no eligible cards in this deck.
            {/if}
          </p>
          <Button
            variant="default"
            data-primary-action
            onclick={onQueueComplete}>Return to Today</Button
          >
        {/if}
      </div>
    </Card.Root>
  {:else if view === "error"}
    <Card.Root class="p-6">
      <div class="state-card">
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>
            {retryAction === "load" || retryAction === "recover"
              ? messages.collectionError
              : "The study action was not completed"}
          </Alert.Title>
          <Alert.Description>
            <p>{errorMessage}</p>
            <p>Your answer and current review state are still available.</p>
          </Alert.Description>
        </Alert.Root>
        <Button variant="default" onclick={retry}>{messages.retry}</Button>
      </div>
    </Card.Root>
  {:else if card}
    <Card.Root class="study-card overflow-hidden p-0">
      <article
        class="study-content"
        data-testid="study-card"
        aria-busy={view === "checking" || view === "committing"}
      >
        <p
          id="study-prompt"
          class="prompt content-text"
          lang={card.language_tag ?? undefined}
          dir={card.direction}
        >
          {#if reveal}
            {#each reveal.source_segments as segment, index (index)}
              {#if segment.highlighted}
                <mark>{segment.text}</mark>
              {:else}
                {segment.text}
              {/if}
            {/each}
          {:else}
            {card.prompt}
          {/if}
        </p>

        {#if view === "prompt" || view === "checking"}
          <div class="prompt-tools">
            {#if card.hint}
              <Button
                variant="ghost"
                size="sm"
                aria-expanded={hintVisible}
                onclick={() => (hintVisible = !hintVisible)}
                >{hintVisible ? "Hide hint" : "Show hint"}</Button
              >
            {/if}
            <Button variant="ghost" size="sm" onclick={beginEdit}
              >Edit note</Button
            >
            <Button variant="ghost" size="sm" onclick={suspendCard}
              >Suspend</Button
            >
          </div>
          {#if hintVisible && card.hint}
            <p
              class="hint content-text"
              lang={card.hint.language_tag ?? undefined}
              dir={card.hint.direction}
            >
              {card.hint.value}
            </p>
          {/if}
          {#each card.prompt_media as media (media.id)}
            <MediaFrame
              kind={media.kind}
              label={media.original_file_name ??
                media.alt_text ??
                "Prompt audio"}
              role={media.role}
              availability={media.availability}
              source={mediaAssetSource(media)}
              contentHash={media.content_hash}
              mediaType={media.media_type}
              altText={media.alt_text}
              width={media.width}
              height={media.height}
              durationMs={media.duration_ms}
              onAudioReady={media.id === firstReadyPromptAudioId
                ? promptAudioReady
                : undefined}
            />
          {/each}
          <form
            onsubmit={(event) => {
              event.preventDefault();
              void checkAnswer();
            }}
          >
            <div class="grid gap-2">
              {#if studyFrontAnswerEnabled}
                <div
                  class="study-front-answer"
                  data-testid="study-front-answer"
                >
                  <span class="eyebrow">Expected answer</span>
                  <strong
                    class="content-text"
                    lang={card.language_tag ?? undefined}
                    dir={card.direction}>{card.expected_answer}</strong
                  >
                </div>
              {/if}
              <Label for="answer">{messages.answerLabel}</Label>
              <Input
                bind:ref={answerInput}
                bind:value={response}
                id="answer"
                name="answer"
                autocomplete="off"
                autocapitalize="off"
                spellcheck="false"
                placeholder={messages.answerPlaceholder}
                disabled={view === "checking"}
                aria-describedby="answer-guidance"
                oncompositionstart={startStudyKeyboardComposition}
                oncompositionend={endStudyKeyboardComposition}
                onkeydown={handleAnswerKeydown}
                onkeyup={releaseStudyKeyboardKey}
                onfocus={() => vimKeybindingsEnabled && (vimMode = "insert")}
                onblur={() => vimKeybindingsEnabled && (vimMode = "normal")}
              />
            </div>
            <p id="answer-guidance" class="input-guidance">
              Enter checks. R replays audio, E edits, and S suspends.
            </p>
            <Button
              class="w-full"
              variant="default"
              disabled={view === "checking"}
              type="submit"
              data-primary-action
            >
              {view === "checking" ? messages.checking : messages.checkAnswer}
            </Button>
          </form>
          {#if studyVisualKeyboardEnabled}
            <div
              class="study-visual-keyboard"
              data-testid="study-visual-keyboard"
            >
              <TypingKeyboard
                expectedCode={null}
                expectedCodes={[]}
                pressedCodes={studyKeyboardPressedCodes}
                completedCodes={[]}
                incorrectCode={null}
                sequenceCompleted={false}
                keyLegends={studyKeyboardLegends}
              />
            </div>
          {/if}
        {:else if reveal && (view === "revealed" || view === "committing")}
          <div class="reveal" aria-live="polite">
            <div class="answer-comparison">
              <div>
                <span class="eyebrow">{messages.expectedAnswer}</span>
                <strong
                  class="content-text"
                  lang={card.language_tag ?? undefined}
                  dir={card.direction}>{reveal.expected_answer}</strong
                >
              </div>
              <div>
                <span class="eyebrow">{messages.yourAnswer}</span>
                <strong class="content-text" dir="auto"
                  >{reveal.raw_response || "—"}</strong
                >
              </div>
            </div>
            <div
              class="answer-difference"
              aria-label={messages.answerDifference}
              data-testid="answer-difference"
            >
              <span class="eyebrow">{messages.answerDifference}</span>
              <p class="content-text" dir="auto">
                {#each reveal.difference as segment, index (`${segment.kind}-${index}`)}
                  {#if segment.kind === "delete"}
                    <del>{segment.text}</del>
                  {:else if segment.kind === "insert"}
                    <ins>{segment.text}</ins>
                  {:else}
                    <span>{segment.text}</span>
                  {/if}
                {/each}
              </p>
              {#if reveal.normalized_response !== reveal.raw_response}
                <small>
                  {messages.comparedAs}:
                  <bdi>{reveal.normalized_response || "—"}</bdi>
                </small>
              {/if}
            </div>
            <span
              class:correct={reveal.comparison === "exact" ||
                reveal.comparison === "accepted_variant"}
              class="result-pill"
            >
              {reveal.comparison.replace("_", " ")}
            </span>

            {#if reveal.annotations.length || reveal.explanation || reveal.answer_media.length}
              <div class="supporting-content">
                {#if reveal.annotations.length}
                  <dl class="annotations">
                    {#each reveal.annotations as annotation, index (index)}
                      <div
                        lang={annotation.language_tag ?? undefined}
                        dir={annotation.direction}
                      >
                        <dt>{annotation.label}</dt>
                        <dd>{annotation.value}</dd>
                      </div>
                    {/each}
                  </dl>
                {/if}
                {#if reveal.explanation}
                  <div
                    lang={reveal.explanation.language_tag ?? undefined}
                    dir={reveal.explanation.direction}
                  >
                    <span class="eyebrow">Explanation</span>
                    <LimitedMarkdown value={reveal.explanation.value} />
                  </div>
                {/if}
                {#each reveal.answer_media as media (media.id)}
                  <MediaFrame
                    kind={media.kind}
                    label={media.original_file_name ??
                      media.alt_text ??
                      "Answer media"}
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
                {/each}
              </div>
            {/if}

            <div class="reveal-tools">
              <Button variant="ghost" size="sm" onclick={beginEdit}
                >Edit note</Button
              >
              <Button variant="ghost" size="sm" onclick={suspendCard}
                >Suspend</Button
              >
            </div>

            <fieldset disabled={view === "committing"}>
              <legend>{messages.gradePrompt}</legend>
              <div class="grade-grid">
                {#each grades as gradeValue (gradeValue)}
                  <Button
                    variant={gradeValue === reveal.suggested_grade
                      ? "default"
                      : "outline"}
                    onclick={() => grade(gradeValue)}
                  >
                    <span>{messages[gradeValue]}</span>
                    {#if previewFor(gradeValue)}
                      <small
                        >{formatInterval(
                          previewFor(gradeValue)?.interval_seconds ?? 0,
                        )}</small
                      >
                    {/if}
                  </Button>
                {/each}
              </div>
            </fieldset>
          </div>
        {:else if view === "committing"}
          <div class="state-card" aria-live="polite">
            <span class="spinner" aria-hidden="true"></span>
            <p>Saving the study action…</p>
          </div>
        {:else if view === "next"}
          <div class="complete-state" aria-live="polite">
            <span class="checkmark" aria-hidden="true">✓</span>
            {#if completionKind === "suspended"}
              <h2>Card suspended</h2>
              <p>This card will stay out of the study queue.</p>
              <div class="next-actions">
                <Button variant="outline" onclick={beginEdit}>Edit note</Button>
                <Button variant="default" onclick={continueStudy}
                  >{queueSession &&
                  queueSession.position >= queueSession.entries.length
                    ? "Finish session"
                    : queueSession
                      ? "Continue"
                      : "Return to Today"}</Button
                >
              </div>
            {:else if result}
              <h2>{messages.saved}</h2>
              <p>
                {messages.nextReview}:
                <strong>{formatDueDate(result.due_at)}</strong>
              </p>
              <div class="next-actions">
                <Button variant="outline" onclick={undoReview}
                  >Undo review</Button
                >
                <Button variant="default" onclick={continueStudy}
                  >{queueSession &&
                  queueSession.position >= queueSession.entries.length
                    ? "Finish session"
                    : "Continue"}</Button
                >
              </div>
            {/if}
          </div>
        {/if}
      </article>
    </Card.Root>
  {/if}
</section>

<style>
  .study-screen {
    width: min(100%, 50rem);
  }

  .review-count {
    flex: 0 0 auto;
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  .study-header-actions {
    display: flex;
    flex: 0 0 auto;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
    justify-content: flex-end;
  }

  .vim-mode-indicator {
    padding: 0.2rem 0.4rem;
    border: 1px solid var(--border);
    color: var(--muted-foreground);
    font-family: ui-monospace, "SFMono-Regular", Consolas, monospace;
    font-size: var(--text-xs);
    font-weight: 700;
    letter-spacing: 0.06em;
  }

  .study-content {
    min-height: 27rem;
    padding: clamp(1.5rem, 7vw, 4rem);
  }

  .prompt {
    min-height: 8rem;
    margin: 0 0 1.25rem;
    color: var(--foreground);
    font-size: var(--text-2xl);
    font-weight: 540;
    line-height: 1.6;
    text-align: center;
    text-wrap: balance;
  }

  .prompt mark {
    padding: 0.08em 0.18em;
    border-radius: 0;
    color: var(--primary-foreground);
    background: var(--primary);
    font-weight: 700;
  }

  .prompt-tools,
  .reveal-tools,
  .next-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    justify-content: center;
    margin-bottom: 1rem;
  }

  .hint {
    width: min(100%, 38rem);
    margin: 0 auto 1rem;
    padding: 0.75rem;
    border-radius: var(--radius-lg);
    color: var(--muted-foreground);
    background: var(--muted);
    text-align: center;
  }

  form {
    width: min(100%, 38rem);
    margin: 1.25rem auto 0;
  }

  .study-front-answer {
    display: grid;
    min-width: 0;
    gap: 0.25rem;
    margin-bottom: 0.5rem;
  }

  .study-front-answer strong {
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }

  .study-visual-keyboard {
    min-width: 0;
    width: 100%;
    margin-top: 1.5rem;
  }

  .input-guidance {
    margin: 0.5rem 0 1rem;
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    line-height: 1.5;
  }

  .answer-comparison {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.25rem;
    padding: 1.25rem 0;
    border-block: 1px solid var(--border);
  }

  .answer-comparison strong {
    display: block;
    overflow-wrap: anywhere;
    font-size: var(--text-lg);
    white-space: pre-wrap;
  }

  .answer-difference {
    display: grid;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .answer-difference p {
    margin: 0;
    overflow-wrap: anywhere;
    font-size: var(--text-lg);
    line-height: 1.7;
  }

  .answer-difference del,
  .answer-difference ins {
    padding: 0.08em 0.18em;
    border-radius: var(--radius-sm);
    text-decoration-thickness: 0.08em;
  }

  .answer-difference del {
    color: var(--destructive);
    background: color-mix(in oklch, var(--destructive) 12%, transparent);
  }

  .answer-difference ins {
    color: var(--foreground);
    background: var(--secondary);
    text-decoration: none;
  }

  .answer-difference small {
    color: var(--muted-foreground);
  }

  .result-pill {
    display: inline-flex;
    margin: 1rem 0;
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius-lg);
    color: var(--destructive);
    background: color-mix(in oklch, var(--destructive) 12%, transparent);
    font-size: var(--text-xs);
    font-weight: 800;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .result-pill.correct {
    color: var(--foreground);
    background: var(--secondary);
  }

  .supporting-content {
    display: grid;
    gap: 1rem;
    margin-bottom: 1.25rem;
  }

  .annotations {
    display: grid;
    gap: 0.5rem;
    margin: 0;
  }

  .annotations div {
    padding: 0.75rem;
    border-radius: var(--radius-lg);
    background: var(--muted);
  }

  .annotations dt {
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  .annotations dd {
    margin: 0.25rem 0 0;
  }

  fieldset {
    padding: 0;
    border: 0;
  }

  legend {
    margin-bottom: 0.75rem;
    color: var(--muted-foreground);
    font-size: var(--text-sm);
    font-weight: 650;
  }

  .grade-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 0.5rem;
  }

  .grade-grid small {
    display: block;
    margin-top: 0.15rem;
    opacity: 0.78;
    font-size: 0.72em;
  }

  .complete-state,
  .state-card {
    display: grid;
    gap: 1rem;
    min-height: 16rem;
    text-align: center;
    place-content: center;
  }

  .complete-state h2 {
    margin: 0;
    font-family: var(--font-sans);
    font-size: var(--text-xl);
  }

  .complete-state p,
  .state-card p {
    margin: 0;
    color: var(--muted-foreground);
  }

  .checkmark {
    display: inline-grid;
    width: 3.4rem;
    height: 3.4rem;
    margin-inline: auto;
    border-radius: var(--radius-lg);
    color: var(--foreground);
    background: var(--secondary);
    font-size: var(--text-xl);
    place-items: center;
  }

  .spinner {
    width: 1.6rem;
    height: 1.6rem;
    margin-inline: auto;
    border: 2px solid var(--border);
    border-top-color: var(--primary);
    border-radius: var(--radius-lg);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width: 560px) {
    .study-content {
      min-height: 25rem;
      padding: 1.5rem 1.25rem;
    }

    .prompt {
      min-height: 7rem;
    }

    .answer-comparison,
    .grade-grid {
      grid-template-columns: 1fr 1fr;
    }
  }
</style>
