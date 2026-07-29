<script lang="ts">
  import { onMount, tick } from "svelte";

  import { api } from "../lib/api";
  import Button from "../lib/components/Button.svelte";
  import Feedback from "../lib/components/Feedback.svelte";
  import Field from "../lib/components/Field.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";
  import TextInput from "../lib/components/TextInput.svelte";
  import type { GradeDto } from "../lib/generated/GradeDto";
  import type { GradeReviewResultDto } from "../lib/generated/GradeReviewResultDto";
  import type { RevealDto } from "../lib/generated/RevealDto";
  import type { StudyCardDto } from "../lib/generated/StudyCardDto";
  import { messages } from "../lib/messages";

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
  let answerInput: HTMLInputElement | undefined;

  onMount(loadCard);

  async function loadCard(): Promise<void> {
    view = "loading";
    errorMessage = "";
    reveal = null;
    result = null;
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
      return;
    }
    const chosenGrade = grades[event.key];
    if (!chosenGrade) return;
    event.preventDefault();
    void grade(chosenGrade);
  }

  function showError(error: unknown): void {
    errorMessage = error instanceof Error ? error.message : String(error);
    view = "error";
  }

  function formatDueDate(value: string): string {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(value));
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<section class="screen study-screen" aria-labelledby="study-title">
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
        {card.completed_reviews}
        {card.completed_reviews === 1 ? "review" : "reviews"} saved
      </span>
    {/if}
  </header>

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
        <Feedback tone="error" title={messages.collectionError}>
          <p>{errorMessage}</p>
        </Feedback>
        <Button variant="primary" onclick={loadCard}>{messages.retry}</Button>
      </div>
    </SurfaceCard>
  {:else if card}
    <SurfaceCard class="study-card" padding="none">
      <article class="study-content" data-testid="study-card">
        <p
          id="study-prompt"
          class="prompt content-text"
          lang={card.language_tag ?? undefined}
          dir={card.direction}
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
              Enter checks your answer. Input method composition is preserved.
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
                  <Button
                    variant={gradeValue === reveal.suggested_grade
                      ? "primary"
                      : "secondary"}
                    shortcut={String(index + 1)}
                    onclick={() => grade(gradeValue)}
                  >
                    {messages[gradeValue]}
                  </Button>
                {/each}
              </div>
            </fieldset>
          </div>
        {:else if result && view === "complete"}
          <div class="complete-state" aria-live="polite">
            <span class="checkmark" aria-hidden="true">✓</span>
            <h2>{messages.saved}</h2>
            <p>
              {messages.nextReview}:
              <strong>{formatDueDate(result.due_at)}</strong>
            </p>
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
    margin: 0 0 clamp(var(--space-7), 6vw, var(--space-10));
    color: var(--color-text);
    font-size: var(--text-2xl);
    font-weight: 540;
    line-height: 1.6;
    text-align: center;
    text-wrap: balance;
  }

  form {
    width: min(100%, 38rem);
    margin-inline: auto;
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
  }

  .result-pill {
    display: inline-flex;
    margin: var(--space-4) 0 var(--space-6);
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
