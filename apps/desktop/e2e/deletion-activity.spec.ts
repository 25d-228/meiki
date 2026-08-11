import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function navigatePrimary(
  page: Page,
  screen: "Today" | "Decks" | "Add" | "Typing" | "Settings",
): Promise<void> {
  const openNavigation = page.getByRole("button", { name: "Open navigation" });
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

async function openDecks(page: Page): Promise<void> {
  await navigatePrimary(page, "Decks");
}

async function startSingleDeckDeletion(page: Page): Promise<void> {
  await page
    .getByRole("button", { name: "Actions for Travel phrases" })
    .click();
  await page.getByRole("menuitem", { name: "Delete deck" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
    .getByRole("button", { name: "Delete deck" })
    .click();
}

async function startBatchDeletion(page: Page): Promise<void> {
  await page.getByRole("checkbox", { name: "Select Travel phrases" }).click();
  await page
    .getByRole("checkbox", { name: "Select Listening practice" })
    .click();
  await page.getByRole("button", { name: "Delete selected" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete 2 selected decks?" })
    .getByRole("button", { name: "Delete selected" })
    .click();
}

async function startBundleRemoval(page: Page): Promise<void> {
  await page.getByRole("button", { name: "Bundle actions" }).click();
  await page
    .getByRole("dialog", { name: "Bundle actions" })
    .getByRole("button", { name: "Remove Japanese" })
    .click();
  await page
    .getByRole("alertdialog", { name: "Remove Japanese?" })
    .getByRole("button", { name: "Remove bundle" })
    .click();
}

async function requestCount(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (requestedCommand) =>
      (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
        (request) => request.command === requestedCommand,
      ).length,
    command,
  );
}

test("single deletion continues after an outside click dismisses its details", async ({
  page,
}) => {
  await page.goto("/?deckDeletion=progress-visual");
  await openDecks(page);
  await startSingleDeckDeletion(page);

  const details = page.getByRole("dialog", {
    name: "Deleting “Travel phrases”",
  });
  await expect(details).toContainText("Removing cards");
  await page
    .locator('[data-slot="dialog-overlay"][data-state="open"]')
    .click({ position: { x: 5, y: 5 } });
  await expect(details).toBeHidden();
  await expect(page.getByTestId("deletion-activity")).toBeVisible();
  expect(await requestCount(page, "delete_deck")).toBe(1);

  await page.evaluate(() =>
    localStorage.setItem("meiki-e2e-finish-deck-deletion", "true"),
  );
  await expect(page.getByTestId("deletion-activity")).toContainText(
    "Deleted Travel phrases.",
  );
});

test("batch deletion continues after Escape and reopens the same progress", async ({
  page,
}) => {
  await page.goto("/?decks=batch&batchDeletion=progress");
  await openDecks(page);
  await startBatchDeletion(page);

  const details = page.getByRole("dialog", { name: "Deleting 2 decks" });
  await expect(details).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(details).toBeHidden();
  const activity = page.getByTestId("deletion-activity");
  await expect(activity).toBeVisible();
  await activity.getByRole("button", { name: "Open deletion details" }).click();
  await expect(details).toBeVisible();
  expect(await requestCount(page, "delete_decks")).toBe(1);
});

test("bundle removal continues after Close dismisses its details", async ({
  page,
}) => {
  await page.goto("/?bundleRemoval=installed");
  await openDecks(page);
  await startBundleRemoval(page);

  const details = page.getByRole("dialog", { name: "Removing Japanese" });
  await expect(details.getByRole("progressbar")).toHaveAttribute(
    "aria-valuemax",
    "9700",
  );
  await details.getByRole("button", { name: "Close" }).last().click();
  await expect(details).toBeHidden();
  await expect(page.getByTestId("deletion-activity")).toBeVisible();
  expect(await requestCount(page, "remove_bundle")).toBe(1);
  await expect(page.getByTestId("deletion-activity")).toContainText(
    "Removed Japanese with 6 decks.",
  );
});

test("bundle media cleanup failure remains a committed warning", async ({
  page,
}) => {
  await page.clock.install({ time: new Date("2026-08-11T00:00:00Z") });
  await page.addInitScript(() => {
    localStorage.setItem(
      "meiki-active-study-queue",
      JSON.stringify({
        version: 2,
        deckId: "deck:ja-JP:05",
        entries: [
          {
            card_id: "due-card",
            card_content_version: 0,
            schedule_version: 0,
          },
        ],
        position: 0,
        startedAtMs: 1_700_000_000_000,
        pendingReview: null,
      }),
    );
    sessionStorage.setItem(
      "meiki-active-study-session",
      "removed bundle session",
    );
    localStorage.setItem("meiki-today-deck", "deck:ja-JP:05");
  });
  await page.goto(
    "/?bundleRemoval=installed&bundleDeletion=postcommit-failure",
  );
  await openDecks(page);
  await page.clock.pauseAt(new Date("2026-08-11T00:01:00Z"));
  await startBundleRemoval(page);

  const card = page.getByTestId("deletion-activity");
  const details = page.getByRole("dialog", { name: "Bundle removed" });
  const warning =
    "Japanese was removed, but some unused audio could not be cleaned up.";
  await expect(card).toContainText(warning);
  await expect(details).toContainText(warning);
  await expect(details).not.toContainText("Your collection was left unchanged");
  await expect(page.getByTestId("deck-deck:ja-JP:05")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Bundle actions" }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(() => ({
      queue: localStorage.getItem("meiki-active-study-queue"),
      session: sessionStorage.getItem("meiki-active-study-session"),
      today: localStorage.getItem("meiki-today-deck"),
    })),
  ).toEqual({ queue: null, session: null, today: "__all_decks__" });

  await page.clock.runFor(200);
  await details.getByRole("button", { name: "Close" }).last().click();
  await page.clock.runFor(200);
  await card.getByRole("button", { name: "Open deletion details" }).click();
  await expect(details).toContainText(warning);

  await page.clock.fastForward(2_599);
  await expect(card).toBeVisible();
  await page.clock.fastForward(1);
  await expect(card).toBeHidden();
  await expect(details).toBeVisible();
});

test("running deletion remains visible through every primary screen", async ({
  page,
}) => {
  await page.goto("/?deckDeletion=progress-visual");
  await openDecks(page);
  await startSingleDeckDeletion(page);
  await page
    .getByRole("dialog", { name: "Deleting “Travel phrases”" })
    .getByRole("button", { name: "Close" })
    .last()
    .click();

  for (const screen of [
    "Today",
    "Decks",
    "Add",
    "Typing",
    "Settings",
  ] as const) {
    await navigatePrimary(page, screen);
    await expect(page.getByTestId("deletion-activity")).toContainText(
      "Deleting “Travel phrases”",
    );
  }
});

test("Today refreshes when deletion finishes while Today is open", async ({
  page,
}) => {
  await page.goto("/?deckDeletion=progress-visual");
  await navigatePrimary(page, "Today");
  await page.getByLabel("Deck").selectOption("travel-deck");
  await openDecks(page);
  await startSingleDeckDeletion(page);
  await page
    .getByRole("dialog", { name: "Deleting “Travel phrases”" })
    .getByRole("button", { name: "Close" })
    .last()
    .click();
  await navigatePrimary(page, "Today");
  await expect(page.getByLabel("Deck")).toHaveValue("travel-deck");

  await page.evaluate(() =>
    localStorage.setItem("meiki-e2e-finish-deck-deletion", "true"),
  );
  await expect(page.getByTestId("deletion-activity")).toContainText(
    "Deleted Travel phrases.",
  );
  await expect(page.getByLabel("Deck")).toHaveValue("__all_decks__");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("progress ignores older phases and lower values", async ({ page }) => {
  await page.goto("/?deckDeletion=progress-visual");
  await openDecks(page);
  await startSingleDeckDeletion(page);
  const activity = page.getByTestId("deletion-activity");
  await expect(activity).toContainText("1,240 / 2,999");

  await page.evaluate(() => {
    window.__MEIKI_TEST_DECK_DELETION_PROGRESS__?.({
      phase: "removing_cards",
      current: 3_000,
      total: 3_000,
    });
    window.__MEIKI_TEST_DECK_DELETION_PROGRESS__?.({
      phase: "cleaning_audio",
      current: 20,
      total: 2_999,
    });
  });
  await expect(activity).toContainText("Cleaning audio");
  await expect(activity).toContainText("1,240 / 2,999");
});

test("running card exposes indeterminate and determinate progress semantics", async ({
  page,
}) => {
  await page.goto("/?deckDeletion=progress-visual");
  await openDecks(page);
  await startSingleDeckDeletion(page);
  const activity = page.getByTestId("deletion-activity");
  const progressbar = activity.getByRole("progressbar");
  await expect(activity).toContainText("Preparing");
  await expect(progressbar).not.toHaveAttribute("aria-valuenow");
  await expect(activity).toContainText("Removing cards");
  await expect(progressbar).toHaveAttribute("aria-valuemax", "3000");
  await expect(activity).toContainText("Cleaning audio");
  await expect(progressbar).toHaveAttribute("aria-valuemax", "2999");
});

for (const terminalCase of [
  {
    name: "success",
    route: "/",
    dialog: "Deck deleted",
    message: "Deleted Travel phrases.",
  },
  {
    name: "warning",
    route: "/?deckDeletion=postcommit-failure",
    dialog: "Deck deleted",
    message: "Deck deleted, but some unused audio could not be cleaned up.",
  },
  {
    name: "failure",
    route: "/?deckDeletion=precommit-failure",
    dialog: "Deck was not deleted",
    message: "Could not delete the deck. Try again.",
  },
] as const) {
  test(`${terminalCase.name} card auto-hides after exactly three seconds without closing details`, async ({
    page,
  }) => {
    await page.clock.install({ time: new Date("2026-08-11T00:00:00Z") });
    await page.goto(terminalCase.route);
    await openDecks(page);
    await page.clock.pauseAt(new Date("2026-08-11T00:01:00Z"));
    await startSingleDeckDeletion(page);
    const card = page.getByTestId("deletion-activity");
    const details = page.getByRole("dialog", { name: terminalCase.dialog });
    await expect(details).toContainText(terminalCase.message);
    await expect(card).toBeVisible();

    await page.clock.runFor(200);
    await page.clock.fastForward(2_799);
    await expect(card).toBeVisible();
    await page.clock.fastForward(1);
    await expect(card).toBeHidden();
    await expect(details).toBeVisible();
  });
}

test("terminal card can be dismissed before its timer", async ({ page }) => {
  await page.clock.install({ time: new Date("2026-08-11T00:00:00Z") });
  await page.goto("/?deckDeletion=precommit-failure");
  await openDecks(page);
  await page.clock.pauseAt(new Date("2026-08-11T00:01:00Z"));
  await startSingleDeckDeletion(page);
  const card = page.getByTestId("deletion-activity");
  const details = page.getByRole("dialog", { name: "Deck was not deleted" });
  await expect(details).toBeVisible();
  await page.clock.runFor(200);
  await details.getByRole("button", { name: "Close" }).last().click();
  await page.clock.runFor(200);
  await card.getByRole("button", { name: "Dismiss deletion status" }).click();
  await expect(card).toBeHidden();
  await expect(details).toBeHidden();
});

test("running card never auto-hides", async ({ page }) => {
  await page.goto("/?deckDeletion=progress-visual");
  await openDecks(page);
  await startSingleDeckDeletion(page);
  const card = page.getByTestId("deletion-activity");
  await expect(card).toContainText("1,240 / 2,999");
  await page.clock.install();
  await page.clock.fastForward(3_001);
  await expect(card).toBeVisible();
  await expect(
    card.getByRole("button", { name: "Dismiss deletion status" }),
  ).toHaveCount(0);
});

test("one running deletion disables single, batch, and bundle starts", async ({
  page,
}) => {
  await page.goto(
    "/?bundleRemoval=installed&decks=batch&deckDeletion=progress-visual",
  );
  await openDecks(page);
  await startSingleDeckDeletion(page);
  await page
    .getByRole("dialog", { name: "Deleting “Travel phrases”" })
    .getByRole("button", { name: "Close" })
    .last()
    .click();

  await page
    .getByRole("checkbox", { name: "Select Listening practice" })
    .click();
  await expect(
    page.getByRole("button", { name: "Delete selected" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Bundle actions" }).click();
  await expect(
    page
      .getByRole("dialog", { name: "Bundle actions" })
      .getByRole("button", { name: "Remove Japanese" }),
  ).toBeDisabled();
  expect(await requestCount(page, "delete_deck")).toBe(1);
  expect(await requestCount(page, "delete_decks")).toBe(0);
  expect(await requestCount(page, "remove_bundle")).toBe(0);
});

test("bundle import and deletion cards coexist without overlap or state interference", async ({
  page,
}) => {
  await page.goto("/?bundleImport=activity&deckDeletion=progress-visual");
  await openDecks(page);
  await page.getByRole("button", { name: "Import bundle" }).click();
  await page
    .getByRole("dialog", { name: "Import bundle" })
    .getByRole("button", { name: "Add bundle" })
    .click();
  await page
    .getByRole("dialog", { name: "Import bundle" })
    .getByRole("button", { name: "Close" })
    .last()
    .click();
  await startSingleDeckDeletion(page);
  await page
    .getByRole("dialog", { name: "Deleting “Travel phrases”" })
    .getByRole("button", { name: "Close" })
    .last()
    .click();

  const importCard = page.getByTestId("bundle-import-activity");
  const deletionCard = page.getByTestId("deletion-activity");
  await expect(importCard).toBeVisible();
  await expect(deletionCard).toBeVisible();
  const importBounds = await importCard.boundingBox();
  const deletionBounds = await deletionCard.boundingBox();
  expect(
    importBounds &&
      deletionBounds &&
      deletionBounds.y + deletionBounds.height <= importBounds.y,
  ).toBe(true);
  expect(await requestCount(page, "import_bundle")).toBe(1);
  expect(await requestCount(page, "delete_deck")).toBe(1);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.waitForTimeout(200);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});
