import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { typingLessons, type TypingLanguage } from "../src/lib/typing-lessons";
import { installMockApi } from "./support/mock-api";

const frenchLessonIds = [
  "typing-french-acute-e",
  "typing-french-grave-e",
  "typing-french-circumflex-e",
  "typing-french-diaeresis-e",
  "typing-french-grave-a",
  "typing-french-grave-u",
  "typing-french-circumflex-i",
  "typing-french-circumflex-o",
  "typing-french-cedilla-c",
  "typing-french-ligature-oe",
  "typing-french-left-guillemet",
  "typing-french-right-guillemet",
  "typing-french-short-words",
  "typing-french-short-sentence",
] as const;

const spanishLessonIds = [
  "typing-spanish-tilde-n",
  "typing-spanish-acute-a",
  "typing-spanish-acute-e",
  "typing-spanish-acute-i",
  "typing-spanish-acute-o",
  "typing-spanish-acute-u",
  "typing-spanish-diaeresis-u",
  "typing-spanish-inverted-question",
  "typing-spanish-inverted-exclamation",
  "typing-spanish-short-words",
  "typing-spanish-short-sentence",
] as const;

const lessonIds = {
  french: frenchLessonIds,
  spanish: spanishLessonIds,
} as const;

const trackLabels = {
  french: "French — Dead-key accents",
  spanish: "Spanish — Dead-key accents",
} as const;

const characterMappings = [
  {
    id: "typing-french-acute-e",
    target: "é",
    windows: ["Quote", "KeyE"],
    macos: ["AltLeft", "KeyE", "KeyE"],
  },
  {
    id: "typing-french-grave-e",
    target: "è",
    windows: ["Backquote", "KeyE"],
    macos: ["AltLeft", "Backquote", "KeyE"],
  },
  {
    id: "typing-french-circumflex-e",
    target: "ê",
    windows: ["ShiftLeft", "Digit6", "KeyE"],
    macos: ["AltLeft", "KeyI", "KeyE"],
  },
  {
    id: "typing-french-diaeresis-e",
    target: "ë",
    windows: ["ShiftLeft", "Quote", "KeyE"],
    macos: ["AltLeft", "KeyU", "KeyE"],
  },
  {
    id: "typing-french-grave-a",
    target: "à",
    windows: ["Backquote", "KeyA"],
    macos: ["AltLeft", "Backquote", "KeyA"],
  },
  {
    id: "typing-french-grave-u",
    target: "ù",
    windows: ["Backquote", "KeyU"],
    macos: ["AltLeft", "Backquote", "KeyU"],
  },
  {
    id: "typing-french-circumflex-i",
    target: "î",
    windows: ["ShiftLeft", "Digit6", "KeyI"],
    macos: ["AltLeft", "KeyI", "KeyI"],
  },
  {
    id: "typing-french-circumflex-o",
    target: "ô",
    windows: ["ShiftLeft", "Digit6", "KeyO"],
    macos: ["AltLeft", "KeyI", "KeyO"],
  },
  {
    id: "typing-french-cedilla-c",
    target: "ç",
    windows: ["AltRight", "Comma"],
    macos: ["AltLeft", "KeyC"],
  },
  {
    id: "typing-french-ligature-oe",
    target: "œ",
    windows: [],
    macos: ["AltLeft", "KeyQ"],
  },
  {
    id: "typing-french-left-guillemet",
    target: "«",
    windows: [],
    macos: ["AltLeft", "Backslash"],
  },
  {
    id: "typing-french-right-guillemet",
    target: "»",
    windows: [],
    macos: ["AltLeft", "ShiftLeft", "Backslash"],
  },
  {
    id: "typing-spanish-tilde-n",
    target: "ñ",
    windows: ["ShiftLeft", "Backquote", "KeyN"],
    macos: ["AltLeft", "KeyN", "KeyN"],
  },
  {
    id: "typing-spanish-acute-a",
    target: "á",
    windows: ["Quote", "KeyA"],
    macos: ["AltLeft", "KeyE", "KeyA"],
  },
  {
    id: "typing-spanish-acute-e",
    target: "é",
    windows: ["Quote", "KeyE"],
    macos: ["AltLeft", "KeyE", "KeyE"],
  },
  {
    id: "typing-spanish-acute-i",
    target: "í",
    windows: ["Quote", "KeyI"],
    macos: ["AltLeft", "KeyE", "KeyI"],
  },
  {
    id: "typing-spanish-acute-o",
    target: "ó",
    windows: ["Quote", "KeyO"],
    macos: ["AltLeft", "KeyE", "KeyO"],
  },
  {
    id: "typing-spanish-acute-u",
    target: "ú",
    windows: ["Quote", "KeyU"],
    macos: ["AltLeft", "KeyE", "KeyU"],
  },
  {
    id: "typing-spanish-diaeresis-u",
    target: "ü",
    windows: ["ShiftLeft", "Quote", "KeyU"],
    macos: ["AltLeft", "KeyU", "KeyU"],
  },
  {
    id: "typing-spanish-inverted-question",
    target: "¿",
    windows: ["AltRight", "Slash"],
    macos: ["AltLeft", "ShiftLeft", "Slash"],
  },
  {
    id: "typing-spanish-inverted-exclamation",
    target: "¡",
    windows: ["AltRight", "Digit1"],
    macos: ["AltLeft", "Digit1"],
  },
] as const;

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

