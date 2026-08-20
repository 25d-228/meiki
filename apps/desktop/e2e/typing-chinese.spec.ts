import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { typingLessons, typingTracks } from "../src/lib/typing-lessons";
import { installMockApi } from "./support/mock-api";

const chineseLessonIds = [
  "typing-chinese-basic-initials",
  "typing-chinese-finals",
  "typing-chinese-digraph-initials",
  "typing-chinese-nihao",
  "typing-chinese-zhongwen",
  "typing-chinese-v-for-umlaut",
  "typing-chinese-apostrophe-separator",
  "typing-chinese-short-phrase",
] as const;

const chineseLessonTitles = [
  "Basic initials",
  "Finals and ü with V",
  "zh, ch, and sh",
  "Hello",
  "Chinese",
  "ü with V",
  "Apostrophe separator",
  "Short Chinese phrase",
] as const;

const physicalSequences = [
  [
    "KeyB",
    "KeyP",
    "KeyM",
    "KeyF",
    "KeyD",
    "KeyT",
    "KeyN",
    "KeyL",
    "KeyG",
    "KeyK",
    "KeyH",
  ],
  ["KeyA", "KeyO", "KeyE", "KeyI", "KeyU", "KeyV"],
  ["KeyZ", "KeyH", "KeyC", "KeyH", "KeyS", "KeyH"],
  ["KeyN", "KeyI", "KeyH", "KeyA", "KeyO"],
  ["KeyZ", "KeyH", "KeyO", "KeyN", "KeyG", "KeyW", "KeyE", "KeyN"],
  ["KeyL", "KeyV"],
  ["KeyX", "KeyI", "Quote", "KeyA", "KeyN"],
  [
    "KeyW",
    "KeyO",
    "KeyX",
    "KeyU",
    "KeyE",
    "KeyZ",
    "KeyH",
    "KeyO",
    "KeyN",
    "KeyG",
    "KeyW",
    "KeyE",
    "KeyN",
  ],
] as const;

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

function chineseLesson(id: (typeof chineseLessonIds)[number]) {
  const lesson = typingLessons.find((candidate) => candidate.id === id);
  if (!lesson) throw new Error(`Missing Chinese lesson ${id}`);
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

async function openChineseLesson(
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
      chineseLessonIds.slice(0, lessonIndex),
    );
    await page.reload();
  }
  await openTyping(page);
  await page.getByRole("button", { name: "Chinese — Pinyin input" }).click();
  await page.getByRole("button", { name: "Start practice" }).click();
  for (let index = 0; index < lessonIndex; index += 1) {
    await page.getByRole("button", { name: "Next" }).click();
  }
  await expect(
    page.getByRole("heading", {
      name: chineseLessonTitles[lessonIndex],
      level: 2,
    }),
  ).toBeVisible();
  const input = page.getByLabel("Practice input");
  await expect(input).toBeVisible();
  return input;
}

function keyForCode(code: string): string {
  if (code.startsWith("Key")) return code.slice(3).toLowerCase();
  if (code.startsWith("Digit")) return code.slice(5);
  return code === "Quote" ? "'" : code;
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

test("Chinese definitions append the exact ordered lessons, targets, tags, and Pinyin sequences", () => {
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

  const lessons = chineseLessonIds.map(chineseLesson);
  expect(lessons.map(({ title }) => title)).toEqual(chineseLessonTitles);
  expect(lessons.map(({ mode }) => mode)).toEqual([
    "physical",
    "physical",
    "physical",
    "committed",
    "committed",
    "committed",
    "committed",
    "committed",
  ]);
  expect(lessons.map(({ target }) => target)).toEqual([
    "b p m f d t n l g k h",
    "a o e i u ü",
    "zh ch sh",
    "你好",
    "中文",
    "绿",
    "西安",
    "我学中文",
  ]);
  expect(lessons.map(({ expectedText }) => expectedText)).toEqual([
    "bpmfdtnlgkh",
    "aoeiuü",
    "zhchsh",
    "你好",
    "中文",
    "绿",
    "西安",
    "我学中文",
  ]);
  expect(lessons.map(({ languageTag }) => languageTag)).toEqual(
    chineseLessonIds.map(() => "zh-Hans"),
  );
  expect(lessons.map(({ sharedPhysicalCodes }) => sharedPhysicalCodes)).toEqual(
    physicalSequences,
  );
  for (const lesson of lessons) {
    expect(lesson.platformPhysicalCodes).toEqual({ windows: [], macos: [] });
    expect(lesson.keyLegends).toEqual({});
  }
});

test("the three Chinese physical lessons complete without committed Han text and preserve repeated H ordinals", async ({
  page,
}) => {
  let input = await openChineseLesson(page, 0);
  await pressSequence(input, physicalSequences[0]);
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await page.getByRole("button", { name: "Next" }).click();

  input = page.getByLabel("Practice input");
  await pressSequence(input, physicalSequences[1]);
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "A → O → E → I → U → V",
  );
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await page.getByRole("button", { name: "Next" }).click();

  input = page.getByLabel("Practice input");
  await pressSequence(input, ["KeyZ", "KeyH", "KeyC"]);
  await dispatchKey(input, "keydown", "KeyH", { repeat: true });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "Z → H → C",
  );
  await dispatchKey(input, "keyup", "KeyH");
  await pressSequence(input, ["KeyH", "KeyS"]);
  await expect(
    page.locator(".practice").getByTestId("typing-key-KeyH"),
  ).toHaveAttribute("data-expected", "true");
  await pressSequence(input, ["KeyH"]);
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "Z → H → C → H → S → H",
  );
  await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
});

