<script lang="ts">
  import RiArrowRightLine from "remixicon-svelte/icons/arrow-right-line";
  import RiRestartLine from "remixicon-svelte/icons/restart-line";

  import { onMount } from "svelte";

  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import type { InstructionPlatform, TypingLesson } from "$lib/typing-lessons";
  import { type VimMode, vimCommandAllowed } from "$lib/vim-keybindings";
  import TypingKeyboard from "./TypingKeyboard.svelte";

  type PracticeResult = "idle" | "correct" | "incorrect";

  type Props = {
    lesson: TypingLesson;
    platform: InstructionPlatform | null;
    completed: boolean;
    onComplete: (lessonId: string) => void;
    onNext: () => void;
    onPrevious: () => void;
    vimEnabled: boolean;
    onVimModeChange: (mode: VimMode) => void;
  };

  let {
    lesson,
    platform,
    completed,
    onComplete,
    onNext,
    onPrevious,
    vimEnabled,
    onVimModeChange,
  }: Props = $props();
  let expectedCodes = $derived([
    ...lesson.sharedPhysicalCodes,
    ...(platform ? lesson.platformPhysicalCodes[platform] : []),
  ]);
  let expectedIndex = $state(0);
  let pressedCodes = $state<string[]>([]);
  let incorrectCode = $state<string | null>(null);
  let physicalTrail = $state<string[]>([]);
  let compositionText = $state("");
  let committedOutput = $state("");
  let inputValue = $state("");
  let composing = $state(false);
  let result = $state<PracticeResult>("idle");
  let sequenceCompleted = $state(false);
  let feedback = $state("");
  let liveStatus = $state("");
  let inputElement = $state<HTMLInputElement | null>(null);

  let expectedCode = $derived(expectedCodes[expectedIndex] ?? null);
  let completedCodes = $derived(expectedCodes.slice(0, expectedIndex));

  onMount(() => {
    if (completed) {
      expectedIndex = expectedCodes.length;
      result = "correct";
      sequenceCompleted = true;
      feedback = `Completed — ${lesson.target}`;
    }
    liveStatus = initialStatus();
  });

  function acceptsPhysicalCode(code: string): boolean {
    return (
      /^(Key[A-Z]|Digit[0-9])$/.test(code) ||
      [
        "Backquote",
        "Minus",
        "Equal",
        "Backspace",
        "BracketLeft",
        "BracketRight",
        "Backslash",
        "CapsLock",
        "Semicolon",
        "Quote",
        "Enter",
        "ShiftLeft",
        "ShiftRight",
        "Comma",
        "Period",
        "Slash",
        "AltLeft",
        "AltRight",
        "Space",
      ].includes(code)
    );
  }

  function codeLabel(code: string): string {
    if (code.startsWith("Key")) return code.slice(3);
    if (code.startsWith("Digit")) return code.slice(5);
    const labels: Record<string, string> = {
      AltLeft: "Option",
      AltRight: "AltGr",
      Backquote: "`",
      Backslash: "\\",
      BracketLeft: "[",
      BracketRight: "]",
      CapsLock: "Caps Lock",
      Comma: ",",
      Equal: "=",
      Minus: "-",
      Period: ".",
      Quote: "'",
      Semicolon: ";",
      ShiftLeft: "Shift",
      ShiftRight: "Shift",
      Slash: "/",
      Space: "Space",
    };
    return labels[code] ?? code;
  }

  function initialStatus(): string {
    return expectedCodes[0]
      ? `Expected ${codeLabel(expectedCodes[0])}.`
      : `Enter ${lesson.target}, then press Enter to check.`;
  }

  function setPressed(code: string): void {
    if (!pressedCodes.includes(code)) pressedCodes = [...pressedCodes, code];
  }

  function updatePhysicalSequence(
    code: string,
    compositionActive: boolean,
  ): void {
    if (!expectedCode) {
      feedback = "";
      liveStatus = compositionActive
        ? `Composing. Pressed ${codeLabel(code)}. No answer has been checked.`
        : `Pressed ${codeLabel(code)}.`;
      return;
    }
    if (code !== expectedCode) {
      incorrectCode = code;
      result =
        lesson.mode === "committed" && compositionActive ? "idle" : "incorrect";
      feedback = `Pressed ${codeLabel(code)}. Expected ${codeLabel(expectedCode)}. Try again.${compositionActive ? " Composition remains unchecked." : ""}`;
      liveStatus = feedback;
      return;
    }

    incorrectCode = null;
    expectedIndex += 1;
    if (expectedIndex === expectedCodes.length) {
      sequenceCompleted = true;
      if (lesson.mode === "physical") {
        markCorrect();
      } else {
        result = "idle";
        feedback = `Physical sequence complete. Commit the target text.${compositionActive ? " Composition remains unchecked." : ""}`;
        liveStatus = feedback;
      }
      return;
    }
    result = "idle";
    feedback = `Correct position. Next: ${codeLabel(expectedCodes[expectedIndex])}.${compositionActive ? " Composition remains unchecked." : ""}`;
    liveStatus = feedback;
  }

  function handleKeyDown(event: KeyboardEvent): void {
    if (event.key === "Escape" && vimEnabled) {
      if (event.isComposing || composing) return;
      event.preventDefault();
      inputElement?.blur();
      onVimModeChange("normal");
      return;
    }
    if (!acceptsPhysicalCode(event.code)) return;
    setPressed(event.code);
    if (event.repeat) return;

    const compositionActive = event.isComposing || composing;
    if (lesson.mode === "physical" && !compositionActive) {
      event.preventDefault();
    }

    physicalTrail = [...physicalTrail, event.code];
    if (event.code === "Enter" && lesson.mode === "committed") {
      if (compositionActive) {
        updatePhysicalSequence(event.code, true);
        const physicalStatus = feedback || liveStatus;
        feedback = "";
        liveStatus = `${physicalStatus} Enter will only commit the active composition.`;
        return;
      }
      event.preventDefault();
      evaluateCommittedText();
      return;
    }

    updatePhysicalSequence(event.code, compositionActive);
  }

  function handleVimKeydown(event: KeyboardEvent): void {
    if (!vimCommandAllowed(event, vimEnabled, composing)) return;
    const key = event.key.toLowerCase();
    if (key === "h") {
      event.preventDefault();
      onPrevious();
    } else if (key === "l" && result === "correct") {
      event.preventDefault();
      onNext();
    } else if (key === "r") {
      event.preventDefault();
      retry();
    } else if (key === "i") {
      event.preventDefault();
      inputElement?.focus();
    }
  }

  function handleKeyUp(event: KeyboardEvent): void {
    pressedCodes = pressedCodes.filter((code) => code !== event.code);
  }

  function handleInput(event: Event): void {
    inputValue = (event.currentTarget as HTMLInputElement).value;
    if (!composing) committedOutput = inputValue;
  }

  function handleCompositionStart(event: CompositionEvent): void {
    composing = true;
    compositionText = event.data;
    result = "idle";
    feedback = "";
    liveStatus = "Composition started. No answer has been checked.";
  }

  function handleCompositionUpdate(event: CompositionEvent): void {
    compositionText = event.data;
    liveStatus = `Composing ${event.data || "text"}. No answer has been checked.`;
  }

  function handleCompositionEnd(event: CompositionEvent): void {
    composing = false;
    compositionText = "";
    inputValue = (event.currentTarget as HTMLInputElement).value;
    committedOutput = inputValue;
    feedback = "";
    liveStatus = committedOutput
      ? `Committed ${committedOutput}. Press Enter to check.`
      : "Composition ended without committed text.";
  }

  function evaluateCommittedText(): void {
    if (composing) return;
    committedOutput = inputValue;
    if (!committedOutput) {
      result = "incorrect";
      feedback = `Enter ${lesson.target} before checking.`;
      liveStatus = feedback;
      return;
    }
    if (!sameGraphemes(committedOutput, lesson.expectedText)) {
      result = "incorrect";
      feedback = `Not yet — expected ${lesson.target}. Try again.`;
      liveStatus = feedback;
      return;
    }
    sequenceCompleted = true;
    markCorrect();
  }

  function sameGraphemes(actual: string, expected: string): boolean {
    const segmenter = new Intl.Segmenter(undefined, {
      granularity: "grapheme",
    });
    const graphemes = (value: string) =>
      Array.from(
        segmenter.segment(value.normalize("NFC")),
        ({ segment }) => segment,
      );
    const actualGraphemes = graphemes(actual);
    const expectedGraphemes = graphemes(expected);
    return (
      actualGraphemes.length === expectedGraphemes.length &&
      actualGraphemes.every(
        (grapheme, index) => grapheme === expectedGraphemes[index],
      )
    );
  }

  function markCorrect(): void {
    incorrectCode = null;
    result = "correct";
    feedback = `Correct — ${lesson.target}`;
    liveStatus = feedback;
    onComplete(lesson.id);
  }

  function retry(): void {
    expectedIndex = 0;
    pressedCodes = [];
    incorrectCode = null;
    physicalTrail = [];
    compositionText = "";
    committedOutput = "";
    inputValue = "";
    composing = false;
    result = "idle";
    sequenceCompleted = false;
    feedback = "";
    liveStatus = initialStatus();
  }
