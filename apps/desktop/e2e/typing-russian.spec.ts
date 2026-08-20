import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { typingLessons, typingTracks } from "../src/lib/typing-lessons";
import { installMockApi } from "./support/mock-api";

const russianLessonIds = [
  "typing-russian-top-row",
  "typing-russian-home-row",
  "typing-russian-bottom-row",
  "typing-russian-uppercase-shift",
  "typing-russian-short-word",
  "typing-russian-repeated-letters",
  "typing-russian-short-phrase",
] as const;

const russianLessonTitles = [
  "Ё and top row",
  "Home row",
  "Bottom row",
  "Uppercase with Shift",
  "Short Russian word",
  "Repeated Russian letters",
  "Short Russian phrase",
] as const;

const topRowCodes = [
  "Backquote",
  "KeyQ",
  "KeyW",
  "KeyE",
  "KeyR",
  "KeyT",
  "KeyY",
  "KeyU",
  "KeyI",
  "KeyO",
  "KeyP",
  "BracketLeft",
  "BracketRight",
] as const;

const homeRowCodes = [
  "KeyA",
  "KeyS",
  "KeyD",
  "KeyF",
  "KeyG",
  "KeyH",
  "KeyJ",
  "KeyK",
  "KeyL",
  "Semicolon",
  "Quote",
] as const;

const bottomRowCodes = [
  "KeyZ",
  "KeyX",
  "KeyC",
  "KeyV",
  "KeyB",
  "KeyN",
  "KeyM",
  "Comma",
  "Period",
] as const;

const russianMappings = {
  Backquote: { shifted: "Ё", base: "ё", latin: "`" },
  KeyQ: { shifted: "Й", base: "й", latin: "Q" },
  KeyW: { shifted: "Ц", base: "ц", latin: "W" },
  KeyE: { shifted: "У", base: "у", latin: "E" },
  KeyR: { shifted: "К", base: "к", latin: "R" },
  KeyT: { shifted: "Е", base: "е", latin: "T" },
  KeyY: { shifted: "Н", base: "н", latin: "Y" },
  KeyU: { shifted: "Г", base: "г", latin: "U" },
  KeyI: { shifted: "Ш", base: "ш", latin: "I" },
  KeyO: { shifted: "Щ", base: "щ", latin: "O" },
  KeyP: { shifted: "З", base: "з", latin: "P" },
  BracketLeft: { shifted: "Х", base: "х", latin: "[" },
  BracketRight: { shifted: "Ъ", base: "ъ", latin: "]" },
  KeyA: { shifted: "Ф", base: "ф", latin: "A" },
  KeyS: { shifted: "Ы", base: "ы", latin: "S" },
  KeyD: { shifted: "В", base: "в", latin: "D" },
  KeyF: { shifted: "А", base: "а", latin: "F" },
  KeyG: { shifted: "П", base: "п", latin: "G" },
  KeyH: { shifted: "Р", base: "р", latin: "H" },
  KeyJ: { shifted: "О", base: "о", latin: "J" },
  KeyK: { shifted: "Л", base: "л", latin: "K" },
  KeyL: { shifted: "Д", base: "д", latin: "L" },
  Semicolon: { shifted: "Ж", base: "ж", latin: ";" },
  Quote: { shifted: "Э", base: "э", latin: "'" },
  KeyZ: { shifted: "Я", base: "я", latin: "Z" },
  KeyX: { shifted: "Ч", base: "ч", latin: "X" },
  KeyC: { shifted: "С", base: "с", latin: "C" },
  KeyV: { shifted: "М", base: "м", latin: "V" },
  KeyB: { shifted: "И", base: "и", latin: "B" },
  KeyN: { shifted: "Т", base: "т", latin: "N" },
  KeyM: { shifted: "Ь", base: "ь", latin: "M" },
  Comma: { shifted: "Б", base: "б", latin: "," },
  Period: { shifted: "Ю", base: "ю", latin: "." },
  Slash: { shifted: ",", base: ".", latin: "/" },
} as const;

const expectedPhysicalCodes = [
  topRowCodes,
  homeRowCodes,
  bottomRowCodes,
  ["ShiftLeft", "KeyZ"],
  ["KeyG", "KeyH", "KeyB", "KeyD", "KeyT", "KeyN"],
  ["KeyH", "KeyE", "KeyC", "KeyC", "KeyR", "KeyB", "KeyQ"],
  [
    "ShiftLeft",
    "KeyG",
    "KeyH",
    "KeyB",
    "KeyD",
    "KeyT",
    "KeyN",
    "Space",
    "KeyV",
    "KeyB",
    "KeyH",
  ],
] as const;

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

