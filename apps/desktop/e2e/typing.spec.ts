import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

const trackNames = [
  "Korean — 2-set Hangul",
  "Japanese — Romaji input",
  "French — Dead-key accents",
  "Spanish — Dead-key accents",
  "German — Umlauts and ß",
  "Portuguese — Dead-key accents",
] as const;

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function openTyping(page: Page, route = "/"): Promise<void> {
  await page.goto(route);
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

async function startPractice(page: Page): Promise<Locator> {
  await page.getByRole("button", { name: "Start practice" }).click();
  const input = page.getByLabel("Practice input");
  await expect(input).toBeVisible();
  return input;
}

async function dispatchKey(
  input: Locator,
  type: "keydown" | "keyup",
  init: {
    code: string;
    key: string;
    repeat?: boolean;
    isComposing?: boolean;
  },
): Promise<void> {
  await input.evaluate(
    (element, event) =>
      element.dispatchEvent(
        new KeyboardEvent(event.type, {
          bubbles: true,
          code: event.code,
          key: event.key,
          repeat: event.repeat,
          isComposing: event.isComposing,
        }),
      ),
    { type, ...init },
  );
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

test("desktop and mobile navigation keep Typing between Add and Settings and preserve the editor guard", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1200, height: 800 });
  await page.goto("/");
  const desktopNavigation = page
    .locator("aside")
    .getByRole("navigation", { name: "Primary navigation" });
  await expect(desktopNavigation.getByRole("button")).toHaveText([
    "Today",
    "Decks",
    "Add",
    "Typing",
    "Settings",
  ]);

  await desktopNavigation.getByRole("button", { name: "Add" }).click();
  const source = page.locator(".segment-text");
  await source.fill("Unsaved typing guard");
  const typingNavigation = desktopNavigation.getByRole("button", {
    name: "Typing",
  });
  await typingNavigation.click();
  const discard = page.getByRole("alertdialog", {
    name: "Discard unsaved changes?",
  });
  await expect(discard).toBeVisible();
  await discard.getByRole("button", { name: "Keep editing" }).click();
  await expect(
    page.getByRole("heading", { name: "Add / Edit card", level: 1 }),
  ).toBeVisible();
  await expect(typingNavigation).toBeFocused();
  await typingNavigation.click();
  await discard.getByRole("button", { name: "Discard changes" }).click();
  await expect(
    page.getByRole("heading", { name: "Typing", level: 1 }),
  ).toBeVisible();
  await expect(page.locator("main")).toBeFocused();

  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await page.getByRole("button", { name: "Open navigation" }).click();
  const mobileNavigation = page.getByRole("navigation", {
    name: "Primary navigation",
  });
  await expect(mobileNavigation.getByRole("button")).toHaveText([
    "Today",
    "Decks",
    "Add",
    "Typing",
    "Settings",
  ]);
});

test("the empty collection offers exactly six static local tracks without network or backend work", async ({
  page,
}) => {
  await page.goto("/?collection=empty");
  await page.evaluate(() => {
    window.__MEIKI_TEST_REQUESTS__ = [];
  });
  const runtimeRequests: string[] = [];
  page.on("request", (request) => {
    if (["fetch", "xhr"].includes(request.resourceType())) {
      runtimeRequests.push(request.url());
    }
  });
  const openNavigation = page.getByRole("button", { name: "Open navigation" });
  if (await openNavigation.isVisible()) await openNavigation.click();
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Typing" })
    .click();

  const languageChoices = page.getByRole("group", { name: "Language" });
  await expect(languageChoices.getByRole("button")).toHaveCount(6);
  for (const name of trackNames) {
    await expect(languageChoices.getByRole("button", { name })).toBeVisible();
  }
  await expect(page.getByText(/Chinese|Arabic|Russian/i)).toHaveCount(0);
  await expect(page.locator("main")).toContainText(
    "does not require a deck or bundle",
  );
  await startPractice(page);
  expect(runtimeRequests).toEqual([]);
  expect(
    await page.evaluate(() => window.__MEIKI_TEST_REQUESTS__ ?? []),
  ).toEqual([]);
});

