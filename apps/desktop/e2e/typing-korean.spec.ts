import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

const koreanLessonIds = [
  "typing-korean-basic-consonants",
  "typing-korean-basic-vowels",
  "typing-korean-shift-forms",
  "typing-korean-compound-vowels",
  "typing-korean-syllable-blocks",
  "typing-korean-short-words",
  "typing-korean-short-phrases",
] as const;

const koreanLessonTitles = [
  "Basic consonants",
  "Basic vowels",
  "Shift forms",
  "Compound vowels",
  "Syllable-block assembly",
  "Short words",
  "Short phrases",
] as const;

const basicConsonantCodes = [
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
] as const;

const basicVowelCodes = [
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
] as const;

const compoundVowelCodes = [
  "KeyH",
  "KeyK",
  "KeyH",
  "KeyL",
  "KeyN",
  "KeyJ",
  "KeyM",
  "KeyL",
] as const;

const shortWordCodes = [
  "KeyD",
  "KeyK",
  "KeyS",
  "KeyS",
  "KeyU",
  "KeyD",
] as const;

const shortPhraseCodes = [
  ...shortWordCodes,
  "Space",
  "KeyC",
  "KeyL",
  "KeyS",
  "KeyR",
  "KeyN",
] as const;

const baseMappings = {
  KeyQ: "ㅂ",
  KeyW: "ㅈ",
  KeyE: "ㄷ",
  KeyR: "ㄱ",
  KeyT: "ㅅ",
  KeyY: "ㅛ",
  KeyU: "ㅕ",
  KeyI: "ㅑ",
  KeyO: "ㅐ",
  KeyP: "ㅔ",
  KeyA: "ㅁ",
  KeyS: "ㄴ",
  KeyD: "ㅇ",
  KeyF: "ㄹ",
  KeyG: "ㅎ",
  KeyH: "ㅗ",
  KeyJ: "ㅓ",
  KeyK: "ㅏ",
  KeyL: "ㅣ",
  KeyZ: "ㅋ",
  KeyX: "ㅌ",
  KeyC: "ㅊ",
  KeyV: "ㅍ",
  KeyB: "ㅠ",
  KeyN: "ㅜ",
  KeyM: "ㅡ",
} as const;

const shiftedMappings = {
  KeyQ: "ㅃ",
  KeyW: "ㅉ",
  KeyE: "ㄸ",
  KeyR: "ㄲ",
  KeyT: "ㅆ",
  KeyO: "ㅒ",
  KeyP: "ㅖ",
} as const;

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

async function openKoreanLesson(
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
      koreanLessonIds.slice(0, lessonIndex),
    );
    await page.reload();
  }
  await openTyping(page);
  await page.getByRole("button", { name: "Korean — 2-set Hangul" }).click();
  await page.getByRole("button", { name: "Start practice" }).click();
  for (let index = 0; index < lessonIndex; index += 1) {
    await page.getByRole("button", { name: "Next" }).click();
  }
  await expect(
    page.getByRole("heading", {
      name: koreanLessonTitles[lessonIndex],
      level: 2,
    }),
  ).toBeVisible();
  const input = page.getByLabel("Practice input");
  await expect(input).toBeVisible();
  return input;
}

function keyForCode(code: string): string {
  if (code.startsWith("Key")) return code.slice(3).toLowerCase();
  if (code === "ShiftLeft") return "Shift";
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

test("renders every Korean base and shifted mapping above its Latin physical key", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 760 });
  await openKoreanLesson(page, 0);
  const keyboard = page.getByTestId("typing-keyboard");

  await expect(keyboard.locator(".target-legend")).toHaveCount(26);
  await expect(keyboard.locator(".shifted-target-legend")).toHaveCount(7);
  for (const [code, jamo] of Object.entries(baseMappings)) {
    const keycap = page.getByTestId(`typing-key-${code}`);
    await expect(keycap.locator(".target-legend")).toHaveText(jamo);
    await expect(keycap.locator(".latin-legend")).toHaveText(code.slice(3));
  }
  for (const [code, jamo] of Object.entries(shiftedMappings)) {
    await expect(
      page.getByTestId(`typing-key-${code}`).locator(".shifted-target-legend"),
    ).toHaveText(jamo);
  }

  const q = page.getByTestId("typing-key-KeyQ");
  const legends = q.locator(
    ".shifted-target-legend, .target-legend, .latin-legend",
  );
  await expect(legends).toHaveText(["ㅃ", "ㅂ", "Q"]);
  const legendMetrics = await legends.evaluateAll((elements) =>
    elements.map((element) => {
      const bounds = element.getBoundingClientRect();
      return {
        center: (bounds.top + bounds.bottom) / 2,
        fontSize: Number.parseFloat(getComputedStyle(element).fontSize),
      };
    }),
  );
  expect(legendMetrics[0].center).toBeLessThan(legendMetrics[1].center);
  expect(legendMetrics[1].center).toBeLessThan(legendMetrics[2].center);
  expect(legendMetrics[0].fontSize).toBeGreaterThan(legendMetrics[2].fontSize);
  expect(legendMetrics[1].fontSize).toBeGreaterThan(legendMetrics[2].fontSize);
  for (const compound of ["ㅘ", "ㅚ", "ㅝ", "ㅢ"]) {
    await expect(keyboard.getByText(compound, { exact: true })).toHaveCount(0);
  }
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});

