export type InstructionPlatform = "windows" | "macos";
export type TypingLanguage = "korean" | "japanese" | "french" | "spanish";
export type TypingDrillMode = "physical" | "committed";

export type TypingTrack = {
  language: TypingLanguage;
  selectionLabel: string;
};

export type TypingKeyLegend = {
  shifted?: string;
  base?: string;
};

export type TypingLesson = {
  id: string;
  language: TypingLanguage;
  languageTag: string;
  title: string;
  mode: TypingDrillMode;
  target: string;
  expectedText: string;
  sharedPhysicalCodes: string[];
  platformPhysicalCodes: Record<InstructionPlatform, string[]>;
  hint: string;
  keyLegends: Record<string, TypingKeyLegend>;
  instructions: Record<InstructionPlatform, string>;
};

export const typingTracks: TypingTrack[] = [
  { language: "korean", selectionLabel: "Korean — 2-set Hangul" },
  { language: "japanese", selectionLabel: "Japanese — Romaji input" },
  { language: "french", selectionLabel: "French — Dead-key accents" },
  { language: "spanish", selectionLabel: "Spanish — Dead-key accents" },
];

const koreanKeyLegends: Record<string, TypingKeyLegend> = {
  KeyQ: { shifted: "ㅃ", base: "ㅂ" },
  KeyW: { shifted: "ㅉ", base: "ㅈ" },
  KeyE: { shifted: "ㄸ", base: "ㄷ" },
  KeyR: { shifted: "ㄲ", base: "ㄱ" },
  KeyT: { shifted: "ㅆ", base: "ㅅ" },
  KeyY: { base: "ㅛ" },
  KeyU: { base: "ㅕ" },
  KeyI: { base: "ㅑ" },
  KeyO: { shifted: "ㅒ", base: "ㅐ" },
  KeyP: { shifted: "ㅖ", base: "ㅔ" },
  KeyA: { base: "ㅁ" },
  KeyS: { base: "ㄴ" },
  KeyD: { base: "ㅇ" },
  KeyF: { base: "ㄹ" },
  KeyG: { base: "ㅎ" },
  KeyH: { base: "ㅗ" },
  KeyJ: { base: "ㅓ" },
  KeyK: { base: "ㅏ" },
  KeyL: { base: "ㅣ" },
  KeyZ: { base: "ㅋ" },
  KeyX: { base: "ㅌ" },
  KeyC: { base: "ㅊ" },
  KeyV: { base: "ㅍ" },
  KeyB: { base: "ㅠ" },
  KeyN: { base: "ㅜ" },
  KeyM: { base: "ㅡ" },
};

const koreanInstructions: Record<InstructionPlatform, string> = {
  windows:
    "Use 2-set Korean. On standard US hardware, use Right Alt for 한/영 switching.",
  macos:
    "Enable Korean input. On standard US hardware, use Right Command for switching.",
};

