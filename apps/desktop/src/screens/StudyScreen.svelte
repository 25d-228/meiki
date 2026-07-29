<script lang="ts">
  import { onMount, tick } from "svelte";

  import { api } from "../lib/api";
  import Button from "../lib/components/Button.svelte";
  import Feedback from "../lib/components/Feedback.svelte";
  import Field from "../lib/components/Field.svelte";
  import LimitedMarkdown from "../lib/components/LimitedMarkdown.svelte";
  import MediaFrame from "../lib/components/MediaFrame.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";
  import TextInput from "../lib/components/TextInput.svelte";
  import type { GradeDto } from "../lib/generated/GradeDto";
  import type { GradePreviewDto } from "../lib/generated/GradePreviewDto";
  import type { GradeReviewResultDto } from "../lib/generated/GradeReviewResultDto";
  import type { RevealDto } from "../lib/generated/RevealDto";
  import type { StudyCardDto } from "../lib/generated/StudyCardDto";
  import { mediaAssetSource } from "../lib/media";
  import { messages } from "../lib/messages";
  import {
    clearStudyQueue,
    readStudyQueue,
    remainingStudyCards,
    writeStudyQueue,
    type StudyQueueSession,
  } from "../lib/study-queue";

  type Props = {
    onEdit?: (cardId: string) => void;
    onQueueComplete?: () => void;
  };

  type ViewState =
    | "loading"
    | "prompt"
    | "checking"
    | "revealed"
    | "committing"
    | "next"
    | "error";
  type StableView = "prompt" | "revealed" | "next";
  type RetryAction = "load" | "check" | "grade" | "suspend" | "undo";
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

  const sessionKey = "meiki-active-study-session";
  const autoplayKey = "meiki-autoplay-prompt-audio";
  const grades: GradeDto[] = ["again", "hard", "good", "easy"];

  let { onEdit, onQueueComplete }: Props = $props();
  let view = $state<ViewState>("loading");
  let recoveryView = $state<StableView>("prompt");
  let retryAction = $state<RetryAction>("load");
  let pendingGrade = $state<GradeDto | null>(null);
  let pendingReviewEventId = $state<string | null>(null);
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
  let answerInput = $state<HTMLInputElement | undefined>();
  let studyElement = $state<HTMLElement | undefined>();
  let promptStartedAt = $state(0);
  let autoplayPromptAudio = $state(false);
  let queueSession = $state<StudyQueueSession | null>(null);

  onMount(() => {
    autoplayPromptAudio = localStorage.getItem(autoplayKey) === "true";
    queueSession = readStudyQueue();
    if (queueSession && queueSession.position >= queueSession.cardIds.length) {
      clearStudyQueue();
      queueSession = null;
      onQueueComplete?.();
      return;
    }
    void restoreOrLoad();
  });

  async function restoreOrLoad(): Promise<void> {
    const stored = sessionStorage.getItem(sessionKey);
    sessionStorage.removeItem(sessionKey);
    if (!stored) {
      await loadCard();
      return;
    }

    view = "loading";
    try {
      const session = JSON.parse(stored) as StoredStudySession;
      const current = await api.getStudyCard(session.card.card_id);
      card = current;
      response = session.response;
      responseDurationMs = session.responseDurationMs;
      if (
        current.card_content_version === session.card.card_content_version &&
        current.schedule_version === session.card.schedule_version
      ) {
        reveal = session.reveal;
        result = session.result;
        completionKind = session.completionKind;
        view = session.view;
      } else {
        reveal = null;
        result = null;
        completionKind = null;
        restoreQueuedCard(session.card.card_id);
        view = "prompt";
        sessionNotice =
          "The note changed in the editor. Your response is preserved; check it again.";
      }
      promptStartedAt = performance.now();
      if (view === "prompt") await focusAnswer();
    } catch (error) {
      fail(error, "prompt", "load");
    }
  }

  async function loadCard(): Promise<void> {
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
    pendingReviewEventId = null;
    pendingUndoEventId = null;
    hintVisible = false;
    try {
      const queuedCardId = queueSession?.cardIds[queueSession.position];
      card = queuedCardId
        ? await api.getStudyCard(queuedCardId)
        : await api.initializeCollection();
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
    } catch (error) {
      fail(error, "prompt", "check");
    }
  }

  async function grade(chosenGrade: GradeDto): Promise<void> {
    if (!card || !reveal || view !== "revealed") return;
    pendingGrade = chosenGrade;
    pendingReviewEventId ??= crypto.randomUUID();
    view = "committing";
    errorMessage = "";
    try {
      result = await api.gradeReview({
        review_event_id: pendingReviewEventId,
        card_id: card.card_id,
        card_content_version: card.card_content_version,
        schedule_version: card.schedule_version,
        raw_response: reveal.raw_response,
        chosen_grade: chosenGrade,
        response_duration_ms: responseDurationMs,
      });
      card = {
        ...card,
        schedule_version: result.schedule_version,
        due_at: result.due_at,
        completed_reviews: card.completed_reviews + 1,
      };
      completionKind = "graded";
      advanceStudyQueue();
      view = "next";
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
      pendingReviewEventId = null;
      pendingUndoEventId = null;
      completionKind = null;
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
    if (queueSession.cardIds[queueSession.position] !== card.card_id) return;
    queueSession = {
      ...queueSession,
      position: Math.min(
        queueSession.cardIds.length,
        queueSession.position + 1,
      ),
    };
    writeStudyQueue(queueSession);
  }

  function restoreStudyQueueCard(): void {
    if (!queueSession || !card || queueSession.position === 0) return;
    const previous = queueSession.position - 1;
    if (queueSession.cardIds[previous] !== card.card_id) return;
    queueSession = { ...queueSession, position: previous };
    writeStudyQueue(queueSession);
  }

  function restoreQueuedCard(cardId: string): void {
    if (!queueSession) return;
    const cardIndex = queueSession.cardIds.indexOf(cardId);
    if (cardIndex < 0 || queueSession.position !== cardIndex + 1) return;
    queueSession = { ...queueSession, position: cardIndex };
    writeStudyQueue(queueSession);
  }

  async function continueStudy(): Promise<void> {
    if (!queueSession && completionKind === "suspended") {
      onQueueComplete?.();
      return;
    }
    if (queueSession && queueSession.position >= queueSession.cardIds.length) {
      clearStudyQueue();
      queueSession = null;
      onQueueComplete?.();
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
    sessionStorage.setItem(sessionKey, JSON.stringify(session));
    onEdit(card.card_id);
  }

  function replayAudio(): void {
    const role = view === "revealed" ? "answer_audio" : "prompt_audio";
    const audio = studyElement?.querySelector<HTMLAudioElement>(
      `[data-media-role="${role}"] audio`,
    );
    if (!audio) {
      audioNotice = "No playable audio is attached to this side of the card.";
      return;
    }
    audio.currentTime = 0;
    void audio.play().then(
      () => (audioNotice = ""),
      () =>
        (audioNotice =
          "Audio playback was blocked. Use the visible audio control to play it."),
    );
  }

  async function retry(): Promise<void> {
    const action = retryAction;
    view = recoveryView;
    if (action === "load") {
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

  function handleAnswerKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter" && !event.isComposing && !composing) {
      event.preventDefault();
      void checkAnswer();
    }
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    if (composing || event.isComposing) return;
    if (
      (event.metaKey || event.ctrlKey) &&
      event.key.toLowerCase() === "z" &&
      view === "next" &&
      result
    ) {
      event.preventDefault();
      void undoReview();
      return;
    }

    const editable =
      event.target instanceof HTMLInputElement ||
      event.target instanceof HTMLTextAreaElement ||
      event.target instanceof HTMLSelectElement;
    if (editable) return;

    const key = event.key.toLowerCase();
    if (key === "r" && (view === "prompt" || view === "revealed")) {
      event.preventDefault();
      replayAudio();
      return;
    }
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
    if (view === "revealed" && reveal) {
      if (event.key === "Enter") {
        event.preventDefault();
        void grade(reveal.suggested_grade);
        return;
      }
      const chosenGrade = grades[Number(event.key) - 1];
      if (chosenGrade) {
        event.preventDefault();
        void grade(chosenGrade);
      }
    } else if (view === "next" && event.key === "Enter") {
      event.preventDefault();
      void continueStudy();
    }
  }

  function fail(error: unknown, resume: StableView, action: RetryAction): void {
    errorMessage = error instanceof Error ? error.message : String(error);
    recoveryView = resume;
    retryAction = action;
    view = "error";
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

<svelte:window onkeydown={handleWindowKeydown} />

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
  </header>

  {#if sessionNotice}
    <Feedback tone="warning" title="Study item refreshed" compact>
      <p>{sessionNotice}</p>
    </Feedback>
  {:else if undoNotice}
    <Feedback tone="success" title={undoNotice} compact />
  {:else if audioNotice}
    <Feedback tone="info" title={audioNotice} compact />
  {/if}

  {#if view === "loading"}
    <SurfaceCard>
      <div class="state-card" aria-live="polite" aria-busy="true">
        <span class="spinner" aria-hidden="true"></span>
        <p>{messages.loading}</p>
      </div>
    </SurfaceCard>
  {:else if view === "error"}
    <SurfaceCard>
      <div class="state-card">
        <Feedback
          tone="error"
          title={retryAction === "load"
            ? messages.collectionError
            : "The study action was not completed"}
        >
          <p>{errorMessage}</p>
          <p>Your answer and current review state are still available.</p>
        </Feedback>
        <Button variant="primary" onclick={retry}>{messages.retry}</Button>
      </div>
    </SurfaceCard>
  {:else if card}
    <SurfaceCard class="study-card" padding="none">
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
                variant="quiet"
                size="small"
                aria-expanded={hintVisible}
                onclick={() => (hintVisible = !hintVisible)}
                >{hintVisible ? "Hide hint" : "Show hint"}</Button
              >
            {/if}
            <Button
              variant="quiet"
              size="small"
              shortcut="R"
              onclick={replayAudio}>Replay audio</Button
            >
            <Button
              variant="quiet"
              size="small"
              shortcut="E"
              onclick={beginEdit}>Edit note</Button
            >
            <Button
              variant="quiet"
              size="small"
              shortcut="S"
              onclick={suspendCard}>Suspend</Button
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
          {#each card.prompt_media as media, index (media.id)}
            <MediaFrame
              kind={media.kind}
              label={media.original_file_name ??
                media.alt_text ??
                "Prompt audio"}
              role={media.role}
              availability={media.availability}
              source={mediaAssetSource(media.asset_path)}
              mediaType={media.media_type}
              altText={media.alt_text}
              width={media.width}
              height={media.height}
              autoplay={autoplayPromptAudio && index === 0}
            />
          {/each}
          <form
            onsubmit={(event) => {
              event.preventDefault();
              void checkAnswer();
            }}
          >
            <Field id="answer" label={messages.answerLabel}>
              <TextInput
                bind:element={answerInput}
                bind:value={response}
                id="answer"
                name="answer"
                autocomplete="off"
                autocapitalize="off"
                spellcheck="false"
                placeholder={messages.answerPlaceholder}
                disabled={view === "checking"}
                aria-describedby="answer-guidance"
                oncompositionstart={() => (composing = true)}
                oncompositionend={() => (composing = false)}
                onkeydown={handleAnswerKeydown}
              />
            </Field>
            <p id="answer-guidance" class="input-guidance">
              Enter checks. R replays audio, E edits, and S suspends.
            </p>
            <Button
              variant="primary"
              full
              shortcut="↵"
              disabled={view === "checking"}
              type="submit"
              data-primary-action
            >
              {view === "checking" ? messages.checking : messages.checkAnswer}
            </Button>
          </form>
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
                    source={mediaAssetSource(media.asset_path)}
                    mediaType={media.media_type}
                    altText={media.alt_text}
                    width={media.width}
                    height={media.height}
                  />
                {/each}
              </div>
            {/if}

            <div class="reveal-tools">
              <Button
                variant="quiet"
                size="small"
                shortcut="R"
                onclick={replayAudio}>Replay audio</Button
              >
              <Button
                variant="quiet"
                size="small"
                shortcut="E"
                onclick={beginEdit}>Edit note</Button
              >
              <Button
                variant="quiet"
                size="small"
                shortcut="S"
                onclick={suspendCard}>Suspend</Button
              >
            </div>

            <fieldset disabled={view === "committing"}>
              <legend>{messages.gradePrompt}</legend>
              <div class="grade-grid">
                {#each grades as gradeValue, index (gradeValue)}
                  <Button
                    variant={gradeValue === reveal.suggested_grade
                      ? "primary"
                      : "secondary"}
                    shortcut={String(index + 1)}
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
                <Button variant="secondary" shortcut="E" onclick={beginEdit}
                  >Edit note</Button
                >
                <Button variant="primary" shortcut="↵" onclick={continueStudy}
                  >{queueSession &&
                  queueSession.position >= queueSession.cardIds.length
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
                <Button
                  variant="secondary"
                  shortcut="⌘/Ctrl Z"
                  onclick={undoReview}>Undo review</Button
                >
                <Button variant="primary" shortcut="↵" onclick={continueStudy}
                  >{queueSession &&
                  queueSession.position >= queueSession.cardIds.length
                    ? "Finish session"
                    : "Continue"}</Button
                >
              </div>
            {/if}
          </div>
        {/if}
      </article>
    </SurfaceCard>
  {/if}
</section>

<style>
  .study-screen {
    width: min(100%, var(--reading-width));
  }

  .review-count {
    flex: 0 0 auto;
    color: var(--color-text-muted);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  .study-content {
    min-height: 27rem;
    padding: clamp(var(--space-6), 7vw, var(--space-10));
  }

  .prompt {
    min-height: 8rem;
    margin: 0 0 var(--space-5);
    color: var(--color-text);
    font-size: var(--text-2xl);
    font-weight: 540;
    line-height: 1.6;
    text-align: center;
    text-wrap: balance;
  }

  .prompt mark {
    padding: 0.08em 0.18em;
    border-radius: var(--radius-xs);
    color: var(--color-accent-strong);
    background: var(--color-accent-soft);
  }

  .prompt-tools,
  .reveal-tools,
  .next-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    justify-content: center;
    margin-bottom: var(--space-4);
  }

  .hint {
    width: min(100%, 38rem);
    margin: 0 auto var(--space-4);
    padding: var(--space-3);
    border-radius: var(--radius-control);
    color: var(--color-text-muted);
    background: var(--color-surface-muted);
    text-align: center;
  }

  form {
    width: min(100%, 38rem);
    margin: var(--space-5) auto 0;
  }

  .input-guidance {
    margin: var(--space-2) 0 var(--space-4);
    color: var(--color-text-muted);
    font-size: var(--text-xs);
    line-height: 1.5;
  }

  .answer-comparison {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-5);
    padding: var(--space-5) 0;
    border-block: var(--border-width) solid var(--color-border);
  }

  .answer-comparison strong {
    display: block;
    overflow-wrap: anywhere;
    font-size: var(--text-lg);
    white-space: pre-wrap;
  }

  .answer-difference {
    display: grid;
    gap: var(--space-2);
    margin-top: var(--space-4);
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
    border-radius: var(--radius-xs);
    text-decoration-thickness: 0.08em;
  }

  .answer-difference del {
    color: var(--color-danger);
    background: var(--color-danger-soft);
  }

  .answer-difference ins {
    color: var(--color-success);
    background: var(--color-success-soft);
    text-decoration: none;
  }

  .answer-difference small {
    color: var(--color-text-muted);
  }

  .result-pill {
    display: inline-flex;
    margin: var(--space-4) 0;
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-pill);
    color: var(--color-danger);
    background: var(--color-danger-soft);
    font-size: var(--text-xs);
    font-weight: 800;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .result-pill.correct {
    color: var(--color-success);
    background: var(--color-success-soft);
  }

  .supporting-content {
    display: grid;
    gap: var(--space-4);
    margin-bottom: var(--space-5);
  }

  .annotations {
    display: grid;
    gap: var(--space-2);
    margin: 0;
  }

  .annotations div {
    padding: var(--space-3);
    border-radius: var(--radius-control);
    background: var(--color-surface-muted);
  }

  .annotations dt {
    color: var(--color-text-muted);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  .annotations dd {
    margin: var(--space-1) 0 0;
  }

  fieldset {
    padding: 0;
    border: 0;
  }

  legend {
    margin-bottom: var(--space-3);
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    font-weight: 650;
  }

  .grade-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-2);
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
    gap: var(--space-4);
    min-height: 16rem;
    text-align: center;
    place-content: center;
  }

  .complete-state h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-xl);
  }

  .complete-state p,
  .state-card p {
    margin: 0;
    color: var(--color-text-muted);
  }

  .checkmark {
    display: inline-grid;
    width: 3.4rem;
    height: 3.4rem;
    margin-inline: auto;
    border-radius: 50%;
    color: var(--color-success);
    background: var(--color-success-soft);
    font-size: var(--text-xl);
    place-items: center;
  }

  .spinner {
    width: 1.6rem;
    height: 1.6rem;
    margin-inline: auto;
    border: 2px solid var(--color-border);
    border-top-color: var(--color-accent);
    border-radius: 50%;
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
      padding: var(--space-6) var(--space-5);
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