test("two static lessons in one language share one track and advance with separate completion IDs", async ({
  page,
}) => {
  await page.goto("/e2e/fixtures/typing-multiple-lessons.html");
  await expect(
    page.getByRole("heading", { name: "Typing", level: 1 }),
  ).toBeVisible();

  const languageChoices = page.getByRole("group", { name: "Language" });
  await expect(languageChoices.getByRole("button")).toHaveCount(6);
  await expect(
    languageChoices.getByRole("button", {
      name: "Korean — 2-set Hangul",
    }),
  ).toHaveCount(1);

  let input = await startPractice(page);
  await dispatchKey(input, "keydown", { code: "KeyD", key: "d" });
  await dispatchKey(input, "keyup", { code: "KeyD", key: "d" });
  await dispatchKey(input, "keydown", { code: "KeyK", key: "k" });
  await dispatchKey(input, "keyup", { code: "KeyK", key: "k" });
  await page.getByRole("button", { name: "Next" }).click();

  await expect(
    page.getByRole("heading", {
      name: "Second Korean fixture lesson",
      level: 2,
    }),
  ).toBeVisible();
  input = page.getByLabel("Practice input");
  await dispatchKey(input, "keydown", { code: "KeyD", key: "d" });
  await dispatchKey(input, "keyup", { code: "KeyD", key: "d" });
  await dispatchKey(input, "keydown", { code: "KeyK", key: "k" });
  await dispatchKey(input, "keyup", { code: "KeyK", key: "k" });

  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toEqual(["typing-korean-basic-consonants", "typing-korean-fixture-second"]);
});

