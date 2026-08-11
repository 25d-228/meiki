import { expect, test, type Locator, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

const minimumActionGapPixels = 8;
const geometryTolerancePixels = 0.5;

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

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

async function waitForDocumentInteractionUnlock(page: Page): Promise<void> {
  await expect
    .poll(() =>
      page.evaluate(() => getComputedStyle(document.body).pointerEvents),
    )
    .not.toBe("none");
}

async function expectActionGroupGap(
  group: Locator,
  minimumActions = 2,
): Promise<void> {
  await expect(group).toBeVisible();
  const measurement = await group.evaluate((element) => {
    const style = getComputedStyle(element);
    const actions = Array.from(
      element.querySelectorAll<HTMLElement>(
        'button, select, [role="button"], [role="checkbox"]',
      ),
    ).filter((action) => {
      const bounds = action.getBoundingClientRect();
      return bounds.width > 0 && bounds.height > 0;
    });
    return {
      actionCount: actions.length,
      columnGap: Number.parseFloat(style.columnGap),
      rowGap: Number.parseFloat(style.rowGap),
    };
  });

  expect(measurement.actionCount).toBeGreaterThanOrEqual(minimumActions);
  expect(measurement.columnGap).toBeGreaterThanOrEqual(minimumActionGapPixels);
  expect(measurement.rowGap).toBeGreaterThanOrEqual(minimumActionGapPixels);
}

async function expectWrappedActionGroup(group: Locator): Promise<void> {
  await expectActionGroupGap(group);
  const rows = await group
    .locator('button, select, [role="button"], [role="checkbox"]')
    .evaluateAll((actions) => {
      const visibleBounds = actions
        .map((action) => action.getBoundingClientRect())
        .filter((bounds) => bounds.width > 0 && bounds.height > 0)
        .sort((left, right) => left.top - right.top || left.left - right.left);
      const groupedRows: Array<{ top: number; bottom: number }> = [];
      for (const bounds of visibleBounds) {
        const row = groupedRows.find(
          (candidate) => Math.abs(candidate.top - bounds.top) <= 1,
        );
        if (row) {
          row.bottom = Math.max(row.bottom, bounds.bottom);
        } else {
          groupedRows.push({ top: bounds.top, bottom: bounds.bottom });
        }
      }
      return groupedRows;
    });

  expect(rows.length).toBeGreaterThan(1);
  for (let index = 1; index < rows.length; index += 1) {
    expect(rows[index].top - rows[index - 1].bottom).toBeGreaterThanOrEqual(
      minimumActionGapPixels - geometryTolerancePixels,
    );
  }
}

async function selectSourceText(page: Page): Promise<void> {
  const source = page.locator(".segment-text").last();
  await source.fill("Audio prompt");
  await source.evaluate((element) => {
    const input = element as HTMLTextAreaElement;
    input.focus();
    input.setSelectionRange(0, 5);
    input.dispatchEvent(new Event("select", { bubbles: true }));
  });
}

test("keeps independent actions separated across maintained screens", async ({
  page,
}) => {
  await page.goto("/");
  await expectActionGroupGap(page.locator(".today-actions"));

  await navigatePrimary(page, "Decks");
  const gridDeck = page.getByTestId("deck-travel-deck");
  await expectActionGroupGap(gridDeck.locator(".deck-navigation-actions"));
  await expectActionGroupGap(gridDeck.locator(".deck-card-actions"));

  await page
    .getByRole("group", { name: "Deck view" })
    .getByRole("button", { name: "List" })
    .click();
  const listDeck = page.getByTestId("deck-travel-deck");
  await expectActionGroupGap(listDeck.locator(".deck-navigation-actions"));
  await expectActionGroupGap(listDeck.locator(".deck-list-actions"));

  await listDeck.getByRole("button", { name: "Open" }).click();
  await expectActionGroupGap(page.locator(".deck-management-actions"));
  await expectActionGroupGap(
    page.getByTestId("card-card-ar").locator(".card-actions"),
  );

  await navigatePrimary(page, "Add");
  await expectActionGroupGap(page.locator(".screen-header .cluster"));
  await expectActionGroupGap(page.locator(".segment-order").first());
  await selectSourceText(page);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await page.getByRole("button", { name: "Local media" }).click();
  await expectActionGroupGap(page.locator(".media-actions"));
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("Card saved on this device.")).toBeVisible();

  await navigatePrimary(page, "Typing");
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  await page.getByRole("button", { name: "Start practice" }).click();
  await expectActionGroupGap(page.locator(".practice-actions"));
  await expect(
    page
      .locator(".conversion-sandbox")
      .getByRole("button", { name: "Reset sandbox" }),
  ).toBeVisible();

  await navigatePrimary(page, "Settings");
  await page
    .getByRole("group", { name: "Scheduling mode" })
    .getByRole("button", { name: "Expert" })
    .click();
  await expectActionGroupGap(page.locator(".scheduler-actions"));
});