function russianLesson(id: (typeof russianLessonIds)[number]) {
  const lesson = typingLessons.find((candidate) => candidate.id === id);
  if (!lesson) throw new Error(`Missing Russian lesson ${id}`);
  return lesson;
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

async function openRussianLesson(
  page: Page,
  lessonIndex: number,
  platform = "MacIntel",
): Promise<Locator> {
  await setRuntimePlatform(page, platform);
  await page.goto("/");
  if (lessonIndex > 0) {
    await page.evaluate(
      (lessonIds) =>
        localStorage.setItem(
          "meiki-typing-completed",
          JSON.stringify(lessonIds),
        ),
      russianLessonIds.slice(0, lessonIndex),
    );
    await page.reload();
  }
  await openTyping(page);
  await page.getByRole("button", { name: "Russian — ЙЦУКЕН" }).click();
  await page.getByRole("button", { name: "Start practice" }).click();
  for (let index = 0; index < lessonIndex; index += 1) {
    await page.getByRole("button", { name: "Next" }).click();
  }
  await expect(
    page.getByRole("heading", {
      name: russianLessonTitles[lessonIndex],
      level: 2,
    }),
  ).toBeVisible();
  const input = page.getByLabel("Practice input");
  await expect(input).toBeVisible();
  return input;
}

function keyForCode(code: string): string {
  if (code.startsWith("Key")) return code.slice(3).toLowerCase();
  const keys: Record<string, string> = {
    Backquote: "`",
    BracketLeft: "[",
    BracketRight: "]",
    Comma: ",",
    Period: ".",
    Quote: "'",
    Semicolon: ";",
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
  isComposing = false,
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

async function finishComposition(
  input: Locator,
  committedText: string,
): Promise<void> {
  await input.evaluate((element, value) => {
    (element as HTMLInputElement).value = value;
  }, committedText);
  await dispatchComposition(input, "compositionend", committedText);
  await input.press("Enter");
}

test("Russian definitions append the exact ordered lessons, targets, tags, and physical sequences", () => {
  expect(
    typingTracks.map(({ language, selectionLabel }) => [
      language,
      selectionLabel,
    ]),
  ).toEqual([
    ["korean", "Korean — 2-set Hangul"],
    ["japanese", "Japanese — Romaji input"],
    ["french", "French — Dead-key accents"],
    ["spanish", "Spanish — Dead-key accents"],
    ["german", "German — Umlauts and ß"],
    ["portuguese", "Portuguese — Dead-key accents"],
    ["russian", "Russian — ЙЦУКЕН"],
    ["chinese", "Chinese — Pinyin input"],
  ]);

  const lessons = russianLessonIds.map(russianLesson);
  expect(lessons.map(({ title }) => title)).toEqual(russianLessonTitles);
  expect(lessons.map(({ target }) => target)).toEqual([
    "ё й ц у к е н г ш щ з х ъ",
    "ф ы в а п р о л д ж э",
    "я ч с м и т ь б ю",
    "Я",
    "привет",
    "русский",
    "Привет мир",
  ]);
  expect(lessons.map(({ expectedText }) => expectedText)).toEqual([
    "ёйцукенгшщзхъ",
    "фывапролджэ",
    "ячсмитьбю",
    "Я",
    "привет",
    "русский",
    "Привет мир",
  ]);
  expect(lessons.map(({ languageTag }) => languageTag)).toEqual(
    russianLessonIds.map(() => "ru"),
  );
  expect(lessons.map(({ mode }) => mode)).toEqual([
    "physical",
    "physical",
    "physical",
    "committed",
    "committed",
    "committed",
    "committed",
  ]);
  expect(lessons.map(({ sharedPhysicalCodes }) => sharedPhysicalCodes)).toEqual(
    expectedPhysicalCodes,
  );
  for (const lesson of lessons) {
    expect(lesson.platformPhysicalCodes).toEqual({ windows: [], macos: [] });
  }
});

test("every Russian key shows exact shifted, base, and Latin legends without narrow overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 760 });
  await openRussianLesson(page, 0);
  const keyboard = page.getByTestId("typing-keyboard");

  await expect(keyboard.locator(".target-legend")).toHaveCount(34);
  await expect(keyboard.locator(".shifted-target-legend")).toHaveCount(34);
  for (const [code, legends] of Object.entries(russianMappings)) {
    const keycap = page.getByTestId(`typing-key-${code}`);
    await expect(keycap.locator(".shifted-target-legend")).toHaveText(
      legends.shifted,
    );
    await expect(keycap.locator(".target-legend")).toHaveText(legends.base);
    await expect(keycap.locator(".latin-legend")).toHaveText(legends.latin);
  }
  await expect(
    page
      .getByTestId("typing-key-Slash")
      .locator(".shifted-target-legend, .target-legend, .latin-legend"),
  ).toHaveText([",", ".", "/"]);
  await expect(keyboard.locator("button, [tabindex]")).toHaveCount(0);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});

test("the three Russian row drills advance through exact physical positions without committed text", async ({
  page,
}) => {
  let input = await openRussianLesson(page, 0);
  await pressSequence(input, topRowCodes);
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await page.getByRole("button", { name: "Next" }).click();

  input = page.getByLabel("Practice input");
  await pressSequence(input, homeRowCodes);
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "A → S → D → F → G → H → J → K → L → ; → '",
  );
  await page.getByRole("button", { name: "Next" }).click();

  input = page.getByLabel("Practice input");
  await pressSequence(input, bottomRowCodes);
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "Z → X → C → V → B → N → M → , → .",
  );
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
});

