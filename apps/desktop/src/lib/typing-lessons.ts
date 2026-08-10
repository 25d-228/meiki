export type InstructionPlatform = "windows" | "macos";
export type TypingLanguage = "korean" | "japanese" | "french" | "spanish";
export type TypingDrillMode = "physical" | "committed";

export type TypingLesson = {
  id: string;
  language: TypingLanguage;
  languageTag: string;
  selectionLabel: string;
  title: string;
  mode: TypingDrillMode;
  target: string;
  expectedText: string;
  sharedPhysicalCodes: string[];
  platformPhysicalCodes: Record<InstructionPlatform, string[]>;
  hint: string;
  keyLegends: Record<string, string>;
  instructions: Record<InstructionPlatform, string>;
};

export const typingLessons: TypingLesson[] = [
  {
    id: "typing-korean-foundation",
    language: "korean",
    languageTag: "ko",
    selectionLabel: "Korean — 2-set Hangul",
    title: "Build 아 from two physical positions",
    mode: "physical",
    target: "아",
    expectedText: "아",
    sharedPhysicalCodes: ["KeyD", "KeyK"],
    platformPhysicalCodes: { windows: [], macos: [] },
    hint: "Press the D position for ㅇ, then the K position for ㅏ.",
    keyLegends: { KeyD: "ㅇ", KeyK: "ㅏ" },
    instructions: {
      windows:
        "Add the Korean Microsoft IME and choose its 2-set Hangul layout in Windows language settings.",
      macos:
        "Add Korean — 2-Set Korean in Keyboard settings, then choose that input source.",
    },
  },
  {
    id: "typing-japanese-foundation",
    language: "japanese",
    languageTag: "ja",
    selectionLabel: "Japanese — Romaji input",
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
    selectionLabel: "French — Dead-key accents",
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
    selectionLabel: "Spanish — Dead-key accents",
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