for (const track of [
  {
    language: "german",
    label: "German — Umlauts and ß",
    firstLesson: "Umlaut ä",
  },
  {
    language: "portuguese",
    label: "Portuguese — Dead-key accents",
    firstLesson: "Acute á",
  },
] as const) {
  test(`${track.language} selection persists independently across reload`, async ({
    page,
  }) => {
    await openTyping(page);
    await page.getByRole("button", { name: track.label }).click();

    expect(
      await page.evaluate(() => localStorage.getItem("meiki-typing-language")),
    ).toBe(track.language);
    await expect(
      page.getByText(track.firstLesson, { exact: true }),
    ).toBeVisible();

    await page.reload();
    await openTyping(page);
    await expect(
      page.getByRole("button", { name: track.label }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(
      page.getByText(track.firstLesson, { exact: true }),
    ).toBeVisible();
  });
}

for (const detected of [
  { runtime: "MacIntel", label: "macOS", stored: "macos" },
  { runtime: "Win32", label: "Windows", stored: "windows" },
] as const) {
  test(`reliably detects ${detected.label} instructions`, async ({ page }) => {
    await setRuntimePlatform(page, detected.runtime);
    await openTyping(page);

    await expect(
      page.getByRole("button", { name: detected.label, exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(
      page.getByText(`${detected.label} was detected.`),
    ).toBeVisible();
    expect(
      await page.evaluate(() => localStorage.getItem("meiki-typing-platform")),
    ).toBe(detected.stored);
  });
}

test("manual platform override persists while Linux keeps non-prescriptive guidance and available exercises", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Linux x86_64");
  await openTyping(page);

  const guidance = page.getByTestId("typing-linux-guidance");
  await expect(guidance).toContainText("varies on Linux");
  await expect(guidance).toContainText("desktop environment");
  await expect(guidance).not.toContainText(/Ctrl|Alt\+|Super/);
  await expect(
    page.getByRole("button", { name: "Start practice" }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "macOS", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "macOS", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-typing-platform")),
  ).toBe("macos");

  await page.reload();
  await openTyping(page);
  await expect(
    page.getByRole("button", { name: "macOS", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(guidance).toBeVisible();
  await expect(page.getByText(/Hold Option while pressing E/)).toHaveCount(0);
});

test("physical drills expose live, held, repeated, ordered, and completed key states", async ({
  page,
}) => {
  await page.goto("/e2e/fixtures/typing-multiple-lessons.html");
  await expect(
    page.getByRole("heading", { name: "Typing", level: 1 }),
  ).toBeVisible();
  const input = await startPractice(page);

  for (const modifier of [
    { code: "ShiftLeft", key: "Shift" },
    { code: "AltRight", key: "AltGraph" },
    { code: "AltLeft", key: "Alt" },
  ]) {
    const keycap = page.getByTestId(`typing-key-${modifier.code}`);
    await dispatchKey(input, "keydown", modifier);
    await expect(keycap).toHaveAttribute("data-pressed", "true");
    await expect(keycap).toHaveAttribute("data-held", "true");
    await dispatchKey(input, "keyup", modifier);
    await expect(keycap).toHaveAttribute("data-pressed", "false");
    await page.getByRole("button", { name: "Retry" }).click();
  }

  const wrongKey = page.getByTestId("typing-key-KeyF");
  await dispatchKey(input, "keydown", { code: "KeyF", key: "f" });
  await expect(wrongKey).toHaveAttribute("data-incorrect", "true");
  await expect(page.locator("#typing-live-status")).toContainText(
    "Pressed F. Expected D. Try again.",
  );
  await dispatchKey(input, "keyup", { code: "KeyF", key: "f" });

  const d = page.getByTestId("typing-key-KeyD");
  await dispatchKey(input, "keydown", { code: "KeyD", key: "d" });
  await expect(d).toHaveAttribute("data-pressed", "true");
  await expect(d).toHaveAttribute("data-correct", "true");
  await expect(page.getByTestId("typing-key-KeyK")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await dispatchKey(input, "keydown", {
    code: "KeyD",
    key: "d",
    repeat: true,
  });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText("F → D");
  await dispatchKey(input, "keyup", { code: "KeyD", key: "d" });

  const k = page.getByTestId("typing-key-KeyK");
  await dispatchKey(input, "keydown", { code: "KeyK", key: "k" });
  await expect(k).toHaveAttribute("data-pressed", "true");
  await expect(page.locator("#typing-live-status")).toContainText(
    "Correct — 아",
  );
  await dispatchKey(input, "keyup", { code: "KeyK", key: "k" });
  await expect(d).toHaveAttribute("data-completed", "true");
  await expect(k).toHaveAttribute("data-completed", "true");
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "F → D → K",
  );
  await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
});

test("a repeated physical code stays expected after its earlier ordinal is completed", async ({
  page,
}) => {
  await setRuntimePlatform(page, "MacIntel");
  await openTyping(page);
  await page.getByRole("button", { name: "French — Dead-key accents" }).click();
  const input = await startPractice(page);
  const e = page.getByTestId("typing-key-KeyE");

  await dispatchKey(input, "keydown", { code: "AltLeft", key: "Alt" });
  await dispatchKey(input, "keyup", { code: "AltLeft", key: "Alt" });
  await dispatchKey(input, "keydown", { code: "KeyE", key: "e" });

  await expect(e).toHaveAttribute("data-correct", "true");
  await expect(e).toHaveAttribute("data-expected", "true");
  await expect(e.locator(".key-marker")).toHaveText("→");
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "Option → E",
  );
  await expect(page.locator("#typing-live-status")).toHaveText(
    "Correct position. Next: E.",
  );

  await dispatchKey(input, "keyup", { code: "KeyE", key: "e" });
  await dispatchKey(input, "keydown", { code: "KeyE", key: "e" });
  await expect(e).toHaveAttribute("data-expected", "false");
  await expect(e).toHaveAttribute("data-completed", "true");
  await expect(e.locator(".key-marker")).toHaveText("✓");
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "Option → E → E",
  );
  await expect(page.locator("#typing-live-status")).toHaveText(
    "Physical sequence complete. Commit the target text.",
  );
});

test("IME composition advances the ordinal physical sequence but defers completion", async ({
  page,
}) => {
  await setRuntimePlatform(page, "MacIntel");
  await openTyping(page);
  await page.getByRole("button", { name: "French — Dead-key accents" }).click();
  const input = await startPractice(page);
  const feedback = page.locator("#typing-live-status");
  const next = page.getByRole("button", { name: "Next" });

  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "e");
  await dispatchKey(input, "keydown", {
    code: "KeyF",
    key: "f",
    isComposing: true,
  });
  await expect(feedback).toContainText("Expected Option");
  await expect(feedback).not.toHaveClass(/incorrect-feedback/);
  await expect(page.getByTestId("typing-composition")).toHaveText("e");
  await expect(next).toBeDisabled();
  await dispatchKey(input, "keyup", { code: "KeyF", key: "f" });

  await dispatchKey(input, "keydown", {
    code: "AltLeft",
    key: "Alt",
    isComposing: true,
  });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "F → Option",
  );
  await expect(page.getByTestId("typing-key-KeyE")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await expect(feedback).toHaveText(
    "Correct position. Next: E. Composition remains unchecked.",
  );
  await expect(next).toBeDisabled();
  await dispatchKey(input, "keyup", { code: "AltLeft", key: "Alt" });

  await dispatchKey(input, "keydown", {
    code: "KeyE",
    key: "e",
    isComposing: true,
  });
  await expect(page.getByTestId("typing-key-KeyE")).toHaveAttribute(
    "data-expected",
    "true",
  );
  await expect(feedback).toHaveText(
    "Correct position. Next: E. Composition remains unchecked.",
  );
  await dispatchKey(input, "keyup", { code: "KeyE", key: "e" });
  await dispatchKey(input, "keydown", {
    code: "KeyE",
    key: "e",
    isComposing: true,
  });
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "F → Option → E → E",
  );
  await expect(feedback).toHaveText(
    "Physical sequence complete. Commit the target text. Composition remains unchecked.",
  );
  await expect(next).toBeDisabled();

  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "é";
  });
  await dispatchComposition(input, "compositionend", "é");
  await expect(next).toBeDisabled();
  await input.press("Enter");
  await expect(feedback).toHaveText("Correct — é");
  await expect(next).toBeEnabled();
});

test("IME composition stays separate and its committing Enter never submits prematurely", async ({
  page,
}) => {
  await openTyping(page);
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  const input = await startPractice(page);

  await dispatchComposition(input, "compositionstart", "");
  await dispatchComposition(input, "compositionupdate", "あいうえお");
  for (const [code, key] of [
    ["KeyA", "a"],
    ["KeyI", "i"],
    ["KeyU", "u"],
    ["KeyE", "e"],
    ["KeyO", "o"],
  ]) {
    await dispatchKey(input, "keydown", {
      code,
      key,
      isComposing: true,
    });
    await dispatchKey(input, "keyup", { code, key, isComposing: true });
  }
  await expect(page.getByTestId("typing-composition")).toHaveText("あいうえお");
  await expect(page.getByTestId("typing-committed-output")).toHaveText("None");
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
  await expect(page.locator("#typing-live-status")).not.toContainText(
    /Not yet|Try again/,
  );

  await dispatchKey(input, "keydown", {
    code: "Enter",
    key: "Enter",
    isComposing: true,
  });
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "あいうえお";
  });
  await dispatchComposition(input, "compositionend", "あいうえお");
  await dispatchKey(input, "keyup", { code: "Enter", key: "Enter" });
  await expect(page.getByTestId("typing-composition")).toHaveText("None");
  await expect(page.getByTestId("typing-committed-output")).toHaveText(
    "あいうえお",
  );
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();

  await input.press("Enter");
  await expect(page.locator("#typing-live-status")).toContainText(
    "Correct — あいうえお",
  );
  await expect(page.getByRole("button", { name: "Next" })).toBeEnabled();
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-typing-language")),
  ).toBe("japanese");
  expect(
    JSON.parse(
      (await page.evaluate(() =>
        localStorage.getItem("meiki-typing-completed"),
      )) ?? "[]",
    ),
  ).toContain("typing-japanese-basic-hiragana");

  await page.reload();
  await openTyping(page);
  await expect(
    page.getByRole("button", { name: "Japanese — Romaji input" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByText("Basic hiragana", { exact: true })).toBeVisible();
});

test("committed-text comparison treats a decomposed accent as one matching grapheme", async ({
  page,
}) => {
  await openTyping(page);
  await page.getByRole("button", { name: "French — Dead-key accents" }).click();
  const input = await startPractice(page);
  await input.fill("e\u0301");
  await input.press("Enter");

  await expect(page.getByTestId("typing-committed-output")).toHaveText(
    "e\u0301",
  );
  await expect(page.locator("#typing-live-status")).toContainText(
    "Correct — é",
  );
});

test("the presentation-only keyboard keeps required staggered rows and no narrow overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 760 });
  await openTyping(page);
  await startPractice(page);
  const keyboard = page.getByTestId("typing-keyboard");
  await expect(keyboard.locator("[data-keyboard-row]")).toHaveCount(5);
  for (const code of [
    "Digit1",
    "KeyQ",
    "KeyA",
    "KeyZ",
    "Quote",
    "ShiftLeft",
    "AltRight",
    "AltLeft",
    "CapsLock",
    "Space",
    "Enter",
    "Backspace",
  ]) {
    await expect(page.getByTestId(`typing-key-${code}`)).toBeVisible();
  }
  await expect(keyboard.locator("button")).toHaveCount(0);
  await expect(keyboard.locator("[tabindex]")).toHaveCount(0);
  await expect(page.getByTestId("typing-key-KeyD")).toContainText("D");
  await expect(page.getByTestId("typing-key-KeyD")).toContainText("ㅇ");
  const q = page.getByTestId("typing-key-KeyQ");
  const qLegends = q.locator(
    ".shifted-target-legend, .target-legend, .latin-legend",
  );
  await expect(qLegends).toHaveText(["ㅃ", "ㅂ", "Q"]);
  const [qBox, qLegendBoxes] = await Promise.all([
    q.evaluate((element) => {
      const { top, bottom } = element.getBoundingClientRect();
      return { top, bottom };
    }),
    qLegends.evaluateAll((elements) =>
      elements.map((element) => {
        const { top, bottom } = element.getBoundingClientRect();
        return { top, bottom, center: (top + bottom) / 2 };
      }),
    ),
  ]);
  expect(qLegendBoxes[0].center).toBeLessThan(qLegendBoxes[1].center);
  expect(qLegendBoxes[1].center).toBeLessThan(qLegendBoxes[2].center);
  for (const box of qLegendBoxes) {
    expect(box.bottom).toBeLessThanOrEqual(qBox.bottom + 1);
    expect(box.top).toBeGreaterThanOrEqual(qBox.top - 1);
  }
  const [numberX, qwertyX] = await Promise.all([
    page
      .getByTestId("typing-key-Digit1")
      .evaluate((element) => element.getBoundingClientRect().x),
    page
      .getByTestId("typing-key-KeyQ")
      .evaluate((element) => element.getBoundingClientRect().x),
  ]);
  expect(qwertyX).toBeGreaterThan(numberX);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);

  await page.getByRole("button", { name: "Retry" }).click();
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  await startPractice(page);
  await expect(
    page
      .getByTestId("typing-keyboard")
      .locator(".shifted-target-legend, .target-legend"),
  ).toHaveCount(0);
});

test("keyboard-only Retry and Next retain live semantics and pass accessibility checks", async ({
  page,
}) => {
  await page.goto("/e2e/fixtures/typing-multiple-lessons.html");
  await expect(
    page.getByRole("heading", { name: "Typing", level: 1 }),
  ).toBeVisible();
  const start = page.getByRole("button", { name: "Start practice" });
  await start.focus();
  await page.keyboard.press("Enter");
  const input = page.getByLabel("Practice input");
  await input.focus();
  await page.keyboard.press("d");
  await page.keyboard.press("k");
  await expect(page.locator("#typing-live-status")).toContainText(
    "Correct — 아",
  );

  const retry = page.getByRole("button", { name: "Retry" });
  await retry.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "None yet",
  );
  await input.focus();
  await page.keyboard.press("d");
  await page.keyboard.press("k");
  const next = page.getByRole("button", { name: "Next" });
  await next.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("button", { name: "Korean — 2-set Hangul" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByRole("heading", {
      name: "Second Korean fixture lesson",
      level: 2,
    }),
  ).toBeVisible();

  const accessibility = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(accessibility.violations).toEqual([]);
});