</script>

<svelte:window
  onkeydown={handleVimKeydown}
  onkeyup={handleKeyUp}
  onblur={() => (pressedCodes = [])}
/>

<section class="practice" aria-labelledby="typing-practice-title">
  <header class="practice-header">
    <div>
      <Badge variant="secondary">
        {lesson.mode === "physical"
          ? "Physical-key drill"
          : "Committed-text drill"}
      </Badge>
      <h2 id="typing-practice-title">{lesson.title}</h2>
    </div>
    <div class="target" aria-label={`Target ${lesson.target}`}>
      <span>Target</span>
      <strong class="content-text" lang={lesson.languageTag}
        >{lesson.target}</strong
      >
    </div>
  </header>

  <dl class="practice-details">
    <div>
      <dt>Expected physical sequence</dt>
      <dd data-testid="typing-expected-sequence">
        {expectedCodes.length > 0
          ? expectedCodes.map(codeLabel).join(" → ")
          : "Varies with your input source"}
      </dd>
    </div>
    <div>
      <dt>Physical keys</dt>
      <dd data-testid="typing-physical-trail">
        {physicalTrail.length > 0
          ? physicalTrail.map(codeLabel).join(" → ")
          : "None yet"}
      </dd>
    </div>
    <div>
      <dt>Composition</dt>
      <dd class="content-text" data-testid="typing-composition">
        {compositionText || "None"}
      </dd>
    </div>
    <div>
      <dt>Committed output</dt>
      <dd class="content-text" data-testid="typing-committed-output">
        {committedOutput || "None"}
      </dd>
    </div>
  </dl>

  <TypingKeyboard
    {expectedCode}
    {expectedCodes}
    {pressedCodes}
    {completedCodes}
    {incorrectCode}
    {sequenceCompleted}
    keyLegends={lesson.keyLegends}
  />

  <div class="practice-input field">
    <Label for="typing-input">Practice input</Label>
    <Input
      bind:ref={inputElement}
      id="typing-input"
      value={inputValue}
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      aria-describedby="typing-hint typing-live-status"
      onkeydown={handleKeyDown}
      oninput={handleInput}
      oncompositionstart={handleCompositionStart}
      oncompositionupdate={handleCompositionUpdate}
      oncompositionend={handleCompositionEnd}
      onfocus={() => vimEnabled && onVimModeChange("insert")}
      onblur={() => vimEnabled && onVimModeChange("normal")}
    />
    <p id="typing-hint" class="field-description">{lesson.hint}</p>
  </div>

  <p
    id="typing-live-status"
    class:correct-feedback={result === "correct"}
    class:incorrect-feedback={result === "incorrect"}
    class="feedback"
    role="status"
    aria-live="polite"
  >
    {feedback || liveStatus}
  </p>

  <div class="practice-actions">
    <Button variant="outline" onclick={retry}>
      <RiRestartLine data-icon="inline-start" aria-hidden="true" />
      Retry
    </Button>
    <Button disabled={result !== "correct"} onclick={onNext}>
      Next
      <RiArrowRightLine data-icon="inline-end" aria-hidden="true" />
    </Button>
  </div>