test("the Korean target stays inside the practice header at the CI narrow viewport", async ({
  page,
}) => {
  await page.setViewportSize({ width: 640, height: 720 });
  await openKoreanLesson(page, 0);

  const headerBounds = await page.locator(".practice-header").boundingBox();
  const targetBounds = await page.locator(".target").boundingBox();
  const targetTextBounds = await page.locator(".target strong").boundingBox();

  expect(headerBounds).not.toBeNull();
  expect(targetBounds).not.toBeNull();
  expect(targetTextBounds).not.toBeNull();
  if (!headerBounds || !targetBounds || !targetTextBounds) {
    throw new Error("The Korean practice header must have measurable bounds.");
  }
  expect(targetBounds.x).toBeGreaterThanOrEqual(headerBounds.x);
  expect(targetBounds.x + targetBounds.width).toBeLessThanOrEqual(
    headerBounds.x + headerBounds.width,
  );
  expect(targetTextBounds.x).toBeGreaterThanOrEqual(targetBounds.x);
  expect(targetTextBounds.x + targetTextBounds.width).toBeLessThanOrEqual(
    targetBounds.x + targetBounds.width,
  );
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});

test("advances consonants, vowels, held Shift forms, and real compound-vowel sequences in order", async ({
  page,
}) => {
  let input = await openKoreanLesson(page, 0);
  await pressSequence(input, basicConsonantCodes);
  await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
  await page.getByRole("button", { name: "Next" }).click();
  await expect(
    page.getByRole("heading", { name: "Basic vowels", level: 2 }),
  ).toBeVisible();

  input = page.getByLabel("Practice input");
  await pressSequence(input, basicVowelCodes);
  await page.getByRole("button", { name: "Next" }).click();
  await expect(
    page.getByRole("heading", { name: "Shift forms", level: 2 }),
  ).toBeVisible();

  input = page.getByLabel("Practice input");
  const shift = page.getByTestId("typing-key-ShiftLeft");
  await dispatchKey(input, "keydown", "ShiftLeft");
  await expect(shift).toHaveAttribute("data-held", "true");
  await pressSequence(input, [
    "KeyQ",
    "KeyW",
    "KeyE",
    "KeyR",
    "KeyT",
    "KeyO",
    "KeyP",
  ]);
  await expect(shift).toHaveAttribute("data-held", "true");
  await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
  await dispatchKey(input, "keyup", "ShiftLeft");
  await expect(shift).toHaveAttribute("data-held", "false");
  await page.getByRole("button", { name: "Next" }).click();
  await expect(
    page.getByRole("heading", { name: "Compound vowels", level: 2 }),
  ).toBeVisible();

  input = page.getByLabel("Practice input");
  await expect(page.getByTestId("typing-expected-sequence")).toHaveText(
    "H → K → H → L → N → J → M → L",
  );
  await pressSequence(input, compoundVowelCodes);
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "H → K → H → L → N → J → M → L",
  );
  await page.getByRole("button", { name: "Next" }).click();
  await expect(
    page.getByRole("heading", {
      name: "Syllable-block assembly",
      level: 2,
    }),
  ).toBeVisible();
});

