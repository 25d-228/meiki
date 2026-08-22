import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

const minimumActionGapPixels = 8;

type Theme = "system" | "light" | "dark";
type Screen =
  "Today" | "Study" | "Decks" | "Deck" | "Add" | "Typing" | "Settings";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function chooseTheme(page: Page, theme: Theme): Promise<void> {
  if (theme === "system") return;
  await page.getByRole("button", { name: "Theme" }).click();
  await page
    .getByRole("option", { name: new RegExp(`^${theme}$`, "i") })
    .click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
  await expect(page.getByRole("listbox")).toBeHidden();
}

async function navigatePrimary(
  page: Page,
  screen: "Today" | "Decks" | "Add" | "Typing" | "Settings",
): Promise<void> {
  const openNavigation = page.getByRole("button", {
    name: "Open navigation",
  });
  if (await openNavigation.isVisible()) await openNavigation.click();
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: screen, exact: true })
    .click();
  await expect(
    page.getByRole("heading", {
      name: screen === "Add" ? "Add / Edit card" : screen,
      level: 1,
    }),
  ).toBeVisible();
}

async function navigate(page: Page, screen: Screen): Promise<void> {
  if (screen === "Study") {
    await navigatePrimary(page, "Today");
    await page.getByRole("button", { name: "Start study" }).click();
  } else if (screen === "Deck") {
    await navigatePrimary(page, "Decks");
    await page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Open" })
      .click();
  } else {
    await navigatePrimary(page, screen);
  }
}

async function prepare(
  page: Page,
  options: {
    route: string;
    screen: Screen;
    theme: Theme;
    viewport: "desktop" | "medium" | "narrow";
  },
): Promise<void> {
  const viewports = {
    desktop: { width: 1440, height: 900 },
    medium: { width: 960, height: 720 },
    narrow: { width: 640, height: 720 },
  };
  await page.setViewportSize(viewports[options.viewport]);
  await page.goto(options.route);
  await chooseTheme(page, options.theme);
  if (options.screen !== "Today") await navigate(page, options.screen);
}

async function expectActionGap(
  group: import("@playwright/test").Locator,
): Promise<void> {
  const gaps = await group.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      column: Number.parseFloat(style.columnGap),
      row: Number.parseFloat(style.rowGap),
    };
  });
  expect(gaps.column).toBeGreaterThanOrEqual(minimumActionGapPixels);
  expect(gaps.row).toBeGreaterThanOrEqual(minimumActionGapPixels);
}

const visualCases = [
  {
    name: "today-desktop-light-normal",
    route: "/",
    screen: "Today",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "today-narrow-dark-empty",
    route: "/?today=empty",
    screen: "Today",
    theme: "dark",
    viewport: "narrow",
  },
  {
    name: "study-prompt-medium-light-latin",
    route: "/?fixture=ltr",
    screen: "Study",
    theme: "light",
    viewport: "medium",
  },
  {
    name: "study-prompt-narrow-dark-rtl",
    route: "/?fixture=rtl",
    screen: "Study",
    theme: "dark",
    viewport: "narrow",
  },
  {
    name: "study-prompt-desktop-dark-mixed",
    route: "/?fixture=mixed",
    screen: "Study",
    theme: "dark",
    viewport: "desktop",
  },
  {
    name: "study-audio-desktop-light",
    route: "/?media=ready",
    screen: "Study",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "study-audio-narrow-dark",
    route: "/?media=ready",
    screen: "Study",
    theme: "dark",
    viewport: "narrow",
  },
  {
    name: "editor-narrow-light-cjk-combining",
    route: "/?fixture=cjk",
    screen: "Add",
    theme: "light",
    viewport: "narrow",
  },
  {
    name: "editor-desktop-dark-ltr",
    route: "/?fixture=ltr",
    screen: "Add",
    theme: "dark",
    viewport: "desktop",
  },
  {
    name: "decks-medium-light-normal",
    route: "/",
    screen: "Decks",
    theme: "light",
    viewport: "medium",
  },
  {
    name: "deck-management-narrow-dark-normal",
    route: "/",
    screen: "Deck",
    theme: "dark",
    viewport: "narrow",
  },
  {
    name: "settings-desktop-light-normal",
    route: "/",
    screen: "Settings",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "settings-narrow-dark-normal",
    route: "/",
    screen: "Settings",
    theme: "dark",
    viewport: "narrow",
  },
] as const;

