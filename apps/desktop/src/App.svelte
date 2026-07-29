<script lang="ts">
  import { onMount, tick } from "svelte";

  import { api } from "./lib/api";
  import type { GradeDto } from "./lib/generated/GradeDto";
  import type { GradeReviewResultDto } from "./lib/generated/GradeReviewResultDto";
  import type { RevealDto } from "./lib/generated/RevealDto";
  import type { StudyCardDto } from "./lib/generated/StudyCardDto";
  import { messages } from "./lib/messages";

  type ViewState =
    | "loading"
    | "prompt"
    | "checking"
    | "revealed"
    | "committing"
    | "complete"
    | "error";

  let view: ViewState = "loading";
  let card: StudyCardDto | null = null;
  let reveal: RevealDto | null = null;
  let result: GradeReviewResultDto | null = null;
  let response = "";
  let errorMessage = "";
  let composing = false;
  let answerInput: HTMLInputElement;

  onMount(loadCard);

  async function loadCard(): Promise<void> {
    view = "loading";
    errorMessage = "";
    try {
      card = await api.initializeCollection();
      view = "prompt";
      await tick();
      answerInput?.focus();
    } catch (error) {
      showError(error);
    }
  }

  async function checkAnswer(): Promise<void> {
    if (!card || composing || view !== "prompt") return;
    view = "checking";
    try {
      reveal = await api.checkAnswer({
        card_id: card.card_id,
        card_content_version: card.card_content_version,
        schedule_version: card.schedule_version,
        raw_response: response,
      });
      view = "revealed";
    } catch (error) {
      showError(error);
    }
  }

  async function grade(chosenGrade: GradeDto): Promise<void> {
    if (!card || !reveal || view !== "revealed") return;
    view = "committing";
    try {
      result = await api.gradeReview({
        card_id: card.card_id,
        card_content_version: card.card_content_version,
        schedule_version: card.schedule_version,
        raw_response: reveal.raw_response,
        chosen_grade: chosenGrade,
      });
      card = {
        ...card,
        schedule_version: result.schedule_version,
        due_at: result.due_at,
        completed_reviews: card.completed_reviews + 1,
      };
      view = "complete";
    } catch (error) {
      showError(error);
    }
  }

  function handleAnswerKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter" && !event.isComposing && !composing) {
      event.preventDefault();
      void checkAnswer();
    }
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    if (
      view !== "revealed" ||
      composing ||
      event.isComposing ||
      event.target instanceof HTMLInputElement
    )
      return;
    const grades: Partial<Record<string, GradeDto>> = {
      "1": "again",
      "2": "hard",
      "3": "good",
      "4": "easy",
    };
    if (event.key === "Enter" && reveal) {
      event.preventDefault();
      void grade(reveal.suggested_grade);
    } else {
      const chosenGrade = grades[event.key];
      if (!chosenGrade) return;
      event.preventDefault();
      void grade(chosenGrade);
    }
  }

  function showError(error: unknown): void {
    errorMessage = error instanceof Error ? error.message : String(error);
    view = "error";
  }

  function gradeLabel(grade: GradeDto): string {
    return messages[grade];
  }

  function formatDueDate(value: string): string {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(value));
  }

  function direction(value: StudyCardDto["direction"]): "auto" | "ltr" | "rtl" {
    return value;
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<header class="app-header">
  <div>
    <span class="wordmark">{messages.appName}</span>
    <span class="tagline">{messages.appTagline}</span>
  </div>
  <span class="local-status">
    <span aria-hidden="true" class="status-dot"></span>
    {messages.localOnly}
  </span>
</header>

<main>
  {#if view === "loading"}
    <section class="state-card" aria-live="polite">
      <div class="spinner" aria-hidden="true"></div>
      <p>{messages.loading}</p>
    </section>
  {:else if view === "error"}
    <section class="state-card error-state" role="alert">
      <p>{errorMessage}</p>
      <button class="primary-button" onclick={loadCard}>{messages.retry}</button
      >
    </section>
  {:else if card}
    <section class="study-shell" aria-labelledby="study-prompt">
      <div class="session-meta">
        <span>Study</span>
        <span
          >{card.completed_reviews}
          {card.completed_reviews === 1 ? "review" : "reviews"} saved</span
        >
      </div>

      <article class="study-card">
        <p
          id="study-prompt"
          class="prompt"
          lang={card.language_tag ?? undefined}
          dir={direction(card.direction)}
        >
          {reveal ? reveal.full_source : card.prompt}
        </p>

        {#if view === "prompt" || view === "checking"}
          <form
            onsubmit={(event) => {
              event.preventDefault();
              void checkAnswer();
            }}
          >
            <label for="answer">{messages.answerLabel}</label>
            <input
              bind:this={answerInput}
              bind:value={response}
              id="answer"
              name="answer"
              autocomplete="off"
              autocapitalize="off"
              spellcheck="false"
              placeholder={messages.answerPlaceholder}
              disabled={view === "checking"}
              oncompositionstart={() => (composing = true)}
              oncompositionend={() => (composing = false)}
              onkeydown={handleAnswerKeydown}
            />
            <button
              class="primary-button"
              disabled={view === "checking"}
              type="submit"
            >
              {view === "checking" ? "Checking…" : messages.checkAnswer}
              <kbd>↵</kbd>
            </button>
          </form>
        {:else if reveal && (view === "revealed" || view === "committing")}
          <div class="reveal" aria-live="polite">
            <div class="answer-comparison">
              <div>
                <span class="eyebrow">{messages.expectedAnswer}</span>
                <strong>{reveal.expected_answer}</strong>
              </div>
              <div>
                <span class="eyebrow">{messages.yourAnswer}</span>
                <strong>{reveal.raw_response || "—"}</strong>
              </div>
            </div>
            <span
              class:correct={reveal.comparison === "exact" ||
                reveal.comparison === "accepted_variant"}
              class="result-pill"
            >
              {reveal.comparison.replace("_", " ")}
            </span>
            <fieldset disabled={view === "committing"}>
              <legend>{messages.gradePrompt}</legend>
              <div class="grade-grid">
                {#each ["again", "hard", "good", "easy"] as GradeDto[] as gradeValue, index (gradeValue)}
                  <button
                    class:suggested={gradeValue === reveal.suggested_grade}
                    class="grade-button"
                    onclick={() => grade(gradeValue)}
                    type="button"
                  >
                    <kbd>{index + 1}</kbd>
                    {gradeLabel(gradeValue)}
                  </button>
                {/each}
              </div>
            </fieldset>
          </div>
        {:else if result && view === "complete"}
          <div class="complete-state" aria-live="polite">
            <div class="checkmark" aria-hidden="true">✓</div>
            <h1>{messages.saved}</h1>
            <p>
              {messages.nextReview}:
              <strong>{formatDueDate(result.due_at)}</strong>
            </p>
          </div>
        {/if}
      </article>
    </section>
  {/if}
</main>
