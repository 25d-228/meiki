import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/?bundleImport=activity");
});

async function navigatePrimary(
  page: Page,
  screen: "Today" | "Decks" | "Add" | "Settings",
): Promise<void> {
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

async function startBundleImport(page: Page): Promise<void> {
  await navigatePrimary(page, "Decks");
  await page.getByRole("button", { name: "Import bundle" }).click();
  const dialog = page.getByRole("dialog", { name: "Import bundle" });
  await expect(dialog.getByText("Japanese", { exact: true })).toBeVisible();
  await dialog.getByRole("button", { name: "Add bundle" }).click();
  await expect(dialog).toContainText("Preparing decks");
  await expect(dialog.getByRole("progressbar")).not.toHaveAttribute(
    "aria-valuenow",
  );
  const activity = page.getByTestId("bundle-import-activity");
  await expect(activity).toContainText(
    /Adding Japanese\s+Adding cards\s+1,240 \/ 9,700/,
  );
  await expect(activity.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "1240",
  );
  await expect(dialog.getByRole("progressbar")).toHaveAttribute(
    "aria-valuemax",
    "9700",
  );
}

test("keeps one monotonic import visible and refreshes Decks after background success", async ({
  page,
}) => {
  await startBundleImport(page);
  const card = page.getByTestId("bundle-import-activity");
  const dialog = page.getByRole("dialog", { name: "Import bundle" });

  await page.mouse.click(4, 4);
  await expect(dialog).toBeHidden();
  await expect(card).toBeVisible();

  await navigatePrimary(page, "Today");
  await expect(card).toBeVisible();
  await card
    .getByRole("button", { name: /Open Japanese import details/ })
    .click();
  await expect(dialog).toContainText(/Adding cards\s+1,240 \/ 9,700/);
  await dialog.getByRole("button", { name: "Close" }).last().click();
  await expect(dialog).toBeHidden();

  await card
    .getByRole("button", { name: /Open Japanese import details/ })
    .click();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  for (const screen of ["Add", "Settings", "Decks"] as const) {
    await navigatePrimary(page, screen);
    await expect(card).toBeVisible();
  }
  await expect(
    page.getByRole("button", { name: "Import bundle" }),
  ).toBeDisabled();
  await expect
    .poll(() =>
      page.evaluate(() =>
        localStorage.getItem("meiki-e2e-bundle-regression-sent"),
      ),
    )
    .toBe("true");
  await expect(card).toContainText("1,240 / 9,700");

  await navigatePrimary(page, "Settings");
  await page.evaluate(() =>
    localStorage.setItem("meiki-e2e-finish-bundle-import", "success"),
  );
  await expect(card).toContainText("Added Japanese with 6 decks.");

  await navigatePrimary(page, "Decks");
  await expect(page.getByTestId("deck-deck:ja-JP:05")).toBeVisible();
  await card
    .getByRole("button", { name: "Dismiss bundle import status" })
    .click();
  await expect(card).toHaveCount(0);
  await expect(page.getByTestId("deck-deck:ja-JP:05")).toBeVisible();
});

test("shows a dismissible failure card that reopens the import error", async ({
  page,
}) => {
  await startBundleImport(page);
  const card = page.getByTestId("bundle-import-activity");
  await page
    .getByRole("dialog", { name: "Import bundle" })
    .getByRole("button", { name: "Close" })
    .last()
    .click();
  await navigatePrimary(page, "Add");
  await page.evaluate(() =>
    localStorage.setItem("meiki-e2e-finish-bundle-import", "failure"),
  );

  await expect(card).toContainText("Could not add Japanese.");
  await card
    .getByRole("button", { name: /Open Japanese import details/ })
    .click();
  const dialog = page.getByRole("dialog", { name: "Import bundle" });
  await expect(dialog.getByRole("alert")).toContainText(
    "The bundle archive could not be verified.",
  );
  await dialog.getByRole("button", { name: "Close" }).last().click();
  await card
    .getByRole("button", { name: "Dismiss bundle import status" })
    .click();
  await expect(card).toHaveCount(0);
});