test("Chinese candidate keys remain unscored after Pinyin and composing Enter does not submit", async ({
  page,
}) => {
  const input = await openChineseLesson(page, 3);
  const feedback = page.locator("#typing-live-status");
  const next = page.getByRole("button", { name: "Next" });

  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "nihao");
  await pressSequence(input, physicalSequences[3], true);
  await pressSequence(input, ["Space", "Digit1"], true);
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "N → I → H → A → O → Space → 1",
  );
  await expect(feedback).not.toHaveClass(/incorrect-feedback/);
  await expect(next).toBeDisabled();

  await dispatchKey(input, "keydown", "Enter", { isComposing: true });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "N → I → H → A → O → Space → 1 → Enter",
  );
  await expect(feedback).toContainText("only commit the active composition");
  await expect(next).toBeDisabled();
  await dispatchKey(input, "keyup", "Enter", { isComposing: true });
  await finishComposition(input, "您好");
  await expect(feedback).toHaveText("Not yet — expected 你好. Try again.");
  await expect(next).toBeDisabled();

  await page.getByRole("button", { name: "Retry" }).click();
  await dispatchComposition(input, "compositionstart", "");
  await pressSequence(input, physicalSequences[3], true);
  await finishComposition(input, "你好");
  await expect(feedback).toHaveText("Correct — 你好");
  await expect(next).toBeEnabled();
});

for (const committedLesson of [
  { index: 4, text: "中文", wrongText: "中国" },
  { index: 5, text: "绿", wrongText: "路" },
  { index: 6, text: "西安", wrongText: "先" },
  { index: 7, text: "我学中文", wrongText: "我学汉语" },
] as const) {
  test(`${committedLesson.text} requires its exact final committed Chinese graphemes`, async ({
    page,
  }) => {
    const input = await openChineseLesson(page, committedLesson.index);
    await dispatchComposition(input, "compositionstart", "");
    await dispatchComposition(input, "compositionupdate", "拼音");
    await pressSequence(input, physicalSequences[committedLesson.index], true);
    await finishComposition(input, committedLesson.wrongText);
    await expect(page.locator("#typing-live-status")).toHaveText(
      `Not yet — expected ${committedLesson.text}. Try again.`,
    );
    await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();

    await page.getByRole("button", { name: "Retry" }).click();
    await dispatchComposition(input, "compositionstart", "");
    await pressSequence(input, physicalSequences[committedLesson.index], true);
    await finishComposition(input, committedLesson.text);
    await expect(page.locator("#typing-live-status")).toHaveText(
      `Correct — ${committedLesson.text}`,
    );
    await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
  });
}

