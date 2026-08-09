import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

type Theme = "system" | "light" | "dark";
type Screen = "Today" | "Study" | "Decks" | "Deck" | "Add" | "Settings";

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
  screen: "Today" | "Decks" | "Add" | "Settings",
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
    if (visualCase.name.startsWith("study-audio")) {
      await expect(page.getByRole("slider", { name: /Seek/ })).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
    }
    await expect(page).toHaveScreenshot(`${visualCase.name}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
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
    await page.keyboard.press("Enter");
    await expect(
      page.getByRole("heading", { name: "Review saved" }),
    ).toBeVisible();
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
