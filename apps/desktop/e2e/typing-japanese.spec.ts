import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

const japaneseLessonIds = [
  "typing-japanese-basic-hiragana",
  "typing-japanese-basic-katakana",
  "typing-japanese-standalone-n",
  "typing-japanese-small-tsu",
  "typing-japanese-long-katakana-vowels",
  "typing-japanese-small-kana",
  "typing-japanese-short-words",
  "typing-japanese-short-phrases",
] as const;

const japaneseLessonTitles = [
  "Basic hiragana",
  "Basic katakana",
  "Standalone ん",
  "Small っ",
  "Long katakana vowels",
  "Small kana",
  "Short words",
  "Short phrases",
] as const;

const japaneseLessons = [
  {
    target: "あいうえお",
    sequence: ["KeyA", "KeyI", "KeyU", "KeyE", "KeyO"],
    visibleSequence: "A → I → U → E → O",
  },
  {
    target: "カタカナ",
    sequence: ["KeyK", "KeyA", "KeyT", "KeyA", "KeyK", "KeyA", "KeyN", "KeyA"],
    visibleSequence: "K → A → T → A → K → A → N → A",
  },
  {
    target: "ん",
    sequence: ["KeyN", "KeyN"],
    visibleSequence: "N → N",
  },
  {
    target: "きって",
    sequence: ["KeyK", "KeyI", "KeyT", "KeyT", "KeyE"],
    visibleSequence: "K → I → T → T → E",
  },
  {
    target: "コーヒー",
    sequence: ["KeyK", "KeyO", "Minus", "KeyH", "KeyI", "Minus"],
    visibleSequence: "K → O → - → H → I → -",
  },
  {
    target: "ゃぁ",
    sequence: ["KeyX", "KeyY", "KeyA", "KeyL", "KeyA"],
    visibleSequence: "X → Y → A → L → A",
  },
  {
    target: "にほんご",
    sequence: ["KeyN", "KeyI", "KeyH", "KeyO", "KeyN", "KeyG", "KeyO"],
    visibleSequence: "N → I → H → O → N → G → O",
  },
  {
    target: "にほんごです",
    sequence: [
      "KeyN",
      "KeyI",
      "KeyH",
      "KeyO",
      "KeyN",
      "KeyG",
      "KeyO",
      "KeyD",
      "KeyE",
      "KeyS",
      "KeyU",
    ],
    visibleSequence: "N → I → H → O → N → G → O → D → E → S → U",
  },
] as const;

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

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

async function openJapaneseLesson(
  page: Page,
  lessonIndex: number,
): Promise<Locator> {
  await setRuntimePlatform(page, "MacIntel");
  await page.goto("/");
  if (lessonIndex > 0) {
    await page.evaluate(
      (lessonIds) =>
        localStorage.setItem(
          "meiki-typing-completed",
          JSON.stringify(lessonIds),
        ),
      japaneseLessonIds.slice(0, lessonIndex),
    );
    await page.reload();
  }
  await openTyping(page);
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  await page.getByRole("button", { name: "Start practice" }).click();
  for (let index = 0; index < lessonIndex; index += 1) {
    await page.getByRole("button", { name: "Next" }).click();
  }
  await expect(
    page.getByRole("heading", {
      name: japaneseLessonTitles[lessonIndex],
      level: 2,
    }),
  ).toBeVisible();
  const input = page.getByLabel("Practice input");
  await expect(input).toBeVisible();
  return input;
}

function keyForCode(code: string): string {
  if (code.startsWith("Key")) return code.slice(3).toLowerCase();
  if (code === "Minus") return "-";
  if (code === "Space") return " ";
  return code;
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
  codes: readonly string[],
  committedText: string,
): Promise<void> {
  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", committedText);
  await pressSequence(input, codes);
  await input.evaluate((element, value) => {
    (element as HTMLInputElement).value = value;
  }, committedText);
  await dispatchComposition(input, "compositionend", committedText);
  await input.press("Enter");
}

test("offers eight ordered Japanese groups with prominent kana, complete romaji sequences, and Latin-only keys", async ({
  page,
}) => {
  for (let index = 0; index < japaneseLessons.length; index += 1) {
    await openJapaneseLesson(page, index);
    const practice = page.locator(".practice");
    await expect(practice.locator(".target strong")).toHaveText(
      japaneseLessons[index].target,
    );
    await expect(practice.getByTestId("typing-expected-sequence")).toHaveText(
      japaneseLessons[index].visibleSequence,
    );
    const keyboard = practice.getByTestId("typing-keyboard");
    await expect(
      keyboard.locator(".target-legend, .shifted-target-legend"),
    ).toHaveCount(0);
    await expect(keyboard.getByTestId("typing-key-KeyA")).toContainText("A");
  }

  const choices = page.getByRole("group", { name: "Language" });
  await expect(
    choices.getByRole("button", { name: "Japanese — Romaji input" }),
  ).toHaveCount(1);
  await expect(page.getByText(/JIS kana/i)).toHaveCount(0);
});

