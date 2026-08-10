<script lang="ts">
  import type { TypingKeyLegend } from "$lib/typing-lessons";

  type Keycap = {
    code: string;
    latin: string;
    shifted?: string;
    units?: number;
    modifier?: boolean;
  };

  type Props = {
    expectedCode: string | null;
    expectedCodes: string[];
    pressedCodes: string[];
    completedCodes: string[];
    incorrectCode: string | null;
    sequenceCompleted: boolean;
    keyLegends: Record<string, TypingKeyLegend>;
  };

  let {
    expectedCode,
    expectedCodes,
    pressedCodes,
    completedCodes,
    incorrectCode,
    sequenceCompleted,
    keyLegends,
  }: Props = $props();

  const keyboardRows: Array<{ id: string; keys: Keycap[] }> = [
    {
      id: "number",
      keys: [
        { code: "Backquote", latin: "`", shifted: "~" },
        { code: "Digit1", latin: "1", shifted: "!" },
        { code: "Digit2", latin: "2", shifted: "@" },
        { code: "Digit3", latin: "3", shifted: "#" },
        { code: "Digit4", latin: "4", shifted: "$" },
        { code: "Digit5", latin: "5", shifted: "%" },
        { code: "Digit6", latin: "6", shifted: "^" },
        { code: "Digit7", latin: "7", shifted: "&" },
        { code: "Digit8", latin: "8", shifted: "*" },
        { code: "Digit9", latin: "9", shifted: "(" },
        { code: "Digit0", latin: "0", shifted: ")" },
        { code: "Minus", latin: "-", shifted: "_" },
        { code: "Equal", latin: "=", shifted: "+" },
        { code: "Backspace", latin: "Backspace", units: 4 },
      ],
    },
    {
      id: "qwerty",
      keys: [
        { code: "Tab", latin: "Tab", units: 4 },
        ..."QWERTYUIOP".split("").map((letter) => ({
          code: `Key${letter}`,
          latin: letter,
        })),
        { code: "BracketLeft", latin: "[", shifted: "{" },
        { code: "BracketRight", latin: "]", shifted: "}" },
        { code: "Backslash", latin: "\\", shifted: "|" },
      ],
    },
    {
      id: "home",
      keys: [
        { code: "CapsLock", latin: "Caps Lock", units: 4, modifier: true },
        ..."ASDFGHJKL".split("").map((letter) => ({
          code: `Key${letter}`,
          latin: letter,
        })),
        { code: "Semicolon", latin: ";", shifted: ":" },
        { code: "Quote", latin: "'", shifted: '"' },
        { code: "Enter", latin: "Enter", units: 4 },
      ],
    },
    {
      id: "lower",
      keys: [
        { code: "ShiftLeft", latin: "Shift", units: 5, modifier: true },
        ..."ZXCVBNM".split("").map((letter) => ({
          code: `Key${letter}`,
          latin: letter,
        })),
        { code: "Comma", latin: ",", shifted: "<" },
        { code: "Period", latin: ".", shifted: ">" },
        { code: "Slash", latin: "/", shifted: "?" },
        { code: "ShiftRight", latin: "Shift", units: 5, modifier: true },
      ],
    },
    {
      id: "modifiers",
      keys: [
        { code: "AltLeft", latin: "Option", units: 5, modifier: true },
        { code: "Space", latin: "Space", units: 20 },
        { code: "AltRight", latin: "AltGr", units: 5, modifier: true },
      ],
    },
  ];

  function keyMarker(key: Keycap): string {
    if (incorrectCode === key.code) return "!";
    if (sequenceCompleted && expectedCodes.includes(key.code)) return "✓";
    if (expectedCode === key.code) return "→";
    if (completedCodes.includes(key.code)) return "✓";
    if (pressedCodes.includes(key.code)) return "●";
    return "";
  }
</script>

