import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function enableStudyKeyboard(
  page: Page,
  platform?: "windows" | "macos",
): Promise<void> {
  await page.addInitScript((savedPlatform) => {
    localStorage.setItem("meiki-study-visual-keyboard", "true");
    if (savedPlatform) {
      localStorage.setItem("meiki-typing-platform", savedPlatform);
    }
  }, platform);
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

async function dispatchKey(
  target: Locator,
  type: "keydown" | "keyup",
  code: string,
  key: string,
  options: { repeat?: boolean; isComposing?: boolean } = {},
): Promise<void> {
  await target.dispatchEvent(type, {
    bubbles: true,
    code,
    key,
    repeat: options.repeat ?? false,
    isComposing: options.isComposing ?? false,
  });
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

async function requestCount(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (name) =>
      (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
        (request) => request.command === name,
      ).length,
    command,
  );
}

async function expectPassiveStateReset(page: Page): Promise<void> {
  await expect(page.getByTestId("study-keyboard-physical-trail")).toHaveText(
    "None yet",
  );
  await expect(page.getByTestId("study-keyboard-composition")).toHaveText(
    "None",
  );
  await expect(page.getByTestId("study-keyboard-committed-output")).toHaveText(
    "None",
  );
  await expect(
    page.getByTestId("study-visual-keyboard").locator('[data-pressed="true"]'),
  ).toHaveCount(0);
}

test("Study visual keyboard defaults Off and persists from the keyboard-operated Settings toggle", async ({
  page,
}) => {
  await page.goto("/");
  await navigate(page, "Settings");
  const toggle = page.getByRole("switch", {
    name: "Show visual keyboard during Study",
  });

  await expect(toggle).not.toBeChecked();
  expect(
    await page.evaluate(() =>
      localStorage.getItem("meiki-study-visual-keyboard"),
    ),
  ).toBeNull();
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

test("Study renders no keyboard when the preference is disabled", async ({
  page,
}) => {
  await openStudy(page);
  await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(0);
});

test("the keyboard remains through checking and hides for reveal and saved states", async ({
  page,
}) => {
  await enableStudyKeyboard(page, "windows");
  await openStudy(page, "/?check=loading&grade=loading");
  const input = page.getByLabel("Your answer");
  await expect(page.getByTestId("study-visual-keyboard")).toBeVisible();
  await input.fill("行きます");
  await input.press("Enter");
  await expect(page.getByTestId("study-visual-keyboard")).toBeVisible();
  await expect(page.getByRole("button", { name: "Checking…" })).toBeVisible();

  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(page.getByRole("button", { name: /^Good/ })).toBeDisabled();
  await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
  await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(0);
});

test("loading, empty, and error states never show the keyboard", async ({
  page,
}) => {
  await enableStudyKeyboard(page, "windows");
  await page.goto("/?fixture=loading");
  await page.getByRole("button", { name: "Start study" }).click();
  await expect(page.getByText("Opening your local collection…")).toBeVisible();
  await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(0);

  await page.evaluate(() => {
    localStorage.removeItem("meiki-active-study-queue");
    sessionStorage.removeItem("meiki-active-study-session");
  });
  await page.goto("/?today=empty");
  await page.getByRole("button", { name: "Start study" }).click();
  await expect(
    page.getByRole("heading", { name: "Nothing is due" }),
  ).toBeVisible();
  await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(0);

  await page.evaluate(() => {
    localStorage.removeItem("meiki-active-study-queue");
    sessionStorage.removeItem("meiki-active-study-session");
  });
  await page.goto("/?fixture=error");
  await page.getByRole("button", { name: "Start study" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByTestId("study-visual-keyboard")).toHaveCount(0);
});

test("Korean tags select shifted and base jamo above the Latin legend", async ({
  page,
}) => {
  await enableStudyKeyboard(page, "windows");
  await openStudy(page, "/?studyLanguage=KO-kR");
  const key = page.getByTestId("typing-key-KeyQ");
  await expect(
    key.locator(".shifted-target-legend, .target-legend, .latin-legend"),
  ).toHaveText(["ㅃ", "ㅂ", "Q"]);
});

test("Japanese remains Latin-only", async ({ page }) => {
  await enableStudyKeyboard(page, "windows");
  await openStudy(page, "/?studyLanguage=JA-jp");
  const key = page.getByTestId("typing-key-KeyQ");
  await expect(
    key.locator(".target-legend, .shifted-target-legend"),
  ).toHaveCount(0);
  await expect(key.locator(".latin-legend")).toHaveText("Q");
});

for (const guidance of [
  {
    language: "fr-FR",
    platform: "windows",
    expected: "Use United States-International on Windows.",
  },
  {
    language: "fr",
    platform: "macos",
    expected: "Use the standard U.S. layout on macOS.",
  },
  {
    language: "es-MX",
    platform: "windows",
    expected: "Use United States-International on Windows.",
  },
  {
    language: "ES",
    platform: "macos",
    expected: "Use the standard U.S. layout on macOS.",
  },
] as const) {
  test(`${guidance.language} uses the saved ${guidance.platform} guidance`, async ({
    page,
  }) => {
    await enableStudyKeyboard(page, guidance.platform);
    await openStudy(
      page,
      `/?studyLanguage=${encodeURIComponent(guidance.language)}`,
    );
    await expect(page.getByTestId("study-keyboard-guidance")).toHaveText(
      guidance.expected,
    );
    await expect(
      page.getByTestId("study-visual-keyboard").locator(".target-legend"),
    ).toHaveCount(0);
  });
}

test("Linux without a saved reference uses plain Latin and non-prescriptive guidance", async ({
  page,
}) => {
  await setRuntimePlatform(page, "Linux x86_64");
  await enableStudyKeyboard(page);
  await openStudy(page, "/?studyLanguage=ko");
  await expect(page.getByTestId("study-keyboard-guidance")).toHaveText(
    "Configure the language input source through your desktop environment.",
  );
  await expect(
    page.getByTestId("study-visual-keyboard").locator(".target-legend"),
  ).toHaveCount(0);
  await expect(page.getByTestId("study-keyboard-guidance")).not.toContainText(
    /Ctrl|Alt\+|Super/,
  );
});

for (const fallback of ["missing", "%%", "zh-Hant"] as const) {
  test(`${fallback} language falls back safely to plain Latin`, async ({
    page,
  }) => {
    await enableStudyKeyboard(page, "windows");
    await openStudy(page, `/?studyLanguage=${encodeURIComponent(fallback)}`);
    await expect(
      page.getByTestId("study-visual-keyboard").locator(".target-legend"),
    ).toHaveCount(0);
    await expect(page.getByTestId("typing-key-KeyQ")).toContainText("Q");
  });
}

test("physical trail preserves ordinal codes, ignores repeat, and clears held keys on keyup and blur", async ({
  page,
}) => {
  await enableStudyKeyboard(page, "windows");
  await openStudy(page);
  const input = page.getByLabel("Your answer");
  const trail = page.getByTestId("study-keyboard-physical-trail");

  await dispatchKey(input, "keydown", "KeyA", "a");
  await dispatchKey(input, "keydown", "KeyA", "a", { repeat: true });
  await dispatchKey(input, "keyup", "KeyA", "a");
  await dispatchKey(input, "keydown", "KeyA", "a");
  await dispatchKey(input, "keyup", "KeyA", "a");
  await expect(trail).toHaveText("A → A");

  await dispatchKey(input, "keydown", "ShiftLeft", "Shift");
  await dispatchKey(input, "keydown", "AltRight", "AltGraph");
  await dispatchKey(input, "keydown", "AltLeft", "Alt");
  const shift = page.getByTestId("typing-key-ShiftLeft");
  const altGr = page.getByTestId("typing-key-AltRight");
  const option = page.getByTestId("typing-key-AltLeft");
  await expect(shift).toHaveAttribute("data-held", "true");
  await expect(altGr).toHaveAttribute("data-held", "true");
  await expect(option).toHaveAttribute("data-held", "true");

  await page.evaluate(() =>
    window.dispatchEvent(
      new KeyboardEvent("keyup", {
        code: "AltRight",
        key: "AltGraph",
      }),
    ),
  );
  await expect(altGr).toHaveAttribute("data-held", "false");
  await expect(shift).toHaveAttribute("data-held", "true");
  await expect(option).toHaveAttribute("data-held", "true");
  const trailBeforeBlur = await trail.textContent();
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await expect(shift).toHaveAttribute("data-held", "false");
  await expect(option).toHaveAttribute("data-held", "false");
  await expect(trail).toHaveText(trailBeforeBlur ?? "");
});

test("composition remains separate until commit and composing Enter never checks", async ({
  page,
}) => {
  await enableStudyKeyboard(page, "windows");
  await openStudy(page, "/?studyLanguage=ja");
  const input = page.getByLabel("Your answer");
  const composition = page.getByTestId("study-keyboard-composition");
  const committed = page.getByTestId("study-keyboard-committed-output");

  await dispatchComposition(input, "compositionstart", "に");
  await dispatchComposition(input, "compositionupdate", "にほ");
  await input.evaluate((element) => {
    element.value = "にほ";
    element.dispatchEvent(
      new InputEvent("input", {
        bubbles: true,
        data: "にほ",
        isComposing: true,
      }),
    );
  });
  await expect(composition).toHaveText("にほ");
  await expect(committed).toHaveText("None");
  await dispatchKey(input, "keydown", "Enter", "Enter", {
    isComposing: true,
  });
  await dispatchKey(input, "keyup", "Enter", "Enter", { isComposing: true });
  expect(await requestCount(page, "check_answer")).toBe(0);

  await input.evaluate((element) => {
    element.value = "日本";
    element.dispatchEvent(
      new InputEvent("input", {
        bubbles: true,
        data: "日本",
        isComposing: true,
      }),
    );
  });
  await dispatchComposition(input, "compositionend", "日本");
  await expect(composition).toHaveText("None");
  await expect(committed).toHaveText("日本");
  expect(await requestCount(page, "check_answer")).toBe(0);

  await input.press("Enter");
  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();
  expect(await requestCount(page, "check_answer")).toBe(1);
});

test("direct input updates committed output without submitting", async ({
  page,
}) => {
  await enableStudyKeyboard(page, "windows");
  await openStudy(page);
  await page.getByLabel("Your answer").fill("direct input");
  await expect(page.getByTestId("study-keyboard-committed-output")).toHaveText(
    "direct input",
  );
  expect(await requestCount(page, "check_answer")).toBe(0);
});

test("two same-language cards never share passive keyboard state", async ({
  page,
}) => {
  await enableStudyKeyboard(page, "windows");
  await openStudy(page, "/?reconcile=request");
  const input = page.getByLabel("Your answer");
  await input.fill("行きます");
  await dispatchKey(input, "keydown", "KeyA", "a");
  await dispatchKey(input, "keyup", "KeyA", "a");
  await expect(page.getByTestId("study-keyboard-physical-trail")).toHaveText(
    "A",
  );

  await input.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expectPassiveStateReset(page);
});

test("Retry and return from Edit reset passive state", async ({ page }) => {
  await enableStudyKeyboard(page, "windows");
  await openStudy(page, "/?failure=check&check=loading");
  const input = page.getByLabel("Your answer");
  await input.fill("行きます");
  await dispatchKey(input, "keydown", "KeyA", "a");
  await dispatchKey(input, "keyup", "KeyA", "a");
  await input.press("Enter");
  await expect(page.getByRole("alert")).toBeVisible();
  await page.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByTestId("study-visual-keyboard")).toBeVisible();
  await expectPassiveStateReset(page);
  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();

  await page.goto("/");
  await page.getByRole("button", { name: /^(Start|Resume) study$/ }).click();
  const nextInput = page.getByLabel("Your answer");
  await nextInput.fill("draft");
  await dispatchKey(nextInput, "keydown", "KeyD", "d");
  await dispatchKey(nextInput, "keyup", "KeyD", "d");
  await page.getByRole("button", { name: "Edit note" }).click();
  await page.getByRole("button", { name: "Return to study" }).click();
  await expectPassiveStateReset(page);
});

test("unmount and queue replacement reset passive state", async ({ page }) => {
  await enableStudyKeyboard(page, "windows");
  await openStudy(page);
  const input = page.getByLabel("Your answer");
  await input.fill("abandoned");
  await dispatchKey(input, "keydown", "KeyA", "a");
  await dispatchKey(input, "keyup", "KeyA", "a");

  await navigate(page, "Today");
  await page.getByRole("button", { name: "Resume study" }).click();
  await expectPassiveStateReset(page);

  await page.getByLabel("Your answer").fill("replace me");
  await dispatchKey(page.getByLabel("Your answer"), "keydown", "KeyR", "r");
  await dispatchKey(page.getByLabel("Your answer"), "keyup", "KeyR", "r");
  await navigate(page, "Today");
  await page.getByLabel("Deck").selectOption("travel-deck");
  await page.getByRole("button", { name: "Start study" }).click();
  await expectPassiveStateReset(page);
});

test("the passive surface exposes no answer-derived state and remains accessible without overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await enableStudyKeyboard(page, "windows");
  await openStudy(page, "/?fixture=korean");
  const surface = page.getByTestId("study-visual-keyboard");
  const hiddenAnswer = "읽어요";
  const hiddenPhysicalSequence = "D → K → S → S → U → D";

  await expect(surface.locator('[data-expected="true"]')).toHaveCount(0);
  await expect(surface.locator('[data-correct="true"]')).toHaveCount(0);
  await expect(surface.locator('[data-incorrect="true"]')).toHaveCount(0);
  await expect(surface.locator('[data-completed="true"]')).toHaveCount(0);
  await expect(surface).not.toContainText(hiddenAnswer);
  await expect(surface).not.toContainText(hiddenPhysicalSequence);
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
