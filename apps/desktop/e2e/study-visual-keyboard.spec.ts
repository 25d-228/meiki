import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

type StudyPreferences = {
  answer?: boolean;
  keyboard?: boolean;
  platform?: "windows" | "macos";
};

async function setStudyPreferences(
  page: Page,
  preferences: StudyPreferences,
): Promise<void> {
  await page.addInitScript((saved) => {
    if (saved.answer !== undefined) {
      localStorage.setItem("meiki-study-front-answer", String(saved.answer));
    }
    if (saved.keyboard !== undefined) {
      localStorage.setItem(
        "meiki-study-visual-keyboard",
        String(saved.keyboard),
      );
    }
    if (saved.platform) {
      localStorage.setItem("meiki-typing-platform", saved.platform);
    }
  }, preferences);
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

async function navigate(page: Page, screen: string): Promise<void> {
  const openNavigation = page.getByRole("button", { name: "Open navigation" });
  if (await openNavigation.isVisible()) await openNavigation.click();
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: screen, exact: true })
    .click();
}

async function openStudy(page: Page, route = "/"): Promise<void> {
  await page.goto(route);
  await page.getByRole("button", { name: /^(Start|Resume) study$/ }).click();
  await expect(page.getByLabel("Your answer")).toBeVisible();
}

async function clearSavedStudy(page: Page): Promise<void> {
  await page.evaluate(() => {
    localStorage.removeItem("meiki-active-study-queue");
    sessionStorage.removeItem("meiki-active-study-session");
  });
}

async function dispatchKey(
  target: Locator,
  type: "keydown" | "keyup",
  code: string,
  key: string,
  repeat = false,
): Promise<void> {
  await target.dispatchEvent(type, {
    bubbles: true,
    code,
    key,
    repeat,
  });
}

async function requestCount(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (name) =>
      (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
        (request) => request.command === name,
      ).length,
    command,
  );
}

async function lastRequest(page: Page, command: string) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

test("front answer defaults Off and persists from the keyboard-operated Settings switch", async ({
  page,
}) => {
  await page.goto("/");
  await navigate(page, "Settings");
  const toggle = page.getByRole("switch", {
    name: "Show answer on card front",
  });

  await expect(toggle).not.toBeChecked();
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-study-front-answer")),
  ).toBeNull();
  await toggle.focus();
  await page.keyboard.press("Space");
  await expect(toggle).toBeChecked();
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-study-front-answer")),
  ).toBe("true");

  await page.reload();
  await navigate(page, "Settings");
  await expect(
    page.getByRole("switch", { name: "Show answer on card front" }),
  ).toBeChecked();
});

test("Study visual keyboard defaults Off and persists from its keyboard-operated Settings switch", async ({
  page,
}) => {
  await page.goto("/");
  await navigate(page, "Settings");
  const toggle = page.getByRole("switch", {
    name: "Show visual keyboard during Study",
  });

  await expect(toggle).not.toBeChecked();
  await toggle.focus();
  await page.keyboard.press("Space");
  await expect(toggle).toBeChecked();
  expect(
    await page.evaluate(() =>
      localStorage.getItem("meiki-study-visual-keyboard"),
    ),
  ).toBe("true");

  await page.reload();
  await navigate(page, "Settings");
  await expect(
    page.getByRole("switch", {
      name: "Show visual keyboard during Study",
    }),
  ).toBeChecked();
});

test("front answer stays hidden when its preference is Off", async ({
  page,
}) => {
  await openStudy(page);
  await expect(page.getByTestId("study-front-answer")).toHaveCount(0);
  await expect(page.getByText("Expected answer", { exact: true })).toHaveCount(
    0,
  );
  await expect(page.getByLabel("Your answer")).toHaveValue("");
  expect(await requestCount(page, "check_answer")).toBe(0);
});

test("front answer uses the current card language and direction without changing the response", async ({
  page,
}) => {
  await setStudyPreferences(page, { answer: true });
  await openStudy(page, "/?fixture=rtl&check=loading");
  const answer = page.getByTestId("study-front-answer");
  const input = page.getByLabel("Your answer");

  await expect(answer).toContainText("Expected answer");
  await expect(answer.locator("strong")).toHaveText("کتاب");
  await expect(answer.locator("strong")).toHaveAttribute("lang", "fa");
  await expect(answer.locator("strong")).toHaveAttribute("dir", "rtl");
  await expect(input).toHaveValue("");
  expect(await requestCount(page, "check_answer")).toBe(0);

  await input.fill("پاسخ من");
  await input.press("Enter");
  await expect(page.getByRole("button", { name: "Checking…" })).toBeVisible();
  await expect(answer.locator("strong")).toHaveText("کتاب");
  await expect(input).toHaveValue("پاسخ من");
  const checkArgs = (await lastRequest(page, "check_answer"))?.args as
    { request: Record<string, unknown> } | undefined;
  expect(checkArgs?.request).toMatchObject({ raw_response: "پاسخ من" });
  expect(checkArgs?.request).not.toHaveProperty("expected_answer");

  await expect(answer).toHaveCount(0);
  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();
});