test("standalone nn remains ordinal and event.repeat cannot consume the second N", async ({
  page,
}) => {
  const input = await openJapaneseLesson(page, 2);
  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "ん");
  await dispatchKey(input, "keydown", "KeyN", { isComposing: true });
  await dispatchKey(input, "keydown", "KeyN", {
    isComposing: true,
    repeat: true,
  });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText("N");
  await expect(
    page.locator(".practice").getByTestId("typing-key-KeyN"),
  ).toHaveAttribute("data-expected", "true");
  await expect(page.locator("#typing-live-status")).toContainText("Next: N");
  await dispatchKey(input, "keyup", "KeyN", { isComposing: true });
  await dispatchKey(input, "keydown", "KeyN", { isComposing: true });
  await dispatchKey(input, "keyup", "KeyN", { isComposing: true });
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "ん";
  });
  await dispatchComposition(input, "compositionend", "ん");
  await input.press("Enter");
  await expect(page.locator("#typing-live-status")).toHaveText("Correct — ん");
});

test("kitte keeps the doubled T as next while partial IME output remains unchecked", async ({
  page,
}) => {
  const input = await openJapaneseLesson(page, 3);
  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "きっ");
  await pressSequence(input, ["KeyK", "KeyI"]);
  await dispatchKey(input, "keydown", "KeyT", { isComposing: true });

  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "K → I → T",
  );
  await expect(page.getByTestId("typing-composition")).toHaveText("きっ");
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await expect(
    page.locator(".practice").getByTestId("typing-key-KeyT"),
  ).toHaveAttribute("data-expected", "true");
  await expect(page.locator("#typing-live-status")).toContainText("Next: T");
  await expect(page.locator("#typing-live-status")).not.toHaveClass(
    /incorrect-feedback/,
  );
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();

  await dispatchKey(input, "keyup", "KeyT", { isComposing: true });
  await pressSequence(input, ["KeyT", "KeyE"]);
  await dispatchKey(input, "keydown", "Enter", { isComposing: true });
  await expect(page.locator("#typing-live-status")).toContainText(
    "only commit the active composition",
  );
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
  await dispatchKey(input, "keyup", "Enter", { isComposing: true });
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "きって";
  });
  await dispatchComposition(input, "compositionend", "きって");
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
  await input.press("Enter");
  await expect(page.locator("#typing-live-status")).toHaveText(
    "Correct — きって",
  );
});

test("ko-hi- uses Minus and xya plus an l-prefix produce committed small kana", async ({
  page,
}) => {
  let input = await openJapaneseLesson(page, 4);
  await commitLesson(input, japaneseLessons[4].sequence, "コーヒー");
  await expect(page.getByTestId("typing-physical-trail")).toContainText(
    "K → O → - → H → I → -",
  );
  await expect(page.locator("#typing-live-status")).toHaveText(
    "Correct — コーヒー",
  );

  await page.getByRole("button", { name: "Next" }).click();
  input = page.getByLabel("Practice input");
  await commitLesson(input, japaneseLessons[5].sequence, "ゃぁ");
  await expect(page.getByTestId("typing-physical-trail")).toContainText(
    "X → Y → A → L → A",
  );
  await expect(page.locator("#typing-live-status")).toHaveText(
    "Correct — ゃぁ",
  );
});

test("nihongo accepts decomposed kana by grapheme and persists only minimal local completion", async ({
  page,
}) => {
  const input = await openJapaneseLesson(page, 6);
  await page.evaluate(() => {
    window.__MEIKI_TEST_REQUESTS__ = [];
  });
  await commitLesson(input, japaneseLessons[6].sequence, "にほんこ\u3099");
  await expect(page.locator("#typing-live-status")).toHaveText(
    "Correct — にほんご",
  );
  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toContain("typing-japanese-short-words");
  expect(
    await page.evaluate(() =>
      Object.keys(localStorage)
        .filter((key) => key.startsWith("meiki-typing-"))
        .sort(),
    ),
  ).toEqual([
    "meiki-typing-completed",
    "meiki-typing-language",
    "meiki-typing-platform",
  ]);
  expect(
    await page.evaluate(() => window.__MEIKI_TEST_REQUESTS__ ?? []),
  ).toEqual([]);
});

