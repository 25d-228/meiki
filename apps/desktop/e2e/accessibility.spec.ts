import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

type Theme = "light" | "dark";
type Screen = "Today" | "Decks" | "Add" | "Typing" | "Settings";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
});

async function chooseTheme(page: Page, theme: Theme): Promise<void> {
  await page.getByRole("button", { name: "Theme" }).click();
  await page
    .getByRole("option", { name: new RegExp(`^${theme}$`, "i") })
    .click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
}

async function navigate(page: Page, screen: Screen): Promise<void> {
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

async function openStudyScenario(
  page: Page,
  route: string,
  theme: Theme,
): Promise<void> {
  await page.goto(route);
  await page.evaluate(() => {
    localStorage.removeItem("meiki-active-study-queue");
    localStorage.removeItem("meiki-active-study-session");
  });
  await page.reload();
  await chooseTheme(page, theme);
  await page.getByRole("button", { name: "Start study" }).click();
  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
}

async function expectNoAccessibilityViolations(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(
    result.violations,
    result.violations
      .map(
        (violation) =>
          `${violation.id}: ${violation.nodes
            .map((node) => node.target.join(" "))
            .join(", ")}`,
      )
      .join("\n"),
  ).toEqual([]);
}

for (const theme of ["light", "dark"] as const) {
  for (const screen of [
    "Today",
    "Decks",
    "Add",
    "Typing",
    "Settings",
  ] as const) {
    test(`${screen} in ${theme} has no automated WCAG A/AA violations`, async ({
      page,
    }) => {
      await chooseTheme(page, theme);
      await navigate(page, screen);
      await expectNoAccessibilityViolations(page);
    });
  }

  test(`transient Study and deck management in ${theme} have no automated WCAG A/AA violations`, async ({
    page,
  }) => {
    await chooseTheme(page, theme);
    await page.getByRole("button", { name: "Start study" }).click();
    await expectNoAccessibilityViolations(page);
    await navigate(page, "Decks");
    await page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Open" })
      .click();
    await expectNoAccessibilityViolations(page);
  });
}

test("deck deletion progress exposes an accessible determinate state", async ({
  page,
}) => {
  await page.goto("/?deckDeletion=progress-visual");
  await chooseTheme(page, "dark");
  await navigate(page, "Decks");
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
  await expect(dialog.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "1240",
  );
  await expectNoAccessibilityViolations(page);
  await page.evaluate(() =>
    localStorage.setItem("meiki-e2e-finish-deck-deletion", "true"),
  );
});

test("deck reset and deletion actions are keyboard accessible", async ({
  page,
}) => {
  await chooseTheme(page, "dark");
  await navigate(page, "Decks");
  const actions = page.getByRole("button", {
    name: "Actions for Travel phrases",
  });
  await actions.focus();
  await page.keyboard.press("Enter");
  const resetAction = page.getByRole("menuitem", { name: "Reset progress" });
  await expect(resetAction).toBeFocused();
  await expectNoAccessibilityViolations(page);

  await page.keyboard.press("Enter");
  const resetConfirmation = page.getByRole("alertdialog", {
    name: "Reset progress for “Travel phrases”?",
  });
  await expect(resetConfirmation).toBeVisible();
  await expectNoAccessibilityViolations(page);
  await resetConfirmation.getByRole("button", { name: "Cancel" }).click();
  await expect(actions).toBeFocused();

  await page.keyboard.press("Enter");
  await expect(resetAction).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const deleteAction = page.getByRole("menuitem", { name: "Delete deck" });
  await expect(deleteAction).toBeFocused();
  await page.keyboard.press("Enter");
  const confirmation = page.getByRole("alertdialog", {
    name: "Delete “Travel phrases”?",
  });
  await expect(confirmation).toBeVisible();
  await confirmation.getByRole("button", { name: "Cancel" }).click();
  await expect(actions).toBeFocused();
});

test("deck view controls expose keyboard-operable selected state", async ({
  page,
}) => {
  await navigate(page, "Decks");
  const viewControl = page.getByRole("group", { name: "Deck view" });
  const grid = viewControl.getByRole("button", { name: "Grid" });
  const list = viewControl.getByRole("button", { name: "List" });
  await expect(grid).toHaveAttribute("aria-pressed", "true");
  await list.focus();
  await page.keyboard.press("Space");
  await expect(list).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("deck-list")).toBeVisible();
  await viewControl.evaluate(async (control) => {
    await Promise.all(
      control
        .getAnimations({ subtree: true })
        .map((animation) => animation.finished),
    );
  });
  await expectNoAccessibilityViolations(page);

  await grid.focus();
  await page.keyboard.press("Enter");
  await expect(grid).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("deck-grid")).toBeVisible();
});