</section>

<style>
  .practice {
    display: grid;
    min-width: 0;
    gap: 1.25rem;
    padding: clamp(1rem, 3vw, 1.5rem);
    border: 1px solid var(--border);
    background: var(--card);
  }

  .practice-header {
    display: flex;
    min-width: 0;
    gap: 1rem;
    align-items: start;
    justify-content: space-between;
  }

  .practice-header > div {
    min-width: 0;
  }

  .practice-header h2 {
    margin: 0.65rem 0 0;
    font-size: var(--text-lg);
  }

  .target {
    display: grid;
    flex: 0 1 auto;
    min-width: min(6rem, 100%);
    max-width: 100%;
    gap: 0.25rem;
    padding: 0.75rem;
    border: 1px solid var(--border);
    text-align: center;
  }

  .target span,
  .practice-details dt {
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  .target strong {
    min-width: 0;
    overflow-wrap: anywhere;
    font-size: var(--text-2xl);
    line-height: 1.2;
  }

  .practice-details {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    min-width: 0;
    gap: 0.75rem;
    margin: 0;
  }

  .practice-details div {
    min-width: 0;
    padding: 0.75rem;
    border: 1px solid var(--border);
    background: var(--background);
  }

  .practice-details dd {
    margin: 0.35rem 0 0;
    overflow-wrap: anywhere;
    font-size: var(--text-sm);
  }

  .practice-input {
    max-width: 38rem;
  }

  .feedback {
    min-height: 1.5rem;
    margin: 0;
    color: var(--muted-foreground);
    font-size: var(--text-sm);
    font-weight: 700;
  }

  .correct-feedback {
    color: var(--foreground);
  }

  .correct-feedback::before {
    content: "✓ ";
  }

  .incorrect-feedback {
    color: var(--destructive);
  }

  .incorrect-feedback::before {
    content: "! ";
  }

  .practice-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  @media (max-width: 560px) {
    .practice-header {
      display: grid;
    }

    .target {
      justify-self: start;
    }

    .practice-details {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