test("Chinese selection and completion persist without changing another track's completion", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Linux x86_64");
  await page.goto("/?collection=empty");
  await page.evaluate(() => {
    localStorage.setItem(
      "meiki-typing-completed",
      JSON.stringify(["typing-russian-top-row"]),
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
  await expect(languageChoices.getByRole("button")).toHaveText([
    "Korean — 2-set Hangul",
    "Japanese — Romaji input",
    "French — Dead-key accents",
    "Spanish — Dead-key accents",
    "German — Umlauts and ß",
    "Portuguese — Dead-key accents",
    /Russian — ЙЦУКЕН/,
    "Chinese — Pinyin input",
  ]);

  await page.getByRole("button", { name: "Chinese — Pinyin input" }).click();
  await page.getByRole("button", { name: "Start practice" }).click();
  await pressSequence(page.getByLabel("Practice input"), physicalSequences[0]);
  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toEqual(["typing-russian-top-row", "typing-chinese-basic-initials"]);
  expect(runtimeRequests).toEqual([]);
  expect(
    await page.evaluate(() => window.__MEIKI_TEST_REQUESTS__ ?? []),
  ).toEqual([]);

  await page.reload();
  await openTyping(page);
  await expect(
    page.getByRole("button", { name: "Chinese — Pinyin input" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByText("Basic initials", { exact: true })).toBeVisible();
  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toEqual(["typing-russian-top-row", "typing-chinese-basic-initials"]);
});

test("Chinese guidance uses current platform terminology and Linux remains non-prescriptive", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Linux x86_64");
  await page.goto("/");
  await openTyping(page);
  await page.getByRole("button", { name: "Chinese — Pinyin input" }).click();
  await expect(page.getByTestId("typing-linux-guidance")).toContainText(
    "desktop environment",
  );
  await expect(page.getByTestId("typing-linux-guidance")).not.toContainText(
    /Ctrl|Alt\+|Super|Command/,
  );

  await page.getByRole("button", { name: "Windows", exact: true }).click();
  await expect(
    page.getByText(
      "Add Chinese (Simplified, China) and use Microsoft Pinyin.",
      { exact: true },
    ),
  ).toBeVisible();
  await page.getByRole("button", { name: "macOS", exact: true }).click();
  await expect(
    page.getByText("Add Chinese, Simplified, and use Pinyin - Simplified.", {
      exact: true,
    }),
  ).toBeVisible();
});

test("the Chinese sandbox presents a non-scored candidate flow and Reset clears its state", async ({
  page,
}) => {
  await setRuntimePlatform(page, "MacIntel");
  await page.goto("/");
  await openTyping(page);
  await page.getByRole("button", { name: "Chinese — Pinyin input" }).click();
  const sandbox = page.locator(".conversion-sandbox");
  const input = page.getByLabel("Conversion sandbox input");

  await expect(
    sandbox.getByRole("heading", { name: "Pinyin conversion", level: 2 }),
  ).toBeVisible();
  await expect(sandbox.getByRole("listitem")).toHaveText([
    "Type Pinyin.",
    "Press Space to show candidates.",
    "Choose the intended candidate, then press Enter to accept.",
  ]);
  await expect(sandbox.getByRole("status")).toHaveText(
    "Type Pinyin with a Simplified Chinese input source.",
  );
  await expect(sandbox).toContainText(
    "Try any Pinyin reading. Candidate results depend on your installed Simplified Chinese IME.",
  );

  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "nihao");
  await pressSequence(input, ["KeyN", "KeyI"], true);
  await expect(sandbox.getByTestId("typing-key-Space")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await pressSequence(input, ["Space"], true);
  await expect(sandbox.getByTestId("typing-key-Enter")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await pressSequence(input, ["Digit2"], true);
  await dispatchKey(input, "keydown", "Enter", { isComposing: true });
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "你好";
  });
  await dispatchComposition(input, "compositionend", "你好");
  await dispatchKey(input, "keyup", "Enter", { isComposing: true });

  await expect(sandbox.getByTestId("conversion-physical-trail")).toHaveText(
    "N → I → Space → 2 → Enter",
  );
  await expect(sandbox.getByTestId("conversion-composition")).toHaveText(
    "None",
  );
  await expect(sandbox.getByTestId("conversion-committed-output")).toHaveText(
    "你好",
  );
  await expect(sandbox.getByRole("status")).toHaveText(
    "Candidate accepted: 你好. No candidate was scored.",
  );
  await expect(sandbox).not.toContainText(/Correct|Incorrect/);

  await page.getByRole("button", { name: "Reset sandbox" }).click();
  await expect(input).toHaveValue("");
  await expect(sandbox.getByTestId("conversion-physical-trail")).toHaveText(
    "None yet",
  );
  await expect(sandbox.getByTestId("conversion-composition")).toHaveText(
    "None",
  );
  await expect(sandbox.getByTestId("conversion-committed-output")).toHaveText(
    "None",
  );
  await expect(sandbox.getByRole("status")).toHaveText(
    "Type Pinyin with a Simplified Chinese input source.",
  );
});

test("Chinese remains Latin-only, keyboard-operable, accessible, and contained at narrow width", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 760 });
  await setRuntimePlatform(page, "MacIntel");
  await page.goto("/");
  await page.evaluate(() => {
    localStorage.setItem("meiki-vim-keybindings", "true");
  });
  await page.reload();
  await openTyping(page);
  await page.getByRole("button", { name: "Chinese — Pinyin input" }).click();
  const start = page.getByRole("button", { name: "Start practice" });
  await start.focus();
  await page.keyboard.press("Enter");
  const input = page.getByLabel("Practice input");
  await input.focus();
  await pressSequence(input, physicalSequences[0]);
  await input.blur();
  await page.keyboard.press("l");
  await expect(
    page.getByRole("heading", { name: "Finals and ü with V", level: 2 }),
  ).toBeVisible();
  await page.keyboard.press("r");
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "None yet",
  );
  await expect(page.getByLabel("Vim mode NORMAL")).toBeVisible();

  for (const keyboard of await page.getByTestId("typing-keyboard").all()) {
    await expect(
      keyboard.locator(".shifted-target-legend, .target-legend"),
    ).toHaveCount(0);
    await expect(keyboard.locator("button, [tabindex]")).toHaveCount(0);
  }
  for (const selector of [
    ".language-choices",
    ".practice",
    ".practice-header",
    ".target",
    ".practice-details",
    ".conversion-sandbox",
    ".conversion-steps",
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

  const accessibility = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(accessibility.violations).toEqual([]);

  await page.getByRole("button", { name: "Korean — 2-set Hangul" }).click();
  await expect(page.locator(".conversion-sandbox")).toHaveCount(0);
});