export const typingLessons: TypingLesson[] = [
  {
    id: "typing-korean-basic-consonants",
    language: "korean",
    languageTag: "ko",
    title: "Basic consonants",
    mode: "physical",
    target: "ㅂ ㅈ ㄷ ㄱ ㅅ ㅁ ㄴ ㅇ ㄹ ㅎ ㅋ ㅌ ㅊ ㅍ",
    expectedText: "ㅂㅈㄷㄱㅅㅁㄴㅇㄹㅎㅋㅌㅊㅍ",
    sharedPhysicalCodes: [
      "KeyQ",
      "KeyW",
      "KeyE",
      "KeyR",
      "KeyT",
      "KeyA",
      "KeyS",
      "KeyD",
      "KeyF",
      "KeyG",
      "KeyZ",
      "KeyX",
      "KeyC",
      "KeyV",
    ],
    platformPhysicalCodes: { windows: [], macos: [] },
    hint: "Press the consonant positions from the top row through the bottom row. No active Korean input source is required.",
    keyLegends: koreanKeyLegends,
    instructions: koreanInstructions,
  },
  {
    id: "typing-korean-basic-vowels",
    language: "korean",
    languageTag: "ko",
    title: "Basic vowels",
    mode: "physical",
    target: "ㅛ ㅕ ㅑ ㅐ ㅔ ㅗ ㅓ ㅏ ㅣ ㅠ ㅜ ㅡ",
    expectedText: "ㅛㅕㅑㅐㅔㅗㅓㅏㅣㅠㅜㅡ",
    sharedPhysicalCodes: [
      "KeyY",
      "KeyU",
      "KeyI",
      "KeyO",
      "KeyP",
      "KeyH",
      "KeyJ",
      "KeyK",
      "KeyL",
      "KeyB",
      "KeyN",
      "KeyM",
    ],
    platformPhysicalCodes: { windows: [], macos: [] },
    hint: "Press each base vowel position in row order. The physical drill is independent of the active input source.",
    keyLegends: koreanKeyLegends,
    instructions: koreanInstructions,
  },
  {
    id: "typing-korean-shift-forms",
    language: "korean",
    languageTag: "ko",
    title: "Shift forms",
    mode: "physical",
    target: "ㅃ ㅉ ㄸ ㄲ ㅆ ㅒ ㅖ",
    expectedText: "ㅃㅉㄸㄲㅆㅒㅖ",
    sharedPhysicalCodes: [
      "ShiftLeft",
      "KeyQ",
      "KeyW",
      "KeyE",
      "KeyR",
      "KeyT",
      "KeyO",
      "KeyP",
    ],
    platformPhysicalCodes: { windows: [], macos: [] },
    hint: "Hold Shift, then press Q, W, E, R, T, O, and P. Release Shift after the final key.",
    keyLegends: koreanKeyLegends,
    instructions: koreanInstructions,
  },
  {
    id: "typing-korean-compound-vowels",
    language: "korean",
    languageTag: "ko",
    title: "Compound vowels",
    mode: "physical",
    target: "ㅘ ㅚ ㅝ ㅢ",
    expectedText: "ㅘㅚㅝㅢ",
    sharedPhysicalCodes: [
      "KeyH",
      "KeyK",
      "KeyH",
      "KeyL",
      "KeyN",
      "KeyJ",
      "KeyM",
      "KeyL",
    ],
    platformPhysicalCodes: { windows: [], macos: [] },
    hint: "Combine H K for ㅘ, H L for ㅚ, N J for ㅝ, and M L for ㅢ.",
    keyLegends: koreanKeyLegends,
    instructions: koreanInstructions,
  },
  {
    id: "typing-korean-syllable-blocks",
    language: "korean",
    languageTag: "ko",
    title: "Syllable-block assembly",
    mode: "committed",
    target: "한",
    expectedText: "한",
    sharedPhysicalCodes: ["KeyG", "KeyK", "KeyS"],
    platformPhysicalCodes: { windows: [], macos: [] },
    hint: "With 2-set Korean active, type G K S and commit the complete syllable block 한.",
    keyLegends: koreanKeyLegends,
    instructions: koreanInstructions,
  },
  {
    id: "typing-korean-short-words",
    language: "korean",
    languageTag: "ko",
    title: "Short words",
    mode: "committed",
    target: "안녕",
    expectedText: "안녕",
    sharedPhysicalCodes: ["KeyD", "KeyK", "KeyS", "KeyS", "KeyU", "KeyD"],
    platformPhysicalCodes: { windows: [], macos: [] },
    hint: "With 2-set Korean active, type D K S S U D and commit the word 안녕.",
    keyLegends: koreanKeyLegends,
    instructions: koreanInstructions,
  },
  {
    id: "typing-korean-short-phrases",
    language: "korean",
    languageTag: "ko",
    title: "Short phrases",
    mode: "committed",
    target: "안녕 친구",
    expectedText: "안녕 친구",
    sharedPhysicalCodes: [
      "KeyD",
      "KeyK",
      "KeyS",
      "KeyS",
      "KeyU",
      "KeyD",
      "Space",
      "KeyC",
      "KeyL",
      "KeyS",
      "KeyR",
      "KeyN",
    ],
    platformPhysicalCodes: { windows: [], macos: [] },
    hint: "With 2-set Korean active, type D K S S U D, Space, C L S R N, then commit 안녕 친구.",
    keyLegends: koreanKeyLegends,
    instructions: koreanInstructions,
  },
  {
    id: "typing-japanese-foundation",
    language: "japanese",
    languageTag: "ja",
    title: "Commit the first Japanese vowel",
    mode: "committed",
    target: "あ",
    expectedText: "あ",
    sharedPhysicalCodes: ["KeyA"],
    platformPhysicalCodes: { windows: [], macos: [] },
    hint: "With Japanese romaji input active, type A and commit あ.",
    keyLegends: {},
    instructions: {
      windows:
        "Add Microsoft Japanese IME and use its romaji input setting in Windows language settings.",
      macos:
        "Add Japanese — Romaji in Keyboard settings, then choose that input source.",
    },
  },
  {
    id: "typing-french-foundation",
    language: "french",
    languageTag: "fr",
    title: "Commit one accented grapheme",
    mode: "committed",
    target: "é",
    expectedText: "é",
    sharedPhysicalCodes: [],
    platformPhysicalCodes: {
      windows: ["Quote", "KeyE"],
      macos: ["AltLeft", "KeyE", "KeyE"],
    },
    hint: "Create the acute accent first, then type E.",
    keyLegends: {},
    instructions: {
      windows:
        "Choose a Windows input layout that provides an acute-accent dead key, then press the dead key followed by E.",
      macos:
        "Hold Option while pressing E, release Option, then press E again.",
    },
  },
  {
    id: "typing-spanish-foundation",
    language: "spanish",
    languageTag: "es",
    title: "Commit one Spanish accented grapheme",
    mode: "committed",
    target: "á",
    expectedText: "á",
    sharedPhysicalCodes: [],
    platformPhysicalCodes: {
      windows: ["Quote", "KeyA"],
      macos: ["AltLeft", "KeyE", "KeyA"],
    },
    hint: "Create the acute accent first, then type A.",
    keyLegends: {},
    instructions: {
      windows:
        "Choose a Windows input layout that provides an acute-accent dead key, then press the dead key followed by A.",
      macos: "Hold Option while pressing E, release Option, then press A.",
    },
  },
];

export function detectInstructionPlatform(
  runtimeNavigator: Navigator,
): InstructionPlatform | null {
  const navigatorWithClientHints = runtimeNavigator as Navigator & {
    userAgentData?: { platform?: string };
  };
  const platform =
    navigatorWithClientHints.userAgentData?.platform ||
    runtimeNavigator.platform;
  if (/^mac/i.test(platform)) return "macos";
  if (/^win/i.test(platform)) return "windows";
  return null;
}