test("the next card replaces the previous front answer", async ({ page }) => {
  await setStudyPreferences(page, { answer: true });
  await openStudy(page, "/?reconcile=request");
  const input = page.getByLabel("Your answer");

  await expect(
    page.getByTestId("study-front-answer").locator("strong"),
  ).toHaveText("行きます");
  await input.fill("行きます");
  await input.press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
  await page.keyboard.press("Enter");

  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(
    page.getByTestId("study-front-answer").locator("strong"),
  ).toHaveText("行きます · second");
  await expect(page.getByLabel("Your answer")).toHaveValue("");
});

for (const preferences of [
  { answer: false, keyboard: false },
  { answer: true, keyboard: false },
  { answer: false, keyboard: true },
  { answer: true, keyboard: true },
] as const) {
  test(`front answer ${preferences.answer ? "On" : "Off"} and keyboard ${preferences.keyboard ? "On" : "Off"} remain independent`, async ({
    page,
  }) => {
    await setStudyPreferences(page, { ...preferences, platform: "windows" });
    await openStudy(page);
    await expect(page.getByTestId("study-front-answer")).toHaveCount(
      preferences.answer ? 1 : 0,
    );
    await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(
      preferences.keyboard ? 1 : 0,
    );
  });
}

test("the keyboard remains through checking and hides for reveal and saved states", async ({
  page,
}) => {
  await setStudyPreferences(page, { keyboard: true, platform: "windows" });
  await openStudy(page, "/?check=loading&grade=loading");
  const input = page.getByLabel("Your answer");
  await expect(page.getByTestId("study-visual-keyboard")).toBeVisible();
  await input.fill("行きます");
  await input.press("Enter");
  await expect(page.getByTestId("study-visual-keyboard")).toBeVisible();
  await expect(page.getByRole("button", { name: "Checking…" })).toBeVisible();

  await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(page.getByRole("button", { name: /^Good/ })).toBeDisabled();
  await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
});