test("the Russian Shift lesson keeps Shift held and requires exact committed uppercase Я", async ({
  page,
}) => {
  const input = await openRussianLesson(page, 3);
  const shift = page.getByTestId("typing-key-ShiftLeft");
  const next = page.getByRole("button", { name: "Next" });

  await dispatchKey(input, "keydown", "ShiftLeft");
  await expect(shift).toHaveAttribute("data-held", "true");
  await dispatchKey(input, "keydown", "KeyZ");
  await dispatchKey(input, "keyup", "KeyZ");
  await expect(shift).toHaveAttribute("data-held", "true");
  await input.fill("я");
  await input.press("Enter");
  await expect(page.locator("#typing-live-status")).toHaveText(
    "Not yet — expected Я. Try again.",
  );
  await expect(next).toBeDisabled();
  await dispatchKey(input, "keyup", "ShiftLeft");

  await page.getByRole("button", { name: "Retry" }).click();
  await dispatchKey(input, "keydown", "ShiftLeft");
  await dispatchKey(input, "keydown", "KeyZ");
  await dispatchKey(input, "keyup", "KeyZ");
  await input.fill("Я");
  await input.press("Enter");
  await expect(page.locator("#typing-live-status")).toHaveText("Correct — Я");
  await expect(next).toBeEnabled();
  await dispatchKey(input, "keyup", "ShiftLeft");
  await expect(shift).toHaveAttribute("data-held", "false");
});

test("Russian repeated letters stay ordinal during composition and composing Enter does not submit", async ({
  page,
}) => {
  const input = await openRussianLesson(page, 5);
  const feedback = page.locator("#typing-live-status");
  const next = page.getByRole("button", { name: "Next" });

  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "рус");
  await pressSequence(input, ["KeyH", "KeyE", "KeyC"], true);
  await expect(page.getByTestId("typing-key-KeyC")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await dispatchKey(input, "keydown", "KeyC", {
    isComposing: true,
    repeat: true,
  });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "H → E → C",
  );
  await expect(next).toBeDisabled();

  await pressSequence(input, ["KeyC", "KeyR", "KeyB", "KeyQ"], true);
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "H → E → C → C → R → B → Q",
  );
  await expect(page.getByTestId("typing-composition")).toHaveText("рус");
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await expect(feedback).not.toHaveClass(/incorrect-feedback/);

  await dispatchKey(input, "keydown", "Enter", { isComposing: true });
  await expect(feedback).toContainText("only commit the active composition");
  await expect(next).toBeDisabled();
  await dispatchKey(input, "keyup", "Enter", { isComposing: true });
  await finishComposition(input, "руский");
  await expect(feedback).toHaveText("Not yet — expected русский. Try again.");
  await expect(next).toBeDisabled();

  await page.getByRole("button", { name: "Retry" }).click();
  await dispatchComposition(input, "compositionstart", "");
  await pressSequence(
    input,
    ["KeyH", "KeyE", "KeyC", "KeyC", "KeyR", "KeyB", "KeyQ"],
    true,
  );
  await finishComposition(input, "русский");
  await expect(feedback).toHaveText("Correct — русский");
  await expect(next).toBeEnabled();
});