function lesson(id: string) {
  const match = typingLessons.find((candidate) => candidate.id === id);
  if (!match) throw new Error(`Missing typing lesson ${id}.`);
  return match;
}

async function setRuntimePlatform(page: Page, platform: string): Promise<void> {
  await page.addInitScript((runtimePlatform) => {
    Object.defineProperty(navigator, "platform", {
      configurable: true,
      get: () => runtimePlatform,
    });
    Object.defineProperty(navigator, "userAgentData", {
      configurable: true,
      get: () => ({ platform: runtimePlatform }),
    });
  }, platform);
}

async function openTyping(page: Page): Promise<void> {
  const openNavigation = page.getByRole("button", { name: "Open navigation" });
  if (await openNavigation.isVisible()) await openNavigation.click();
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Typing", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Typing", level: 1 }),
  ).toBeVisible();
}

async function openLesson(
  page: Page,
  language: "french" | "spanish",
  lessonIndex: number,
  runtimePlatform: "Win32" | "MacIntel",
): Promise<Locator> {
  await setRuntimePlatform(page, runtimePlatform);
  await page.goto("/");
  await page.evaluate(
    ({ completedLessonIds, platform }) => {
      localStorage.setItem("meiki-typing-platform", platform);
      if (completedLessonIds.length > 0) {
        localStorage.setItem(
          "meiki-typing-completed",
          JSON.stringify(completedLessonIds),
        );
      } else {
        localStorage.removeItem("meiki-typing-completed");
      }
    },
    {
      completedLessonIds: lessonIds[language].slice(0, lessonIndex),
      platform: runtimePlatform === "Win32" ? "windows" : "macos",
    },
  );
  await page.reload();
  await openTyping(page);
  await page.getByRole("button", { name: trackLabels[language] }).click();
  await page.getByRole("button", { name: "Start practice" }).click();
  for (let index = 0; index < lessonIndex; index += 1) {
    await page.getByRole("button", { name: "Next" }).click();
  }
  const currentLesson = lesson(lessonIds[language][lessonIndex]);
  await expect(
    page.getByRole("heading", { name: currentLesson.title, level: 2 }),
  ).toBeVisible();
  return page.getByLabel("Practice input");
}

function keyForCode(code: string): string {
  if (code.startsWith("Key")) return code.slice(3).toLowerCase();
  if (code.startsWith("Digit")) return code.slice(5);
  const keys: Record<string, string> = {
    AltLeft: "Alt",
    AltRight: "AltGraph",
    Backquote: "`",
    Backslash: "\\",
    Comma: ",",
    Quote: "'",
    ShiftLeft: "Shift",
    Slash: "/",
    Space: " ",
  };
  return keys[code] ?? code;
}