test("deck selection and one batch confirmation are keyboard accessible", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await navigate(page, "Decks");
  const travel = page.getByRole("checkbox", {
    name: "Select Travel phrases",
  });
  await travel.focus();
  await page.keyboard.press("Space");
  await expect(travel).toHaveAttribute("aria-checked", "true");
  await expect(page.getByTestId("deck-selection-count")).toContainText(
    "1 deck selected",
  );
  await page.evaluate(async () => {
    await Promise.allSettled(
      document.getAnimations().map((animation) => animation.finished),
    );
  });
  await expectNoAccessibilityViolations(page);

  const list = page
    .getByRole("group", { name: "Deck view" })
    .getByRole("button", { name: "List" });
  await list.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("checkbox", { name: "Select Travel phrases" }),
  ).toHaveAttribute("aria-checked", "true");
  const deleteSelected = page.getByRole("button", {
    name: "Delete selected",
  });
  await deleteSelected.focus();
  await page.keyboard.press("Enter");
  const confirmation = page.getByRole("alertdialog", {
    name: "Delete 1 selected deck?",
  });
  await expect(confirmation).toBeVisible();
  await page.evaluate(async () => {
    await Promise.allSettled(
      document.getAnimations().map((animation) => animation.finished),
    );
  });
  await expectNoAccessibilityViolations(page);
  const confirm = confirmation.getByRole("button", {
    name: "Delete selected",
  });
  await confirm.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByTestId("deletion-activity").getByText("Deleted 1 deck."),
  ).toBeVisible();
});

for (const theme of ["light", "dark"] as const) {
  test(`loading, empty, error, and stale study states pass axe in ${theme}`, async ({
    page,
  }) => {
    await openStudyScenario(page, "/?fixture=loading", theme);
    await expect(
      page.getByText("Opening your local collection…"),
    ).toBeVisible();
    await expectNoAccessibilityViolations(page);

    await openStudyScenario(page, "/?today=empty&collection=empty", theme);
    await expect(
      page.getByRole("heading", { name: "Your collection is empty" }),
    ).toBeVisible();
    await expectNoAccessibilityViolations(page);

    await openStudyScenario(page, "/?fixture=error", theme);
    await expect(
      page.getByRole("alert").getByText("The collection could not be opened"),
    ).toBeVisible();
    await expectNoAccessibilityViolations(page);

    await openStudyScenario(page, "/?fixture=stale", theme);
    await expect(page.getByRole("alert")).toContainText(
      "The study queue changed while it was loading.",
    );
    await expectNoAccessibilityViolations(page);
  });

  test(`recoverable card trash and study success states pass axe in ${theme}`, async ({
    page,
  }) => {
    await chooseTheme(page, theme);
    await navigate(page, "Decks");
    await page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Open" })
      .click();
    await page
      .getByTestId("card-card-ar")
      .getByRole("button", { name: "Move to Trash" })
      .click();
    await expect(
      page.getByTestId("app-shell").getByRole("status"),
    ).toContainText("Moved the card to Trash.");
    await expectNoAccessibilityViolations(page);

    await openStudyScenario(page, "/", theme);
    const answer = page.getByLabel("Your answer");
    await answer.fill("行きます");
    await answer.press("Enter");
    await expect(
      page.getByText("Expected answer", { exact: true }),
    ).toBeVisible();
    await page.keyboard.press("Enter");
    await expect(page.getByText(/Second card ·/)).toBeVisible();
    await expect(page.getByTestId("review-saved-status")).toBeVisible();
    await expectNoAccessibilityViolations(page);
  });
}

test("skip navigation, focus transfer, and live study states are exposed", async ({
  page,
}) => {
  await page.keyboard.press("Tab");
  const skipLink = page.getByRole("link", { name: "Skip to content" });
  await expect(skipLink).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("#main-content")).toBeFocused();

  await page.getByRole("button", { name: "Decks", exact: true }).click();
  await expect(page.locator("#main-content")).toBeFocused();
  await page.getByRole("button", { name: "Today", exact: true }).click();
  await page.getByRole("button", { name: "Start study" }).click();
  const answer = page.getByLabel("Your answer");
  await expect(answer).toBeFocused();
  await answer.fill("行きます");
  await answer.press("Enter");
  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByTestId("review-saved-status")).toBeVisible();
});

test("RTL learning content does not reverse application controls", async ({
  page,
}) => {
  await page.goto("/?fixture=rtl");
  await page.getByRole("button", { name: "Start study" }).click();
  await expect(page.locator("#study-prompt")).toHaveAttribute("dir", "rtl");
  await expect(page.getByTestId("app-shell")).toHaveAttribute("dir", "ltr");
  await expectNoAccessibilityViolations(page);
});
