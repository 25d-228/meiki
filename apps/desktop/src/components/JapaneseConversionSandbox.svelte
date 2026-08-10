<script lang="ts">
  import RiRestartLine from "remixicon-svelte/icons/restart-line";

  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import TypingKeyboard from "./TypingKeyboard.svelte";

  type ConversionStep = "reading" | "convert" | "accept" | "accepted";

  let step = $state<ConversionStep>("reading");
  let pressedCodes = $state<string[]>([]);
  let physicalTrail = $state<string[]>([]);
  let compositionText = $state("");
  let committedOutput = $state("");
  let inputValue = $state("");
  let composing = $state(false);
  let acceptRequested = $state(false);
  let liveStatus = $state("Type a reading with Japanese romaji input.");

  let expectedCode = $derived(
    step === "convert" ? "Space" : step === "accept" ? "Enter" : null,
  );
  let completedCodes = $derived(
    step === "accepted"
      ? ["Space", "Enter"]
      : step === "accept"
        ? ["Space"]
        : [],
  );

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
      CapsLock: "Caps Lock",
      Enter: "Enter",
      Minus: "-",
      ShiftLeft: "Shift",
      ShiftRight: "Shift",
      Space: "Space",
    };
    return labels[code] ?? code;
  }

  function setPressed(code: string): void {
    if (!pressedCodes.includes(code)) pressedCodes = [...pressedCodes, code];
  }

  function handleKeyDown(event: KeyboardEvent): void {
    if (!acceptsPhysicalCode(event.code)) return;
    setPressed(event.code);
    if (event.repeat) return;
    physicalTrail = [...physicalTrail, event.code];

    if (event.code === "Space" && step === "convert") {
      step = "accept";
      liveStatus =
        "Conversion requested. Press Enter to accept the current IME candidate.";
      return;
    }

    if (event.code !== "Enter" || step !== "accept") return;
    if (event.isComposing || composing) {
      acceptRequested = true;
      liveStatus =
        "Enter is accepting the active composition. No candidate is scored.";
      return;
    }
    event.preventDefault();
    acceptCandidate();
  }

  function handleKeyUp(event: KeyboardEvent): void {
    pressedCodes = pressedCodes.filter((code) => code !== event.code);
  }

  function handleInput(event: Event): void {
    inputValue = (event.currentTarget as HTMLInputElement).value;
    if (!composing) committedOutput = inputValue;
    if (step === "reading" && inputValue) {
      step = "convert";
      liveStatus = "Reading entered. Press Space to convert.";
    }
  }

  function handleCompositionStart(event: CompositionEvent): void {
    composing = true;
    compositionText = event.data;
    liveStatus = "Composition started. No candidate is scored.";
  }

  function handleCompositionUpdate(event: CompositionEvent): void {
    compositionText = event.data;
    if (step === "reading" && event.data) step = "convert";
    liveStatus =
      step === "convert"
        ? `Composing ${event.data || "text"}. Press Space to convert when the reading is ready.`
        : `Composing ${event.data || "text"}. Press Enter to accept the current candidate.`;
  }

  function handleCompositionEnd(event: CompositionEvent): void {
    composing = false;
    compositionText = "";
    inputValue = (event.currentTarget as HTMLInputElement).value;
    committedOutput = inputValue;
    if (acceptRequested) {
      acceptRequested = false;
      acceptCandidate();
      return;
    }
    if (step === "reading" && committedOutput) step = "convert";
    liveStatus =
      step === "accept"
        ? "Composition committed. Press Enter to accept the current candidate."
        : "Reading committed. Press Space to convert.";
  }

  function acceptCandidate(): void {
    step = "accepted";
    committedOutput = inputValue;
    liveStatus = `Candidate accepted${committedOutput ? `: ${committedOutput}` : ""}. No candidate was scored.`;
  }

  function reset(): void {
    step = "reading";
    pressedCodes = [];
    physicalTrail = [];
    compositionText = "";
    committedOutput = "";
    inputValue = "";
    composing = false;
    acceptRequested = false;
    liveStatus = "Type a reading with Japanese romaji input.";
  }
</script>

<svelte:window onkeyup={handleKeyUp} onblur={() => (pressedCodes = [])} />

