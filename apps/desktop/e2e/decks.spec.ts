import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
});

async function openDecks(page: import("@playwright/test").Page): Promise<void> {
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Decks", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Decks", level: 1 }),
  ).toBeVisible();
}

async function lastRequest(
  page: import("@playwright/test").Page,
  command: string,
) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

test("includes suspended cards in Total and presents the populated default deck as Unsorted", async ({
  page,
}) => {
  await openDecks(page);

  const unsorted = page.getByTestId("deck-default-deck");
  await expect(unsorted.getByText("Unsorted", { exact: true })).toBeVisible();
  await expect(unsorted.locator("dl")).toContainText(
    /Total\s*3\s*Due\s*1\s*New\s*1/,
  );
});

test("hides the empty internal default deck", async ({ page }) => {
  await page.goto("/?decks=empty-default");
  await openDecks(page);

  await expect(page.getByTestId("deck-default-deck")).toHaveCount(0);
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
});

test("keeps Unsorted visible when active cards exist but hides rename and delete", async ({
  page,
}) => {
  await openDecks(page);
  await page
    .getByTestId("deck-default-deck")
    .getByRole("button", { name: "Open" })
    .click();

  await expect(
    page.getByRole("heading", { name: "Unsorted", level: 1 }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Rename deck" })).toHaveCount(
    0,
  );
  await expect(page.getByRole("button", { name: "Delete deck" })).toHaveCount(
    0,
  );
  await expect(
    page.getByRole("button", { name: "Daily time", exact: true }),
  ).toHaveCount(0);
});

test("creates a deck from its name only", async ({ page }) => {
  await page.goto("/?decks=lifecycle");
  await openDecks(page);
  await page.getByRole("button", { name: "New deck" }).click();
  const dialog = page.getByRole("dialog", { name: "New deck" });
  await expect(dialog.getByRole("textbox")).toHaveCount(1);
  await dialog.getByLabel("Name").fill(" Listening ");
  await dialog.getByRole("button", { name: "Create deck" }).click();

  await expect(page.getByText("Created deck “Listening”.")).toBeVisible();
  await expect(page.getByTestId("deck-listening-deck")).toBeVisible();
  expect((await lastRequest(page, "create_deck"))?.args).toMatchObject({
    request: { name: " Listening " },
  });
});

test("starts and resumes a study queue restricted to one deck", async ({
  page,
}) => {
  await openDecks(page);
  const travelDeck = page.getByTestId("deck-travel-deck");
  await travelDeck.getByRole("button", { name: "Study" }).click();
  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
  expect((await lastRequest(page, "prepare_study"))?.args).toMatchObject({
    request: { deck_id: "travel-deck" },
  });

  await openDecks(page);
  await expect(
    page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Resume" }),
  ).toBeVisible();
  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Resume" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
});

test("manages deck identity and daily time without Settings deck controls", async ({
  page,
}) => {
  await page.goto("/?decks=lifecycle");
  await openDecks(page);
  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Open" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Travel phrases", level: 1 }),
  ).toBeVisible();
  expect((await lastRequest(page, "get_deck_cards"))?.args).toMatchObject({
    request: { deck_id: "travel-deck" },
  });
  await page.getByRole("button", { name: "Add card" }).click();
  await expect(page.getByLabel("Deck")).toHaveValue("travel-deck");
  await page.getByRole("button", { name: "Cancel" }).click();

  await page.getByRole("button", { name: "Rename deck" }).click();
  const renameDialog = page.getByRole("dialog", { name: "Rename deck" });
  await renameDialog.getByLabel("Name").fill("Audio");
  await renameDialog.getByRole("button", { name: "Rename deck" }).click();
  await expect(
    page.getByRole("heading", { name: "Audio", level: 1 }),
  ).toBeVisible();
  expect((await lastRequest(page, "rename_deck"))?.args).toMatchObject({
    request: { deck_id: "travel-deck", name: "Audio" },
  });

  await page.getByRole("button", { name: "Daily time", exact: true }).click();
  const timeDialog = page.getByRole("dialog", { name: "Daily time for Audio" });
  await timeDialog.getByRole("switch").click();
  await timeDialog.getByLabel("Minutes per day").fill("45");
  await timeDialog.getByRole("button", { name: "Save daily time" }).click();
  expect(
    (await lastRequest(page, "update_scheduler_settings"))?.args,
  ).toMatchObject({
    request: {
      deck_id: "travel-deck",
      deck_daily_time_budget_minutes: 45,
    },
  });

  await page.getByRole("button", { name: "Daily time", exact: true }).click();
  await page
    .getByRole("dialog", { name: "Daily time for Audio" })
    .getByRole("switch")
    .click();
  await page.getByRole("button", { name: "Save daily time" }).click();
  expect(
    (await lastRequest(page, "update_scheduler_settings"))?.args,
  ).toMatchObject({
    request: {
      deck_id: "travel-deck",
      deck_daily_time_budget_minutes: null,
    },
  });

  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await expect(page.getByLabel("Deck to configure")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Rename deck" })).toHaveCount(
    0,
  );
  await expect(page.getByRole("button", { name: "Delete deck" })).toHaveCount(
    0,
  );
  await expect(page.getByText("Override for this deck")).toHaveCount(0);
});

test("collection Settings ignores and clears a legacy Unsorted time override", async ({
  page,
}) => {
  await page.goto("/?settings=legacy-default-override");
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Save preferences" }),
  ).toBeEnabled();

  expect(
    (await lastRequest(page, "preview_scheduler_policy"))?.args,
  ).toMatchObject({
    request: {
      deck_id: "default-deck",
      deck_daily_time_budget_minutes: null,
    },
  });
  await expect(page.getByText(/Collection budget/).first()).toBeVisible();
  await page.getByRole("button", { name: "Save preferences" }).click();
  expect(
    (await lastRequest(page, "update_scheduler_settings"))?.args,
  ).toMatchObject({
    request: {
      deck_id: "default-deck",
      deck_daily_time_budget_minutes: null,
    },
  });
});