async function dispatchKey(
  input: Locator,
  type: "keydown" | "keyup",
  code: string,
  options: { isComposing?: boolean; repeat?: boolean } = {},
): Promise<void> {
  await input.evaluate(
    (element, event) =>
      element.dispatchEvent(
        new KeyboardEvent(event.type, {
          bubbles: true,
          code: event.code,
          key: event.key,
          isComposing: event.isComposing,
          repeat: event.repeat,
        }),
      ),
    {
      type,
      code,
      key: keyForCode(code),
      isComposing: options.isComposing,
      repeat: options.repeat,
    },
  );
}

async function pressSequence(
  input: Locator,
  codes: readonly string[],
  isComposing = true,
): Promise<void> {
  for (const code of codes) {
    await dispatchKey(input, "keydown", code, { isComposing });
    await dispatchKey(input, "keyup", code, { isComposing });
  }
}

async function dispatchComposition(
  input: Locator,
  type: "compositionstart" | "compositionupdate" | "compositionend",
  data: string,
): Promise<void> {
  await input.evaluate(
    (element, event) =>
      element.dispatchEvent(
        new CompositionEvent(event.type, { bubbles: true, data: event.data }),
      ),
    { type, data },
  );
}

async function commitLesson(
  input: Locator,
  physicalCodes: readonly string[],
  committedText: string,
): Promise<void> {
  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", committedText);
  await pressSequence(input, physicalCodes);
  await input.evaluate((element, value) => {
    (element as HTMLInputElement).value = value;
  }, committedText);
  await dispatchComposition(input, "compositionend", committedText);
  await input.press("Enter");
}

test("French and Spanish definitions preserve every required target and exact platform mapping", () => {
  expect(frenchLessonIds.slice(0, 12).map((id) => lesson(id).target)).toEqual([
    "é",
    "è",
    "ê",
    "ë",
    "à",
    "ù",
    "î",
    "ô",
    "ç",
    "œ",
    "«",
    "»",
  ]);
  expect(spanishLessonIds.slice(0, 9).map((id) => lesson(id).target)).toEqual([
    "ñ",
    "á",
    "é",
    "í",
    "ó",
    "ú",
    "ü",
    "¿",
    "¡",
  ]);

  for (const mapping of characterMappings) {
    const mappedLesson = lesson(mapping.id);
    expect(mappedLesson.target).toBe(mapping.target);
    expect(mappedLesson.sharedPhysicalCodes).toEqual([]);
    expect(mappedLesson.platformPhysicalCodes.windows).toEqual(mapping.windows);
    expect(mappedLesson.platformPhysicalCodes.macos).toEqual(mapping.macos);
    expect(mappedLesson.keyLegends).toEqual({});
  }

  expect(lesson("typing-french-short-words").target).toBe("déjà hôtel");
  expect(lesson("typing-french-short-sentence").target).toBe(
    "Où est le café ?",
  );
  expect(lesson("typing-spanish-short-words").target).toBe("niño pingüino");
  expect(lesson("typing-spanish-short-sentence").target).toBe(
    "¿Dónde está? ¡Qué niño más feliz!",
  );
});