test("the last phrase completes the ordered Japanese track and survives reload", async ({
  page,
}) => {
  const input = await openJapaneseLesson(page, 7);
  await commitLesson(input, japaneseLessons[7].sequence, "にほんごです");
  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toEqual(japaneseLessonIds);

  await page.reload();
  await openTyping(page);
  await expect(
    page.getByRole("button", { name: "Japanese — Romaji input" }),
  ).toContainText("Completed");
});

test("the local conversion sandbox highlights and announces Space then composition-safe Enter without scoring", async ({
  page,
}) => {
  await setRuntimePlatform(page, "MacIntel");
  await page.goto("/");
  await openTyping(page);
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  const sandbox = page.locator(".conversion-sandbox");
  const input = page.getByLabel("Conversion sandbox input");
  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "にほんご");
  await expect(sandbox.getByTestId("typing-key-Space")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await expect(sandbox.getByRole("status")).toContainText("Press Space");

  await dispatchKey(input, "keydown", "Space", { isComposing: true });
  await dispatchKey(input, "keyup", "Space", { isComposing: true });
  await expect(sandbox.getByTestId("typing-key-Enter")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await expect(sandbox.getByRole("status")).toContainText("Press Enter");
  await dispatchKey(input, "keydown", "Enter", { isComposing: true });
  await expect(sandbox.getByRole("status")).toContainText(
    "No candidate is scored",
  );
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "日本語";
  });
  await dispatchComposition(input, "compositionend", "日本語");
  await dispatchKey(input, "keyup", "Enter", { isComposing: true });

  await expect(sandbox.getByTestId("conversion-physical-trail")).toHaveText(
    "Space → Enter",
  );
  await expect(sandbox.getByTestId("conversion-composition")).toHaveText(
    "None",
  );
  await expect(sandbox.getByTestId("conversion-committed-output")).toHaveText(
    "日本語",
  );
  await expect(sandbox.getByRole("status")).toHaveText(
    "Candidate accepted: 日本語. No candidate was scored.",
  );
  await expect(sandbox).not.toContainText(/Correct|Incorrect/);

  const accessibility = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(accessibility.violations).toEqual([]);
});

test("sandbox controls are keyboard operable and Reset returns to the reading step", async ({
  page,
}) => {
  await setRuntimePlatform(page, "MacIntel");
  await page.goto("/");
  await openTyping(page);
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  const sandbox = page.locator(".conversion-sandbox");
  const input = page.getByLabel("Conversion sandbox input");
  await input.focus();
  await input.fill("nihongo");
  await page.keyboard.press("Space");
  await page.keyboard.press("Enter");
  await expect(sandbox.getByRole("status")).toContainText(
    "No candidate was scored",
  );
  const reset = page.getByRole("button", { name: "Reset sandbox" });
  await reset.focus();
  await page.keyboard.press("Enter");
  await expect(input).toHaveValue("");
  await expect(sandbox.getByRole("status")).toHaveText(
    "Type a reading with Japanese romaji input.",
  );
});

test("uses the required Japanese platform guidance and keeps Linux non-prescriptive", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Win32");
  await page.goto("/");
  await openTyping(page);
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  await expect(
    page.getByText(
      "Use Microsoft Japanese IME with English 101/102-key hardware.",
      { exact: true },
    ),
  ).toBeVisible();

  await page.getByRole("button", { name: "macOS", exact: true }).click();
  await expect(
    page.getByText(
      "Use romaji input and enable “Use Caps Lock to switch to and from ABC.”",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    page.locator(".conversion-sandbox").getByTestId("typing-key-CapsLock"),
  ).toBeVisible();

  await page.evaluate(() => localStorage.clear());
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "platform", {
      configurable: true,
      get: () => "Linux x86_64",
    });
    Object.defineProperty(navigator, "userAgentData", {
      configurable: true,
      get: () => ({ platform: "Linux" }),
    });
  });
  await page.reload();
  await openTyping(page);
  await expect(page.getByTestId("typing-linux-guidance")).toContainText(
    "Configure the language input source through your desktop environment.",
  );
});

test("Japanese practice and sandbox fit a narrow dark viewport without page overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 760 });
  await setRuntimePlatform(page, "MacIntel");
  await page.goto("/");
  await page.getByRole("button", { name: "Theme" }).click();
  await page.getByRole("option", { name: "Dark", exact: true }).click();
  await openTyping(page);
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  await page.getByRole("button", { name: "Start practice" }).click();

  for (const selector of [
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
});