for (const committedLesson of [
  {
    index: 4,
    text: "привет",
    codes: ["KeyG", "KeyH", "KeyB", "KeyD", "KeyT", "KeyN"],
  },
  {
    index: 6,
    text: "Привет мир",
    codes: [
      "ShiftLeft",
      "KeyG",
      "KeyH",
      "KeyB",
      "KeyD",
      "KeyT",
      "KeyN",
      "Space",
      "KeyV",
      "KeyB",
      "KeyH",
    ],
  },
] as const) {
  test(`${committedLesson.text} completes only after exact Cyrillic is committed`, async ({
    page,
  }) => {
    const input = await openRussianLesson(page, committedLesson.index);
    await dispatchComposition(input, "compositionstart", "");
    await dispatchComposition(
      input,
      "compositionupdate",
      committedLesson.text.slice(0, 2),
    );
    await pressSequence(input, committedLesson.codes, true);
    await expect(page.getByTestId("typing-committed-output")).toHaveText(
      "None",
    );
    await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();

    await finishComposition(input, committedLesson.text.toUpperCase());
    await expect(page.locator("#typing-live-status")).toHaveText(
      `Not yet — expected ${committedLesson.text}. Try again.`,
    );
    await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();

    await page.getByRole("button", { name: "Retry" }).click();
    await dispatchComposition(input, "compositionstart", "");
    await pressSequence(input, committedLesson.codes, true);
    await finishComposition(input, committedLesson.text);
    await expect(page.locator("#typing-live-status")).toHaveText(
      `Correct — ${committedLesson.text}`,
    );
    await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
  });
}

test("Russian selection and completion persist without changing another track's completion", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Linux x86_64");
  await page.goto("/?collection=empty");
  await page.evaluate(() => {
    localStorage.setItem(
      "meiki-typing-completed",
      JSON.stringify(["typing-german-diaeresis-a"]),
    );
    window.__MEIKI_TEST_REQUESTS__ = [];
  });
  const runtimeRequests: string[] = [];
  page.on("request", (request) => {
    if (["fetch", "xhr"].includes(request.resourceType())) {
      runtimeRequests.push(request.url());
    }
  });
  await openTyping(page);
  const languageChoices = page.getByRole("group", { name: "Language" });
  await expect(languageChoices.getByRole("button")).toHaveCount(8);
  await expect(
    languageChoices.getByRole("button", { name: "Russian — ЙЦУКЕН" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Russian — ЙЦУКЕН" }).click();
  await page.getByRole("button", { name: "Start practice" }).click();
  const input = page.getByLabel("Practice input");
  await pressSequence(input, topRowCodes);
  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toEqual(["typing-german-diaeresis-a", "typing-russian-top-row"]);
  expect(runtimeRequests).toEqual([]);
  expect(
    await page.evaluate(() => window.__MEIKI_TEST_REQUESTS__ ?? []),
  ).toEqual([]);

  await page.reload();
  await openTyping(page);
  await expect(
    page.getByRole("button", { name: "Russian — ЙЦУКЕН" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByText("Ё and top row", { exact: true })).toBeVisible();
  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toEqual(["typing-german-diaeresis-a", "typing-russian-top-row"]);
});

test("Russian guidance follows the selected platform and Linux stays non-prescriptive", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Linux x86_64");
  await page.goto("/");
  await openTyping(page);
  await page.getByRole("button", { name: "Russian — ЙЦУКЕН" }).click();
  const linuxGuidance = page.getByTestId("typing-linux-guidance");
  await expect(linuxGuidance).toContainText("desktop environment");
  await expect(linuxGuidance).not.toContainText(/Ctrl|Alt\+|Super|Command/);

  await page.getByRole("button", { name: "Windows", exact: true }).click();
  await expect(
    page.getByText(
      "Add Russian on Windows and use the standard Russian keyboard layout.",
      { exact: true },
    ),
  ).toBeVisible();
  await page.getByRole("button", { name: "macOS", exact: true }).click();
  await expect(
    page.getByText(
      "Add a Russian input source on macOS that uses the displayed standard ЙЦУКЕН letter positions.",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(page.getByText(/shortcut|Control|Option|Command/i)).toHaveCount(
    0,
  );
});

test("Russian Retry, keyboard progression, Vim commands, and accessibility use the shared practice surface", async ({
  page,
}) => {
  await setRuntimePlatform(page, "MacIntel");
  await page.goto("/");
  await page.evaluate(() => {
    localStorage.setItem("meiki-vim-keybindings", "true");
  });
  await page.reload();
  await openTyping(page);
  await page.getByRole("button", { name: "Russian — ЙЦУКЕН" }).click();
  const start = page.getByRole("button", { name: "Start practice" });
  await start.focus();
  await page.keyboard.press("Enter");
  const input = page.getByLabel("Practice input");
  await input.focus();
  for (const code of topRowCodes) {
    await page.keyboard.press(keyForCode(code));
  }
  await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
  await input.blur();
  await page.keyboard.press("l");
  await expect(
    page.getByRole("heading", { name: "Home row", level: 2 }),
  ).toBeVisible();
  await page.keyboard.press("r");
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "None yet",
  );
  await expect(page.getByLabel("Vim mode NORMAL")).toBeVisible();

  const accessibility = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(accessibility.violations).toEqual([]);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});