test("Option, AltGr, and Shift chords show the held modifier beside the expected key until keyup", async ({
  page,
}) => {
  let input = await openLesson(page, "french", 0, "MacIntel");
  await dispatchKey(input, "keydown", "AltLeft");
  await expect(page.getByTestId("typing-key-AltLeft")).toHaveAttribute(
    "data-held",
    "true",
  );
  await expect(page.getByTestId("typing-key-KeyE")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await dispatchKey(input, "keydown", "KeyE");
  await dispatchKey(input, "keyup", "KeyE");
  await expect(page.getByTestId("typing-key-AltLeft")).toHaveAttribute(
    "data-held",
    "true",
  );
  await dispatchKey(input, "keyup", "AltLeft");
  await expect(page.getByTestId("typing-key-AltLeft")).toHaveAttribute(
    "data-held",
    "false",
  );

  input = await openLesson(page, "french", 8, "Win32");
  await dispatchKey(input, "keydown", "AltRight");
  await expect(page.getByTestId("typing-key-AltRight")).toHaveAttribute(
    "data-held",
    "true",
  );
  await expect(page.getByTestId("typing-key-Comma")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await dispatchKey(input, "keyup", "AltRight");

  input = await openLesson(page, "french", 2, "Win32");
  await dispatchKey(input, "keydown", "ShiftLeft");
  await expect(page.getByTestId("typing-key-ShiftLeft")).toHaveAttribute(
    "data-held",
    "true",
  );
  await expect(page.getByTestId("typing-key-Digit6")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await dispatchKey(input, "keyup", "ShiftLeft");
});

test("dead-key composition stays unchecked, preserves repeated codes, compares graphemes, and Retry clears all state", async ({
  page,
}) => {
  const input = await openLesson(page, "french", 0, "MacIntel");
  const status = page.locator("#typing-live-status");
  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "´");
  await dispatchKey(input, "keydown", "AltLeft", { isComposing: true });
  await dispatchKey(input, "keyup", "AltLeft", { isComposing: true });
  await dispatchKey(input, "keydown", "KeyE", { isComposing: true });
  await dispatchKey(input, "keydown", "KeyE", {
    isComposing: true,
    repeat: true,
  });

  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "Option → E",
  );
  await expect(page.getByTestId("typing-composition")).toHaveText("´");
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await expect(page.getByTestId("typing-key-KeyE")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await expect(status).not.toHaveClass(/incorrect-feedback/);
  await dispatchKey(input, "keyup", "KeyE", { isComposing: true });
  await dispatchKey(input, "keydown", "KeyE", { isComposing: true });
  await dispatchKey(input, "keyup", "KeyE", { isComposing: true });
  await dispatchKey(input, "keydown", "Enter", { isComposing: true });
  await expect(status).toContainText("only commit the active composition");
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
  await dispatchKey(input, "keyup", "Enter", { isComposing: true });

  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "e\u0301";
  });
  await dispatchComposition(input, "compositionend", "e\u0301");
  await input.press("Enter");
  await expect(status).toHaveText("Correct — é");
  await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();

  await page.getByRole("button", { name: "Retry" }).click();
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "None yet",
  );
  await expect(page.getByTestId("typing-composition")).toHaveText("None");
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await expect(page.getByTestId("typing-key-AltLeft")).toHaveAttribute(
    "data-held",
    "false",
  );
  await expect(page.getByTestId("typing-key-KeyE")).toHaveAttribute(
    "data-correct",
    "false",
  );
  await expect(status).toHaveText("Expected Option.");
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
});

test("Spanish ñ preserves ordinal N progress and does not consume repeat events", async ({
  page,
}) => {
  const input = await openLesson(page, "spanish", 0, "MacIntel");
  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "~");
  await pressSequence(input, ["AltLeft"]);
  await dispatchKey(input, "keydown", "KeyN", { isComposing: true });
  await dispatchKey(input, "keydown", "KeyN", {
    isComposing: true,
    repeat: true,
  });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "Option → N",
  );
  await expect(page.getByTestId("typing-key-KeyN")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await expect(page.locator("#typing-live-status")).toContainText("Next: N");
  await dispatchKey(input, "keyup", "KeyN", { isComposing: true });
  await dispatchKey(input, "keydown", "KeyN", { isComposing: true });
  await dispatchKey(input, "keyup", "KeyN", { isComposing: true });
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "n\u0303";
  });
  await dispatchComposition(input, "compositionend", "n\u0303");
  await input.press("Enter");
  await expect(page.locator("#typing-live-status")).toHaveText("Correct — ñ");
});