test("keeps Study prompt, reveal, grading, audio, and saved actions separated", async ({
  page,
}) => {
  await page.goto("/?media=ready");
  await page.getByRole("button", { name: "Start study" }).click();
  await expectActionGroupGap(page.locator(".prompt-tools"));
  await expectActionGroupGap(page.locator(".audio-actions"));

  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await expectActionGroupGap(page.locator(".reveal-tools"));
  await expectActionGroupGap(page.locator(".grade-grid"), 4);

  await page.getByRole("button", { name: /^Good/ }).click();
  await expectActionGroupGap(page.locator(".next-actions"));
});

test("keeps import, bundle, and deck deletion dialog actions separated", async ({
  page,
}) => {
  await page.goto("/?bundleRemoval=installed&decks=batch");
  await navigatePrimary(page, "Decks");

  await page.getByRole("button", { name: "Import bundle" }).click();
  const importDialog = page.getByRole("dialog", { name: "Import bundle" });
  await expectActionGroupGap(
    importDialog.locator('[data-slot="dialog-footer"]'),
  );
  await importDialog.getByRole("button", { name: "Close" }).last().click();
  await expect(importDialog).toBeHidden();

  await page.getByRole("button", { name: "Bundle actions" }).click();
  const bundleActions = page.getByRole("dialog", { name: "Bundle actions" });
  await expectActionGroupGap(
    bundleActions.locator(".bundle-action-list > div").first(),
  );
  await bundleActions.getByRole("button", { name: /^Remove Japanese/ }).click();
  await expect(bundleActions).toBeHidden();
  const bundleRemoval = page.getByRole("alertdialog", {
    name: "Remove Japanese?",
  });
  await expectActionGroupGap(
    bundleRemoval.locator('[data-slot="alert-dialog-footer"]'),
  );
  await bundleRemoval.getByRole("button", { name: "Cancel" }).click();
  await expect(bundleRemoval).toBeHidden();
  await expect(bundleActions).toBeHidden();
  await waitForDocumentInteractionUnlock(page);

  const travelDeck = page.getByTestId("deck-travel-deck");
  await travelDeck
    .getByRole("button", { name: "Actions for Travel phrases" })
    .click();
  await page.getByRole("menuitem", { name: "Delete deck" }).click();
  const deckDeletion = page.getByRole("alertdialog", {
    name: "Delete “Travel phrases”?",
  });
  await expectActionGroupGap(
    deckDeletion.locator('[data-slot="alert-dialog-footer"]'),
    3,
  );
  await deckDeletion
    .getByRole("button", { name: "Move cards instead" })
    .click();
  const moveDialog = page.getByRole("dialog", { name: "Move cards instead" });
  await expectActionGroupGap(moveDialog.locator('[data-slot="dialog-footer"]'));
  await moveDialog.getByRole("button", { name: "Cancel" }).click();
  await expect(moveDialog).toBeHidden();

  await page.getByRole("checkbox", { name: "Select Travel phrases" }).click();
  await page.getByRole("checkbox", { name: "Select Listening drills" }).click();
  await page.getByRole("button", { name: "Delete selected" }).click();
  const batchDeletion = page.getByRole("alertdialog", {
    name: "Delete 2 selected decks?",
  });
  await expectActionGroupGap(
    batchDeletion.locator('[data-slot="alert-dialog-footer"]'),
  );
});

test("keeps wrapped narrow action groups separated without horizontal overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 720 });
  await page.goto("/?bundleRemoval=installed");
  await navigatePrimary(page, "Decks");
  await expectWrappedActionGroup(page.locator(".screen-actions"));

  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Open" })
    .click();
  await expectWrappedActionGroup(page.locator(".deck-management-actions"));

  await navigatePrimary(page, "Add");
  await expectWrappedActionGroup(page.locator(".screen-header .cluster"));
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});