for (const visualCase of visualCases) {
  test(`visual regression: ${visualCase.name}`, async ({ page }) => {
    await prepare(page, visualCase);
    if (visualCase.screen === "Add") {
      await page
        .getByLabel("Sentence text segment 1")
        .fill(
          visualCase.name.includes("cjk")
            ? "Cafe\u0301 · 日曜日は図書館に行きます"
            : "Le dimanche, je vais à la bibliothèque",
        );
    }
    if (visualCase.screen === "Study") {
      await expect(page.locator("#study-prompt")).toBeVisible();
    }
    if (visualCase.screen === "Today") {
      await expect(
        page.getByRole("heading", { name: "Review statistics" }),
      ).toBeVisible();
      await expect(
        page.getByRole("img", { name: /Daily review activity from/ }),
      ).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
    }
    if (visualCase.name.startsWith("study-audio")) {
      await expect(page.getByRole("slider", { name: /Seek/ })).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
    }
    if (visualCase.name === "deck-management-narrow-dark-normal") {
      await expectActionGap(page.locator(".deck-management-actions"));
      await expectActionGap(
        page.getByTestId("card-card-ar").locator(".card-actions"),
      );
    }
    await expect(page).toHaveScreenshot(`${visualCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const statisticsCase of [
  {
    name: "today-statistics-dashboard-desktop-light",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "today-statistics-dashboard-narrow-dark",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${statisticsCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: "/?today=normal",
      screen: "Today",
      theme: statisticsCase.theme,
      viewport: statisticsCase.viewport,
    });
    await expect(
      page.getByRole("heading", { name: "Review statistics" }),
    ).toBeVisible();
    await expect(
      page.getByRole("img", { name: /Daily review activity from/ }),
    ).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${statisticsCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      fullPage: true,
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const vimCase of [
  {
    name: "vim-decks-normal-desktop-light",
    screen: "Decks",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "vim-study-insert-narrow-dark",
    screen: "Study",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${vimCase.name}`, async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem("meiki-vim-keybindings", "true");
    });
    await prepare(page, {
      route: "/",
      screen: vimCase.screen,
      theme: vimCase.theme,
      viewport: vimCase.viewport,
    });
    if (vimCase.screen === "Decks") {
      await page.locator("#main-content").focus();
      await page.keyboard.press("j");
      await expect(page.getByTestId("deck-travel-deck")).toBeFocused();
      await expect(page.getByLabel("Vim mode NORMAL")).toBeVisible();
    } else {
      await expect(page.getByLabel("Your answer")).toBeFocused();
      await expect(page.getByLabel("Vim mode INSERT")).toBeVisible();
    }
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${vimCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const studyKeyboardCase of [
  {
    name: "study-keyboard-korean-desktop-light",
    route: "/?fixture=korean",
    theme: "light",
    viewport: "desktop",
    platform: "windows",
  },
  {
    name: "study-keyboard-spanish-narrow-dark",
    route: "/?studyLanguage=es",
    theme: "dark",
    viewport: "narrow",
    platform: "macos",
  },
] as const) {
  test(`visual regression: ${studyKeyboardCase.name}`, async ({ page }) => {
    await page.addInitScript((platform) => {
      localStorage.setItem("meiki-study-front-answer", "true");
      localStorage.setItem("meiki-study-visual-keyboard", "true");
      localStorage.setItem("meiki-typing-platform", platform);
    }, studyKeyboardCase.platform);
    await prepare(page, {
      route: studyKeyboardCase.route,
      screen: "Study",
      theme: studyKeyboardCase.theme,
      viewport: studyKeyboardCase.viewport,
    });
    await expect(page.getByTestId("study-front-answer")).toBeVisible();
    await expect(page.getByTestId("study-visual-keyboard")).toBeVisible();
    await expect(
      page.getByText("Visual keyboard", { exact: true }),
    ).toHaveCount(0);
    await expect(page.getByTestId("study-keyboard-guidance")).toHaveCount(0);
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${studyKeyboardCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      fullPage: true,
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const typingLandingCase of [
  {
    name: "typing-tracks-desktop-light",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "typing-tracks-narrow-dark",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${typingLandingCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: "/",
      screen: "Typing",
      theme: typingLandingCase.theme,
      viewport: typingLandingCase.viewport,
    });
    await expect(
      page.getByRole("group", { name: "Language" }).getByRole("button"),
    ).toHaveCount(8);
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${typingLandingCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      fullPage: true,
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const typingCase of [
  {
    name: "typing-practice-desktop-light",
    theme: "light",
    viewport: "desktop",
    language: "Korean — 2-set Hangul",
  },
  {
    name: "typing-practice-narrow-dark",
    theme: "dark",
    viewport: "narrow",
    language: "French — Dead-key accents",
  },
  {
    name: "typing-french-desktop-light",
    theme: "light",
    viewport: "desktop",
    language: "French — Dead-key accents",
  },
  {
    name: "typing-spanish-desktop-light",
    theme: "light",
    viewport: "desktop",
    language: "Spanish — Dead-key accents",
  },
  {
    name: "typing-spanish-narrow-dark",
    theme: "dark",
    viewport: "narrow",
    language: "Spanish — Dead-key accents",
  },
  {
    name: "typing-korean-narrow-dark",
    theme: "dark",
    viewport: "narrow",
    language: "Korean — 2-set Hangul",
  },
  {
    name: "typing-japanese-desktop-light",
    theme: "light",
    viewport: "desktop",
    language: "Japanese — Romaji input",
  },
  {
    name: "typing-japanese-narrow-dark",
    theme: "dark",
    viewport: "narrow",
    language: "Japanese — Romaji input",
  },
  {
    name: "typing-german-desktop-light",
    theme: "light",
    viewport: "desktop",
    language: "German — Umlauts and ß",
  },
  {
    name: "typing-portuguese-narrow-dark",
    theme: "dark",
    viewport: "narrow",
    language: "Portuguese — Dead-key accents",
  },
  {
    name: "typing-russian-desktop-light",
    theme: "light",
    viewport: "desktop",
    language: "Russian — ЙЦУКЕН",
  },
  {
    name: "typing-russian-narrow-dark",
    theme: "dark",
    viewport: "narrow",
    language: "Russian — ЙЦУКЕН",
  },
  {
    name: "typing-chinese-desktop-light",
    theme: "light",
    viewport: "desktop",
    language: "Chinese — Pinyin input",
  },
  {
    name: "typing-chinese-narrow-dark",
    theme: "dark",
    viewport: "narrow",
    language: "Chinese — Pinyin input",
  },
] as const) {
  test(`visual regression: ${typingCase.name}`, async ({ page }) => {
    await page.addInitScript(() => {
      Object.defineProperty(navigator, "platform", {
        configurable: true,
        get: () => "MacIntel",
      });
      Object.defineProperty(navigator, "userAgentData", {
        configurable: true,
        get: () => ({ platform: "macOS" }),
      });
    });
    await prepare(page, {
      route: "/",
      screen: "Typing",
      theme: typingCase.theme,
      viewport: typingCase.viewport,
    });
    await page.getByRole("button", { name: typingCase.language }).click();
    await page.getByRole("button", { name: "Start practice" }).click();
    const keyboard = page.locator(".practice").getByTestId("typing-keyboard");
    await expect(keyboard).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${typingCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      fullPage: true,
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const deckViewCase of [
  {
    name: "deck-view-grid-desktop-light",
    route: "/",
    view: "Grid",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "deck-view-list-desktop-light",
    route: "/",
    view: "List",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "deck-view-grid-narrow-dark",
    route: "/?decks=long-name",
    view: "Grid",
    theme: "dark",
    viewport: "narrow",
  },
  {
    name: "deck-view-list-narrow-dark",
    route: "/?decks=long-name",
    view: "List",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${deckViewCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: deckViewCase.route,
      screen: "Decks",
      theme: deckViewCase.theme,
      viewport: deckViewCase.viewport,
    });
    if (deckViewCase.view === "List") {
      await page
        .getByRole("group", { name: "Deck view" })
        .getByRole("button", { name: "List" })
        .click();
    }
    await expect(
      page.getByTestId(
        deckViewCase.view === "Grid" ? "deck-grid" : "deck-list",
      ),
    ).toBeVisible();
    if (deckViewCase.name === "deck-view-grid-desktop-light") {
      await expectActionGap(
        page
          .getByTestId("deck-travel-deck")
          .locator(".deck-navigation-actions"),
      );
    }
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${deckViewCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const selectionCase of [
  {
    name: "deck-selection-grid-desktop-light",
    view: "Grid",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "deck-selection-list-narrow-dark-selected",
    view: "List",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${selectionCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: "/?decks=batch",
      screen: "Decks",
      theme: selectionCase.theme,
      viewport: selectionCase.viewport,
    });
    if (selectionCase.view === "List") {
      await page
        .getByRole("group", { name: "Deck view" })
        .getByRole("button", { name: "List" })
        .click();
    }
    await page.getByRole("checkbox", { name: "Select Travel phrases" }).click();
    await page
      .getByRole("checkbox", {
        name: "Select Japanese 00 — Kana, sound, and Japanese input",
      })
      .click();
    await expect(page.getByTestId("deck-selection-count")).toContainText(
      "2 decks selected",
    );
    await page.locator("#main-content").focus();
    await page.evaluate(() => {
      document.documentElement.scrollTop = 0;
      document.body.scrollTop = 0;
    });
    const heading = page.getByRole("heading", { name: "Decks", level: 1 });
    await heading.scrollIntoViewIfNeeded();
    await expect(heading).toBeInViewport();
    expect(
      await heading.evaluate((element) => element.getBoundingClientRect().top),
    ).toBeGreaterThanOrEqual(48);
    await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
    const areaBounds = await page
      .getByTestId("deck-selection-area")
      .boundingBox();
    const deckBounds = await page.getByTestId("deck-travel-deck").boundingBox();
    if (!areaBounds || !deckBounds) {
      throw new Error("Deck selection geometry is unavailable");
    }
    const start =
      selectionCase.view === "Grid"
        ? {
            x: deckBounds.x + deckBounds.width + 8,
            y: deckBounds.y + deckBounds.height - 12,
          }
        : { x: deckBounds.x + 8, y: deckBounds.y - 4 };
    const end =
      selectionCase.view === "Grid"
        ? {
            x: deckBounds.x + deckBounds.width - 48,
            y: deckBounds.y + 40,
          }
        : {
            x: deckBounds.x + deckBounds.width - 24,
            y: deckBounds.y + 40,
          };
    await page.keyboard.down("Shift");
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.keyboard.up("Shift");
    await page.mouse.move(end.x, end.y, { steps: 4 });
    await expect(page.getByTestId("deck-selection-rectangle")).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${selectionCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
    await page.mouse.up();
  });
}

for (const deckActionsCase of [
  {
    name: "deck-actions-menu-desktop-light",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "deck-actions-menu-narrow-dark",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${deckActionsCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: "/",
      screen: "Decks",
      theme: deckActionsCase.theme,
      viewport: deckActionsCase.viewport,
    });
    await page
      .getByRole("button", { name: "Actions for Travel phrases" })
      .click();
    await expect(
      page.getByRole("menuitem", { name: "Reset progress" }),
    ).toBeVisible();
    await expect(
      page.getByRole("menuitem", { name: "Delete deck" }),
    ).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${deckActionsCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const resetCase of [
  {
    name: "deck-reset-confirmation-desktop-light",
    state: "confirmation",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "deck-reset-confirmation-narrow-dark",
    state: "confirmation",
    theme: "dark",
    viewport: "narrow",
  },
  {
    name: "deck-reset-success-desktop-light",
    state: "success",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "deck-reset-success-narrow-dark",
    state: "success",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${resetCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: "/",
      screen: "Decks",
      theme: resetCase.theme,
      viewport: resetCase.viewport,
    });
    await page
      .getByRole("button", { name: "Actions for Travel phrases" })
      .click();
    await page.getByRole("menuitem", { name: "Reset progress" }).click();
    const confirmation = page.getByRole("alertdialog", {
      name: "Reset progress for “Travel phrases”?",
    });
    await expect(confirmation).toBeVisible();
    if (resetCase.state === "success") {
      await confirmation
        .getByRole("button", { name: "Reset progress" })
        .click();
      await expect(
        page.getByText("Reset progress for Travel phrases."),
      ).toBeVisible();
    }
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${resetCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const activityCase of [
  {
    name: "bundle-import-activity-desktop-light",
    route: "/?bundleImport=activity",
    state: "determinate",
    screen: "Settings",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "bundle-import-activity-narrow-dark",
    route: "/?bundleImport=activity",
    state: "determinate",
    screen: "Add",
    theme: "dark",
    viewport: "narrow",
  },
  {
    name: "bundle-import-activity-preparing-desktop-light",
    route: "/?bundleImport=activity&bundleProgress=preparing",
    state: "preparing",
    screen: "Settings",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "bundle-import-activity-terminal-desktop-light",
    route: "/?bundleImport=activity",
    state: "terminal",
    screen: "Today",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "bundle-import-activity-long-language-narrow-dark",
    route: "/?bundleImport=activity&bundleLanguage=long",
    state: "long-language",
    screen: "Add",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${activityCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: activityCase.route,
      screen: "Decks",
      theme: activityCase.theme,
      viewport: activityCase.viewport,
    });
    await page.getByRole("button", { name: "Import bundle" }).click();
    const dialog = page.getByRole("dialog", { name: "Import bundle" });
    await dialog.getByRole("button", { name: "Add bundle" }).click();
    const activity = page.getByTestId("bundle-import-activity");
    if (activityCase.state === "preparing") {
      await expect(activity).toContainText("Preparing decks");
    } else {
      await expect(activity).toContainText("1,240 / 9,700");
    }
    if (activityCase.state === "terminal") {
      await page.evaluate(() =>
        localStorage.setItem("meiki-e2e-finish-bundle-import", "success"),
      );
      await expect(
        activity.getByRole("button", {
          name: "Dismiss bundle import status",
        }),
      ).toBeVisible();
      await expect(dialog).toBeHidden();
    } else {
      await dialog.getByRole("button", { name: "Close" }).last().click();
    }
    await navigatePrimary(page, activityCase.screen);
    const activityBounds = await activity.boundingBox();
    const primaryActionBounds = await page
      .locator("[data-primary-action]")
      .boundingBox();
    expect(activityBounds?.width).toBeLessThanOrEqual(256);
    expect(
      activityBounds &&
        primaryActionBounds &&
        activityBounds.x < primaryActionBounds.x + primaryActionBounds.width &&
        activityBounds.x + activityBounds.width > primaryActionBounds.x &&
        activityBounds.y < primaryActionBounds.y + primaryActionBounds.height &&
        activityBounds.y + activityBounds.height > primaryActionBounds.y,
    ).toBeFalsy();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    const snapshotName =
      process.platform === "linux" &&
      (activityCase.state === "terminal" ||
        activityCase.state === "long-language")
        ? `${activityCase.name}-linux.png`
        : `${activityCase.name}.png`;
    await expect(activity).toHaveScreenshot(snapshotName, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.03,
    });
  });
}

for (const deletionCase of [
  {
    name: "deck-deletion-progress-desktop-light",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "deck-deletion-progress-narrow-dark",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${deletionCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: "/?deckDeletion=progress-visual",
      screen: "Decks",
      theme: deletionCase.theme,
      viewport: deletionCase.viewport,
    });
    await page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Open" })
      .click();
    await page.getByRole("button", { name: "Delete deck" }).click();
    await page
      .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
      .getByRole("button", { name: "Delete deck" })
      .click();
    const dialog = page.getByRole("dialog", {
      name: "Deleting “Travel phrases”",
    });
    await expect(dialog).toContainText("1,240 / 2,999");
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${deletionCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
    await page.evaluate(() =>
      localStorage.setItem("meiki-e2e-finish-deck-deletion", "true"),
    );
  });
}

for (const deletionActivityCase of [
  {
    name: "deletion-activity-desktop-light",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "deletion-activity-narrow-dark",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${deletionActivityCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: "/?deckDeletion=progress-visual",
      screen: "Decks",
      theme: deletionActivityCase.theme,
      viewport: deletionActivityCase.viewport,
    });
    await page
      .getByRole("button", { name: "Actions for Travel phrases" })
      .click();
    await page.getByRole("menuitem", { name: "Delete deck" }).click();
    await page
      .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
      .getByRole("button", { name: "Delete deck" })
      .click();
    const dialog = page.getByRole("dialog", {
      name: "Deleting “Travel phrases”",
    });
    const activity = page.getByTestId("deletion-activity");
    await expect(activity).toContainText("1,240 / 2,999");
    await dialog.getByRole("button", { name: "Close" }).last().click();
    await navigatePrimary(page, "Settings");
    await expect(activity).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(page).toHaveScreenshot(`${deletionActivityCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
    await page.evaluate(() =>
      localStorage.setItem("meiki-e2e-finish-deck-deletion", "true"),
    );
  });
}

for (const revealCase of [
  {
    name: "study-reveal-desktop-light-cjk",
    fixture: "cjk",
    answer: "行きます",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "study-reveal-medium-dark-mixed",
    fixture: "mixed",
    answer: "三時",
    theme: "dark",
    viewport: "medium",
  },
  {
    name: "study-reveal-desktop-light-korean",
    fixture: "korean",
    answer: "읽어요",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "study-reveal-narrow-dark-long-answer",
    fixture: "longanswer",
    answer:
      "this intentionally long highlighted answer includes 한국어, 日本語, and العربية while wrapping naturally across several lines",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${revealCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: `/?fixture=${revealCase.fixture}`,
      screen: "Study",
      theme: revealCase.theme,
      viewport: revealCase.viewport,
    });
    await page.getByLabel("Your answer").fill(revealCase.answer);
    await page.getByLabel("Your answer").press("Enter");
    await expect(page.getByText("Expected answer")).toBeVisible();
    await expect(page).toHaveScreenshot(`${revealCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

for (const feedbackCase of [
  {
    name: "study-feedback-desktop-light",
    theme: "light",
    viewport: "desktop",
  },
  {
    name: "study-feedback-narrow-dark",
    theme: "dark",
    viewport: "narrow",
  },
] as const) {
  test(`visual regression: ${feedbackCase.name}`, async ({ page }) => {
    await prepare(page, {
      route: "/?answer=extra-prefix",
      screen: "Study",
      theme: feedbackCase.theme,
      viewport: feedbackCase.viewport,
    });
    await page.getByLabel("Your answer").fill("大学生");
    await page.getByLabel("Your answer").press("Enter");
    await expect(
      page.getByTestId("answer-difference").locator("del"),
    ).toBeVisible();
    await expect(
      page.getByTestId("answer-difference").locator("ins"),
    ).toHaveCount(0);
    await expect(page).toHaveScreenshot(`${feedbackCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

test("visual regression: study-loading-medium-light", async ({ page }) => {
  await prepare(page, {
    route: "/?fixture=loading",
    screen: "Study",
    theme: "light",
    viewport: "medium",
  });
  await expect(page.getByText("Opening your local collection…")).toBeVisible();
  await expect(page).toHaveScreenshot("study-loading-medium-light.png", {
    animations: "disabled",
    caret: "hide",
    maxDiffPixelRatio: 0.12,
  });
});

test("visual regression: study-loading-medium-dark", async ({ page }) => {
  await prepare(page, {
    route: "/?fixture=loading",
    screen: "Study",
    theme: "dark",
    viewport: "medium",
  });
  await expect(page.getByText("Opening your local collection…")).toBeVisible();
  await expect(page).toHaveScreenshot("study-loading-medium-dark.png", {
    animations: "disabled",
    caret: "hide",
    maxDiffPixelRatio: 0.12,
  });
});

for (const theme of ["light", "dark"] as const) {
  test(`visual regression: study-empty-medium-${theme}`, async ({ page }) => {
    await prepare(page, {
      route: "/?today=empty&collection=empty",
      screen: "Study",
      theme,
      viewport: "medium",
    });
    await expect(
      page.getByRole("heading", { name: "Your collection is empty" }),
    ).toBeVisible();
    await expect(page).toHaveScreenshot(`study-empty-medium-${theme}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

test("visual regression: study-error-narrow-dark", async ({ page }) => {
  await prepare(page, {
    route: "/?fixture=error",
    screen: "Study",
    theme: "dark",
    viewport: "narrow",
  });
  await expect(
    page.getByRole("alert").getByText("The collection could not be opened"),
  ).toBeVisible();
  await expect(page).toHaveScreenshot("study-error-narrow-dark.png", {
    animations: "disabled",
    caret: "hide",
    maxDiffPixelRatio: 0.12,
  });
});

test("visual regression: study-error-narrow-light", async ({ page }) => {
  await prepare(page, {
    route: "/?fixture=error",
    screen: "Study",
    theme: "light",
    viewport: "narrow",
  });
  await expect(
    page.getByRole("alert").getByText("The collection could not be opened"),
  ).toBeVisible();
  await expect(page).toHaveScreenshot("study-error-narrow-light.png", {
    animations: "disabled",
    caret: "hide",
    maxDiffPixelRatio: 0.12,
  });
});

for (const theme of ["light", "dark"] as const) {
  test(`visual regression: study-stale-medium-${theme}`, async ({ page }) => {
    await prepare(page, {
      route: "/?fixture=stale",
      screen: "Study",
      theme,
      viewport: "medium",
    });
    await expect(page.getByRole("alert")).toContainText(
      "The study queue changed while it was loading.",
    );
    await expect(page).toHaveScreenshot(`study-stale-medium-${theme}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

test("visual regression: card-trash-success-medium-dark", async ({ page }) => {
  await prepare(page, {
    route: "/",
    screen: "Deck",
    theme: "dark",
    viewport: "medium",
  });
  await page
    .getByTestId("card-card-ar")
    .getByRole("button", { name: "Move to Trash" })
    .click();
  await expect(page.getByTestId("app-shell").getByRole("status")).toContainText(
    "Moved the card to Trash.",
  );
  await expect(page).toHaveScreenshot("card-trash-success-medium-dark.png", {
    animations: "disabled",
    caret: "hide",
    maxDiffPixelRatio: 0.12,
  });
});

test("visual regression: card-trash-success-medium-light", async ({ page }) => {
  await prepare(page, {
    route: "/",
    screen: "Deck",
    theme: "light",
    viewport: "medium",
  });
  await page
    .getByTestId("card-card-ar")
    .getByRole("button", { name: "Move to Trash" })
    .click();
  await expect(page.getByTestId("app-shell").getByRole("status")).toContainText(
    "Moved the card to Trash.",
  );
  await expect(page).toHaveScreenshot("card-trash-success-medium-light.png", {
    animations: "disabled",
    caret: "hide",
    maxDiffPixelRatio: 0.12,
  });
});

for (const theme of ["light", "dark"] as const) {
  test(`visual regression: study-success-medium-${theme}`, async ({ page }) => {
    await prepare(page, {
      route: "/",
      screen: "Study",
      theme,
      viewport: "medium",
    });
    const answer = page.getByLabel("Your answer");
    await answer.fill("行きます");
    await answer.press("Enter");
    await expect(page.getByText("Expected answer")).toBeVisible();
    await page.getByRole("button", { name: /^Good/ }).click();
    await expect(page.getByText(/Second card ·/)).toBeVisible();
    await expect(page.getByTestId("review-saved-status")).toBeVisible();
    await expect(page).toHaveScreenshot(`study-success-medium-${theme}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}

test("dialogs trap focus and restore it to their launch control", async ({
  page,
}) => {
  await page.goto("/");
  await navigate(page, "Add");
  const source = page.getByLabel("Sentence text segment 1");
  await source.fill("Keyboard");
  await source.evaluate((element) => {
    const textarea = element as HTMLTextAreaElement;
    textarea.focus();
    textarea.setSelectionRange(0, 8);
    textarea.dispatchEvent(new Event("select", { bubbles: true }));
  });
  await page.getByRole("button", { name: "Make cloze" }).click();
  const previewButton = page.getByRole("button", { name: "Preview" });
  await previewButton.click();
  const dialog = page.getByRole("dialog", { name: "Card preview" });
  await expect(dialog).toBeVisible();

  for (let index = 0; index < 8; index += 1) {
    await page.keyboard.press("Tab");
    await expect
      .poll(() =>
        page.evaluate(() =>
          Boolean(document.activeElement?.closest("[role='dialog']")),
        ),
      )
      .toBe(true);
  }

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(previewButton).toBeFocused();
});

test("alert dialogs trap focus and restore it to their launch control", async ({
  page,
}) => {
  await page.goto("/");
  await navigate(page, "Add");
  await page.getByLabel("Sentence text segment 1").fill("Unsaved card");
  const decksButton = page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Decks", exact: true });
  await decksButton.click();
  const dialog = page.getByRole("alertdialog", {
    name: "Discard unsaved changes?",
  });
  await expect(dialog).toBeVisible();

  for (let index = 0; index < 8; index += 1) {
    await page.keyboard.press("Tab");
    await expect
      .poll(() =>
        page.evaluate(() =>
          Boolean(document.activeElement?.closest("[role='alertdialog']")),
        ),
      )
      .toBe(true);
  }

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(decksButton).toBeFocused();
});

test("screens reflow at a 200% zoom-equivalent CSS viewport", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 720 });
  await page.goto("/");
  for (const screen of [
    "Today",
    "Decks",
    "Deck",
    "Study",
    "Add",
    "Settings",
  ] as const) {
    if (screen !== "Today") await navigate(page, screen);
    await expect(
      page.getByRole("heading", {
        name:
          screen === "Add"
            ? "Add / Edit card"
            : screen === "Deck"
              ? "Travel phrases"
              : screen,
        level: 1,
      }),
    ).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth > window.innerWidth,
      ),
    ).toBe(false);
  }
});
