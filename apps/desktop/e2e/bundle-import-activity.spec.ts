import { expect, test, type Locator, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/?bundleImport=activity");
});

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

async function startBundleImport(
  page: Page,
  expectedLanguage = "Japanese",
): Promise<void> {
  await navigatePrimary(page, "Decks");
  await page.getByRole("button", { name: "Import bundle" }).click();
  const dialog = page.getByRole("dialog", { name: "Import bundle" });
  await expect(
    dialog.getByText(expectedLanguage, { exact: true }),
  ).toBeVisible();
  await dialog.getByRole("button", { name: "Add bundle" }).click();
  await expect(dialog).toContainText("Preparing decks");
  await expect(dialog.getByRole("progressbar")).not.toHaveAttribute(
    "aria-valuenow",
  );
  const activity = page.getByTestId("bundle-import-activity");
  await expect(activity).toContainText(`Adding ${expectedLanguage}`);
  await expect(activity).toContainText(/Adding cards\s+1,240 \/ 9,700/);
  await expect(activity.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "1240",
  );
  await expect(dialog.getByRole("progressbar")).toHaveAttribute(
    "aria-valuemax",
    "9700",
  );
}

type Bounds = NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;

async function expectMainActivityFillsWidth(activity: Locator): Promise<{
  activity: Bounds;
  main: Bounds;
}> {
  const main = activity.locator('button[aria-label^="Open "]').first();
  const content = main.locator(":scope > span");
  const [activityBounds, mainBounds, contentBounds] = await Promise.all([
    activity.boundingBox(),
    main.boundingBox(),
    content.boundingBox(),
  ]);
  expect(activityBounds).not.toBeNull();
  expect(mainBounds).not.toBeNull();
  expect(contentBounds).not.toBeNull();
  if (!activityBounds || !mainBounds || !contentBounds) {
    throw new Error("The compact import activity must have measurable bounds.");
  }

  const leftInset = contentBounds.x - mainBounds.x;
  const rightInset =
    mainBounds.x + mainBounds.width - contentBounds.x - contentBounds.width;
  expect(leftInset).toBeGreaterThanOrEqual(15);
  expect(leftInset).toBeLessThanOrEqual(17);
  expect(rightInset).toBeGreaterThanOrEqual(15);
  expect(rightInset).toBeLessThanOrEqual(17);

  for (const child of [
    content.locator(":scope > strong"),
    content.locator(":scope > span").first(),
    content.getByRole("progressbar"),
  ]) {
    if ((await child.count()) === 0) continue;
    const childBounds = await child.boundingBox();
    expect(childBounds).not.toBeNull();
    if (!childBounds) continue;
    expect(Math.abs(childBounds.x - contentBounds.x)).toBeLessThanOrEqual(1);
    expect(
      Math.abs(childBounds.width - contentBounds.width),
    ).toBeLessThanOrEqual(1);
  }

  return { activity: activityBounds, main: mainBounds };
}

test("fills the compact activity width while Preparing decks is indeterminate", async ({
  page,
}) => {
  await page.goto("/?bundleImport=activity&bundleProgress=preparing");
  await navigatePrimary(page, "Decks");
  await page.getByRole("button", { name: "Import bundle" }).click();
  const dialog = page.getByRole("dialog", { name: "Import bundle" });
  await dialog.getByRole("button", { name: "Add bundle" }).click();
  const activity = page.getByTestId("bundle-import-activity");
  await expect(activity).toContainText(/Adding Japanese\s+Preparing decks/);
  await expect(activity.getByRole("progressbar")).not.toHaveAttribute(
    "aria-valuenow",
  );
  await dialog.getByRole("button", { name: "Close" }).last().click();

  await expectMainActivityFillsWidth(activity);
  await activity
    .getByRole("button", { name: "Open Japanese import details" })
    .click();
  await expect(dialog).toBeVisible();
  await page.evaluate(() =>
    localStorage.setItem("meiki-e2e-finish-bundle-import", "success"),
  );
});

test("wraps a long language name without narrowing determinate progress", async ({
  page,
}) => {
  const language = "An exceptionally long language display name for wrapping";
  await page.setViewportSize({ width: 640, height: 720 });
  await page.goto("/?bundleImport=activity&bundleLanguage=long");
  await startBundleImport(page, language);
  const dialog = page.getByRole("dialog", { name: "Import bundle" });
  await dialog.getByRole("button", { name: "Close" }).last().click();
  const activity = page.getByTestId("bundle-import-activity");

  await expectMainActivityFillsWidth(activity);
  const title = activity.getByText(`Adding ${language}`, { exact: true });
  const wraps = await title.evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    const lineHeight = Number.parseFloat(getComputedStyle(element).lineHeight);
    return {
      wrapped: bounds.height > lineHeight * 1.5,
      contained: element.scrollWidth <= element.clientWidth,
    };
  });
  expect(wraps).toEqual({ wrapped: true, contained: true });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});

test("keeps one monotonic import visible and refreshes Decks after background success", async ({
  page,
}) => {
  await startBundleImport(page);
  const card = page.getByTestId("bundle-import-activity");
  const dialog = page.getByRole("dialog", { name: "Import bundle" });
  await expectMainActivityFillsWidth(card);

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
  const terminalBounds = await expectMainActivityFillsWidth(card);
  const dismissBounds = await card
    .getByRole("button", { name: "Dismiss bundle import status" })
    .boundingBox();
  expect(dismissBounds).not.toBeNull();
  if (dismissBounds) {
    const cardRight = terminalBounds.activity.x + terminalBounds.activity.width;
    const mainRight = terminalBounds.main.x + terminalBounds.main.width;
    const reservedWidth = cardRight - mainRight;
    const dismissFootprintWidth = dismissBounds.width + 8;
    expect(Math.abs(reservedWidth - dismissFootprintWidth)).toBeLessThanOrEqual(
      2,
    );
    expect(mainRight).toBeLessThanOrEqual(dismissBounds.x);
  }

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