<section class="conversion-sandbox" aria-labelledby="conversion-sandbox-title">
  <header>
    <div>
      <Badge variant="secondary">Non-scored sandbox</Badge>
      <h2 id="conversion-sandbox-title">Kana conversion</h2>
    </div>
    <p>
      Candidate order varies by operating system, dictionary, and IME history.
      No candidate is scored.
    </p>
  </header>

  <ol class="conversion-steps">
    <li data-active={step === "reading"}>Type the reading.</li>
    <li data-active={step === "convert"}>Press Space to convert.</li>
    <li data-active={step === "accept"}>Press Enter to accept.</li>
  </ol>

  <dl class="conversion-details">
    <div>
      <dt>Physical keys</dt>
      <dd data-testid="conversion-physical-trail">
        {physicalTrail.length > 0
          ? physicalTrail.map(codeLabel).join(" → ")
          : "None yet"}
      </dd>
    </div>
    <div>
      <dt>Composition</dt>
      <dd class="content-text" data-testid="conversion-composition">
        {compositionText || "None"}
      </dd>
    </div>
    <div>
      <dt>Committed output</dt>
      <dd class="content-text" data-testid="conversion-committed-output">
        {committedOutput || "None"}
      </dd>
    </div>
  </dl>

  <TypingKeyboard
    {expectedCode}
    expectedCodes={["Space", "Enter"]}
    {pressedCodes}
    {completedCodes}
    incorrectCode={null}
    sequenceCompleted={step === "accepted"}
    keyLegends={{}}
  />

  <div class="field conversion-input">
    <Label for="conversion-sandbox-input">Conversion sandbox input</Label>
    <Input
      id="conversion-sandbox-input"
      value={inputValue}
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      aria-describedby="conversion-sandbox-hint conversion-sandbox-status"
      onkeydown={handleKeyDown}
      oninput={handleInput}
      oncompositionstart={handleCompositionStart}
      oncompositionupdate={handleCompositionUpdate}
      oncompositionend={handleCompositionEnd}
    />
    <p id="conversion-sandbox-hint" class="field-description">
      Try any reading. Conversion results depend on your installed Japanese IME.
    </p>
  </div>

  <p
    id="conversion-sandbox-status"
    class="sandbox-status"
    role="status"
    aria-live="polite"
  >
    {liveStatus}
  </p>

  <div>
    <Button variant="outline" onclick={reset}>
      <RiRestartLine data-icon="inline-start" aria-hidden="true" />
      Reset sandbox
    </Button>
  </div>
</section>

<style>
  .conversion-sandbox {
    display: grid;
    min-width: 0;
    gap: 1.25rem;
    padding: clamp(1rem, 3vw, 1.5rem);
    border: 1px solid var(--border);
    background: var(--card);
  }

  .conversion-sandbox header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(14rem, 1fr);
    gap: 1rem;
    align-items: start;
  }

  .conversion-sandbox header > * {
    min-width: 0;
  }

  .conversion-sandbox h2 {
    margin: 0.65rem 0 0;
    font-size: var(--text-lg);
  }

  .conversion-sandbox header p,
  .sandbox-status {
    margin: 0;
    overflow-wrap: anywhere;
    color: var(--muted-foreground);
    font-size: var(--text-sm);
  }

  .conversion-steps {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.5rem;
    margin: 0;
    padding: 0;
    list-style-position: inside;
  }

  .conversion-steps li {
    min-width: 0;
    padding: 0.75rem;
    overflow-wrap: anywhere;
    border: 1px solid var(--border);
    font-size: var(--text-sm);
    font-weight: 700;
  }

  .conversion-steps li[data-active="true"] {
    border-color: var(--primary);
    background: var(--secondary);
  }

  .conversion-details {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    min-width: 0;
    gap: 0.75rem;
    margin: 0;
  }

  .conversion-details div {
    min-width: 0;
    padding: 0.75rem;
    border: 1px solid var(--border);
    background: var(--background);
  }

  .conversion-details dt {
    color: var(--muted-foreground);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  .conversion-details dd {
    margin: 0.35rem 0 0;
    overflow-wrap: anywhere;
    font-size: var(--text-sm);
  }

  .conversion-input {
    max-width: 38rem;
  }

  .sandbox-status {
    min-height: 1.5rem;
    font-weight: 700;
  }

  @media (max-width: 560px) {
    .conversion-sandbox header,
    .conversion-steps,
    .conversion-details {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