<div class="typing-keyboard" data-testid="typing-keyboard" aria-hidden="true">
  {#each keyboardRows as row (row.id)}
    <div class="keyboard-row" data-keyboard-row={row.id}>
      {#each row.keys as key (key.code)}
        <div
          class="keycap"
          class:keycap-modifier={key.modifier}
          style={`--key-units: ${key.units ?? 2}`}
          data-testid={`typing-key-${key.code}`}
          data-code={key.code}
          data-expected={expectedCode === key.code}
          data-pressed={pressedCodes.includes(key.code)}
          data-held={Boolean(key.modifier && pressedCodes.includes(key.code))}
          data-correct={completedCodes.includes(key.code)}
          data-incorrect={incorrectCode === key.code}
          data-completed={Boolean(
            sequenceCompleted && expectedCodes.includes(key.code),
          )}
        >
          <span class="shifted-legend">{key.shifted ?? ""}</span>
          {#if keyLegends[key.code]?.shifted}
            <strong class="shifted-target-legend"
              >{keyLegends[key.code].shifted}</strong
            >
          {/if}
          {#if keyLegends[key.code]?.base}
            <strong class="target-legend">{keyLegends[key.code].base}</strong>
          {/if}
          <span class="latin-legend">{key.latin}</span>
          {#if keyMarker(key)}
            <span class="key-marker">{keyMarker(key)}</span>
          {/if}
        </div>
      {/each}
    </div>
  {/each}
</div>

<style>
  .typing-keyboard {
    display: grid;
    width: 100%;
    min-width: 0;
    gap: clamp(0.125rem, 0.5vw, 0.35rem);
    overflow: hidden;
  }

  .keyboard-row {
    display: grid;
    grid-template-columns: repeat(30, minmax(0, 1fr));
    gap: clamp(0.0625rem, 0.3vw, 0.25rem);
  }

  .keycap {
    position: relative;
    display: grid;
    grid-column: span var(--key-units);
    min-width: 0;
    height: clamp(2.15rem, 5vw, 3.15rem);
    padding: clamp(0.125rem, 0.45vw, 0.35rem);
    overflow: hidden;
    border: 1px solid var(--border);
    border-bottom-width: 3px;
    border-radius: 0;
    color: var(--foreground);
    background: var(--card);
  }

  .keycap[data-pressed="true"] {
    border-bottom-width: 1px;
    background: var(--muted);
    transform: translateY(2px);
  }

  .keycap[data-correct="true"],
  .keycap[data-completed="true"] {
    border-style: solid;
    border-color: var(--primary);
    background: var(--primary);
    color: var(--primary-foreground);
  }

  .keycap[data-expected="true"] {
    border: 2px dashed var(--primary);
    color: var(--foreground);
    background: var(--secondary);
  }

  .keycap[data-incorrect="true"] {
    border: 2px double var(--destructive);
    color: var(--destructive);
    background: color-mix(in oklch, var(--destructive) 10%, var(--card));
  }

  .keycap[data-held="true"] {
    box-shadow: inset 0 0 0 2px currentColor;
  }

  .keycap-modifier .latin-legend {
    font-size: clamp(0.42rem, 1.35vw, 0.67rem);
  }

  .latin-legend {
    align-self: end;
    overflow: hidden;
    font-family: var(--font-content);
    font-size: clamp(0.48rem, 1.45vw, 0.75rem);
    line-height: 1;
    text-overflow: clip;
    white-space: nowrap;
  }

  .shifted-legend {
    min-height: 0.65em;
    font-family: var(--font-content);
    font-size: clamp(0.38rem, 1vw, 0.6rem);
    line-height: 1;
  }

  .shifted-target-legend,
  .target-legend {
    position: absolute;
    left: 50%;
    font-family: var(--font-content);
    line-height: 1;
    transform: translateX(-50%);
  }

  .shifted-target-legend {
    top: 0.15rem;
    font-size: clamp(0.45rem, 1.3vw, 0.7rem);
  }

  .target-legend {
    top: 50%;
    font-size: clamp(0.6rem, 1.8vw, 1rem);
    transform: translate(-50%, -50%);
  }

  .key-marker {
    position: absolute;
    top: 0.15rem;
    right: 0.2rem;
    font-family: var(--font-content);
    font-size: clamp(0.45rem, 1.3vw, 0.75rem);
    font-weight: 900;
    line-height: 1;
  }

  @media (max-width: 480px) {
    .keycap {
      height: 1.9rem;
      padding: 0.1rem;
      border-bottom-width: 2px;
    }

    .keycap-modifier .latin-legend {
      font-size: 0.38rem;
    }
  }
</style>