test("wrong and repeated physical keys remain ordinal while Retry clears transient state", async ({
  page,
}) => {
  const input = await openKoreanLesson(page, 0);
  const feedback = page.locator("#typing-live-status");

  await dispatchKey(input, "keydown", "KeyY");
  await expect(feedback).toHaveText("Pressed Y. Expected Q. Try again.");
  await expect(page.getByTestId("typing-key-KeyY")).toHaveAttribute(
    "data-incorrect",
    "true",
  );
  await dispatchKey(input, "keyup", "KeyY");

  await dispatchKey(input, "keydown", "KeyQ");
  await dispatchKey(input, "keydown", "KeyQ", { repeat: true });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText("Y → Q");
  await expect(page.getByTestId("typing-key-KeyW")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await dispatchKey(input, "keyup", "KeyQ");

  await page.getByRole("button", { name: "Retry" }).click();
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "None yet",
  );
  await expect(page.getByTestId("typing-composition")).toHaveText("None");
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await expect(page.getByTestId("typing-key-KeyQ")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await expect(page.getByTestId("typing-key-KeyQ")).toHaveAttribute(
    "data-correct",
    "false",
  );
  await expect(page.getByTestId("typing-key-KeyY")).toHaveAttribute(
    "data-incorrect",
    "false",
  );
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
});

test("D K S S U D stays separate from composition until 안녕 is committed", async ({
  page,
}) => {
  const input = await openKoreanLesson(page, 5);
  const feedback = page.locator("#typing-live-status");
  const next = page.getByRole("button", { name: "Next" });

  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "안");
  await pressSequence(input, ["KeyD", "KeyK"], true);
  await dispatchKey(input, "keydown", "KeyS", { isComposing: true });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "D → K → S",
  );
  await expect(page.getByTestId("typing-composition")).toHaveText("안");
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await expect(page.getByTestId("typing-key-KeyS")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await expect(feedback).not.toHaveClass(/incorrect-feedback/);
  await expect(next).toBeDisabled();
  await dispatchKey(input, "keydown", "KeyS", {
    isComposing: true,
    repeat: true,
  });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "D → K → S",
  );
  await dispatchKey(input, "keyup", "KeyS", { isComposing: true });
  await pressSequence(input, ["KeyS", "KeyU", "KeyD"], true);
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "D → K → S → S → U → D",
  );
  await expect(feedback).toContainText("Physical sequence complete");
  await expect(next).toBeDisabled();

  await dispatchKey(input, "keydown", "Enter", { isComposing: true });
  await expect(feedback).toContainText("only commit the active composition");
  await expect(next).toBeDisabled();
  await dispatchKey(input, "keyup", "Enter", { isComposing: true });
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "안녕";
  });
  await dispatchComposition(input, "compositionend", "안녕");
  await expect(page.getByTestId("typing-composition")).toHaveText("None");
  await expect(page.getByTestId("typing-committed-output")).toHaveText("안녕");
  await expect(next).toBeDisabled();
  await input.press("Enter");
  await expect(feedback).toHaveText("Correct — 안녕");
  await expect(next).toBeEnabled();
  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toContain("typing-korean-short-words");
  await next.click();
  await expect(
    page.getByRole("heading", { name: "Short phrases", level: 2 }),
  ).toBeVisible();
});

test("committed Korean syllables compare by grapheme and remain accessible", async ({
  page,
}) => {
  const input = await openKoreanLesson(page, 4);
  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "하");
  await pressSequence(input, ["KeyG", "KeyK", "KeyS"], true);
  await finishComposition(input, "한");

  await expect(page.locator("#typing-live-status")).toHaveText("Correct — 한");
  await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
  const accessibility = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(accessibility.violations).toEqual([]);
});

test("the short phrase completes all seven lessons with only minimal local progress", async ({
  page,
}) => {
  const input = await openKoreanLesson(page, 6);
  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "안녕 친");
  await pressSequence(input, shortPhraseCodes, true);
  await finishComposition(input, "안녕 친구");
  await expect(page.locator("#typing-live-status")).toHaveText(
    "Correct — 안녕 친구",
  );

  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toEqual(koreanLessonIds);
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

  await page.reload();
  await openTyping(page);
  await expect(
    page.getByRole("button", { name: "Korean — 2-set Hangul" }),
  ).toContainText("Completed");
});

test("uses the required Windows and macOS Korean switching guidance", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Win32");
  await page.goto("/");
  await openTyping(page);
  await expect(
    page.getByText(
      "Use 2-set Korean. On standard US hardware, use Right Alt for 한/영 switching.",
      { exact: true },
    ),
  ).toBeVisible();

  await page.getByRole("button", { name: "macOS", exact: true }).click();
  await expect(
    page.getByText(
      "Enable Korean input. On standard US hardware, use Right Command for switching.",
      { exact: true },
    ),
  ).toBeVisible();
});

test("the Korean physical sequence and progression are keyboard operable", async ({
  page,
}) => {
  await setRuntimePlatform(page, "MacIntel");
  await page.goto("/");
  await openTyping(page);
  const start = page.getByRole("button", { name: "Start practice" });
  await start.focus();
  await page.keyboard.press("Enter");
  const input = page.getByLabel("Practice input");
  await input.focus();
  for (const code of basicConsonantCodes) {
    await page.keyboard.press(code.slice(3).toLowerCase());
  }
  const next = page.getByRole("button", { name: "Next" });
  await next.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Basic vowels", level: 2 }),
  ).toBeVisible();
});