for (const language of ["french", "spanish"] as const) {
  test(`${language} words and sentences validate committed Unicode and retain separate completion`, async ({
    page,
  }) => {
    const wordIndex = lessonIds[language].length - 2;
    let input = await openLesson(page, language, wordIndex, "MacIntel");
    const wordLesson = lesson(lessonIds[language][wordIndex]);
    await commitLesson(
      input,
      wordLesson.platformPhysicalCodes.macos,
      wordLesson.expectedText,
    );
    await expect(page.locator("#typing-live-status")).toHaveText(
      `Correct — ${wordLesson.target}`,
    );
    await page.getByRole("button", { name: "Next" }).click();

    input = page.getByLabel("Practice input");
    const sentenceLesson = lesson(lessonIds[language][wordIndex + 1]);
    await expect(page.getByTestId("typing-expected-sequence")).toContainText(
      "→",
    );
    await commitLesson(
      input,
      sentenceLesson.platformPhysicalCodes.macos,
      sentenceLesson.expectedText,
    );
    await expect(page.locator("#typing-live-status")).toHaveText(
      `Correct — ${sentenceLesson.target}`,
    );

    const completed = JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    );
    expect(completed).toEqual(lessonIds[language]);
    const otherLanguage: TypingLanguage =
      language === "french" ? "spanish" : "french";
    expect(
      completed.some((id: string) => id.startsWith(`typing-${otherLanguage}`)),
    ).toBe(false);
  });
}

test("Windows and macOS setup copy uses only the required U.S. terminology", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Win32");
  await page.goto("/");
  await openTyping(page);
  await page.getByRole("button", { name: trackLabels.french }).click();
  await expect(
    page.getByText("Use United States-International on Windows.", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(page.getByText(/AZERTY/i)).toHaveCount(0);

  await page.getByRole("button", { name: trackLabels.spanish }).click();
  await expect(
    page.getByText("Use United States-International on Windows.", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(page.getByText(/separate Spanish layout/i)).toHaveCount(0);

  await page.getByRole("button", { name: "macOS", exact: true }).click();
  await expect(
    page.getByText("Use the standard U.S. layout on macOS.", { exact: true }),
  ).toBeVisible();
});

test("French lessons are keyboard operable and perform no runtime backend work", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Win32");
  await page.goto("/");
  await openTyping(page);
  const track = page.getByRole("button", { name: trackLabels.french });
  await track.focus();
  await page.keyboard.press("Enter");
  const start = page.getByRole("button", { name: "Start practice" });
  await start.focus();
  await page.keyboard.press("Enter");
  const input = page.getByLabel("Practice input");
  await input.focus();
  await page.evaluate(() => {
    window.__MEIKI_TEST_REQUESTS__ = [];
  });
  await page.keyboard.press("'");
  await page.keyboard.press("e");
  await input.fill("é");
  await page.keyboard.press("Enter");
  await expect(page.locator("#typing-live-status")).toHaveText("Correct — é");
  const retry = page.getByRole("button", { name: "Retry" });
  await retry.focus();
  await page.keyboard.press("Enter");
  await input.focus();
  await page.keyboard.press("'");
  await page.keyboard.press("e");
  await input.fill("é");
  await page.keyboard.press("Enter");
  const next = page.getByRole("button", { name: "Next" });
  await next.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Grave è", level: 2 }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => window.__MEIKI_TEST_REQUESTS__ ?? []),
  ).toEqual([]);
});

test("long French and Spanish lessons wrap in narrow layouts and remain accessible", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 760 });
  for (const language of ["french", "spanish"] as const) {
    await openLesson(
      page,
      language,
      lessonIds[language].length - 1,
      "MacIntel",
    );
    for (const selector of [
      ".practice",
      ".practice-header",
      ".target",
      ".practice-details",
      "[data-testid='typing-expected-sequence']",
      "[data-testid='typing-keyboard']",
    ]) {
      const bounds = await page.locator(selector).boundingBox();
      expect(bounds).not.toBeNull();
      if (!bounds) throw new Error(`${selector} must have measurable bounds.`);
      expect(bounds.x).toBeGreaterThanOrEqual(0);
      expect(bounds.x + bounds.width).toBeLessThanOrEqual(320);
    }
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
  }

  const accessibility = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(accessibility.violations).toEqual([]);
});