test("the enabled Study keyboard renders only the passive keyboard layout", async ({
  page,
}) => {
  await setStudyPreferences(page, { keyboard: true, platform: "windows" });
  await openStudy(page);
  const surface = page.getByTestId("study-visual-keyboard");

  await expect(surface.getByTestId("typing-keyboard")).toBeVisible();
  await expect(page.getByText("Visual keyboard", { exact: true })).toHaveCount(
    0,
  );
  await expect(page.getByTestId("study-keyboard-guidance")).toHaveCount(0);
  await expect(
    page.getByText("Physical keys typed", { exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByText("Current composition", { exact: true }),
  ).toHaveCount(0);
  await expect(page.getByText("Committed output", { exact: true })).toHaveCount(
    0,
  );
  expect(
    await surface.evaluate((element) => {
      const style = getComputedStyle(element);
      return {
        background: style.backgroundColor,
        border: style.borderTopWidth,
        padding: style.paddingTop,
      };
    }),
  ).toEqual({
    background: "rgba(0, 0, 0, 0)",
    border: "0px",
    padding: "0px",
  });
});

test("Korean retains shifted and base jamo while Japanese remains Latin-only", async ({
  page,
}) => {
  await setStudyPreferences(page, { keyboard: true, platform: "windows" });
  await openStudy(page, "/?studyLanguage=KO-kR");
  const koreanKey = page.getByTestId("typing-key-KeyQ");
  await expect(
    koreanKey.locator(".shifted-target-legend, .target-legend, .latin-legend"),
  ).toHaveText(["ㅃ", "ㅂ", "Q"]);

  await clearSavedStudy(page);
  await openStudy(page, "/?studyLanguage=JA-jp");
  const japaneseKey = page.getByTestId("typing-key-KeyQ");
  await expect(
    japaneseKey.locator(".target-legend, .shifted-target-legend"),
  ).toHaveCount(0);
  await expect(japaneseKey.locator(".latin-legend")).toHaveText("Q");
});

test("Linux and unsupported language tags keep the Study keyboard Latin-only", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Linux x86_64");
  await setStudyPreferences(page, { keyboard: true });
  for (const [index, language] of [
    "ko",
    "missing",
    "%%",
    "zh-Hant",
  ].entries()) {
    if (index > 0) await clearSavedStudy(page);
    await openStudy(page, `/?studyLanguage=${encodeURIComponent(language)}`);
    const surface = page.getByTestId("study-visual-keyboard");
    await expect(
      surface.locator(".target-legend, .shifted-target-legend"),
    ).toHaveCount(0);
    await expect(page.getByTestId("typing-key-KeyQ")).toContainText("Q");
  }
});

test("pressed and held keys update without duplicate repeat state and clear on keyup and blur", async ({
  page,
}) => {
  await setStudyPreferences(page, { keyboard: true, platform: "windows" });
  await openStudy(page);
  const input = page.getByLabel("Your answer");
  const keyA = page.getByTestId("typing-key-KeyA");
  const shift = page.getByTestId("typing-key-ShiftLeft");
  const altGr = page.getByTestId("typing-key-AltRight");
  const option = page.getByTestId("typing-key-AltLeft");

  await dispatchKey(input, "keydown", "KeyA", "a");
  await dispatchKey(input, "keydown", "KeyA", "a", true);
  await expect(keyA).toHaveAttribute("data-pressed", "true");
  await dispatchKey(input, "keyup", "KeyA", "a");
  await expect(keyA).toHaveAttribute("data-pressed", "false");

  await dispatchKey(input, "keydown", "ShiftLeft", "Shift");
  await dispatchKey(input, "keydown", "AltRight", "AltGraph");
  await dispatchKey(input, "keydown", "AltLeft", "Alt");
  await expect(shift).toHaveAttribute("data-held", "true");
  await expect(altGr).toHaveAttribute("data-held", "true");
  await expect(option).toHaveAttribute("data-held", "true");

  await page.evaluate(() =>
    window.dispatchEvent(
      new KeyboardEvent("keyup", { code: "AltRight", key: "AltGraph" }),
    ),
  );
  await expect(altGr).toHaveAttribute("data-held", "false");
  await expect(shift).toHaveAttribute("data-held", "true");
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await expect(shift).toHaveAttribute("data-held", "false");
  await expect(option).toHaveAttribute("data-held", "false");
});

test("pressed state from one card cannot appear on the next card", async ({
  page,
}) => {
  await setStudyPreferences(page, { keyboard: true, platform: "windows" });
  await openStudy(page, "/?reconcile=request");
  const input = page.getByLabel("Your answer");
  const keyA = page.getByTestId("typing-key-KeyA");
  await dispatchKey(input, "keydown", "KeyA", "a");
  await expect(keyA).toHaveAttribute("data-pressed", "true");

  await input.fill("行きます");
  await input.press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
  await page.keyboard.press("Enter");

  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByTestId("typing-key-KeyA")).toHaveAttribute(
    "data-pressed",
    "false",
  );
});

test("keyboard-only Study remains answer-safe, accessible, and free of horizontal overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await setStudyPreferences(page, {
    answer: true,
    keyboard: true,
    platform: "windows",
  });
  await openStudy(page, "/?fixture=korean");
  const surface = page.getByTestId("study-visual-keyboard");

  await expect(page.getByTestId("study-front-answer")).toContainText("읽어요");
  await expect(surface.locator('[data-expected="true"]')).toHaveCount(0);
  await expect(surface.locator('[data-correct="true"]')).toHaveCount(0);
  await expect(surface.locator('[data-incorrect="true"]')).toHaveCount(0);
  await expect(surface.locator('[data-completed="true"]')).toHaveCount(0);
  await expect(surface).not.toContainText("읽어요");
  await expect(surface).not.toContainText("D → K → S → S → U → D");
  await expect(surface).not.toContainText(
    /Expected physical sequence|answer length/i,
  );
  await expect(surface.locator("[data-answer-length]")).toHaveCount(0);
  await expect(surface.getByTestId("typing-keyboard")).toHaveAttribute(
    "aria-hidden",
    "true",
  );
  expect(
    await surface
      .locator('[data-testid^="typing-key-"]')
      .evaluateAll((keys) =>
        keys.every((key) => (key as HTMLElement).tabIndex === -1),
      ),
  ).toBe(true);

  const input = page.getByLabel("Your answer");
  await input.focus();
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("button", { name: "Check answer" }),
  ).toBeFocused();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(results.violations).toEqual([]);
});
