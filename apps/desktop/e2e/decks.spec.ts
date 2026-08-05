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

test("shows active card counts and presents the populated default deck as Unsorted", async ({
  page,
}) => {
  await openDecks(page);

  const unsorted = page.getByTestId("deck-default-deck");
  await expect(unsorted.getByText("Unsorted", { exact: true })).toBeVisible();
  await expect(unsorted.locator("dl")).toContainText(
    /Total\s*2\s*Due\s*1\s*New\s*1/,
  );
});

test("hides the empty internal default deck", async ({ page }) => {
  await page.goto("/?decks=empty-default");
  await openDecks(page);

  await expect(page.getByTestId("deck-default-deck")).toHaveCount(0);
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
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

test("opens deck-scoped management and keeps rename and deletion in Settings", async ({
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
  expect((await lastRequest(page, "get_library"))?.args).toMatchObject({
    request: { deck_id: "travel-deck" },
  });
  await page.getByRole("button", { name: "Add a source note" }).click();
  await expect(page.getByLabel("Deck")).toHaveValue("travel-deck");

  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await page.getByLabel("Deck to configure").selectOption("travel-deck");
  await page.getByLabel("Deck name", { exact: true }).fill("Audio");
  await page.getByRole("button", { name: "Rename deck" }).click();
  await expect(page.getByText("Renamed deck to “Audio”.")).toBeVisible();

  await page
    .getByLabel("Move notes before deletion")
    .selectOption("default-deck");
  await page.getByRole("button", { name: "Delete deck" }).click();
  const confirmation = page.getByRole("alertdialog", {
    name: "Delete this deck?",
  });
  await confirmation.getByRole("button", { name: "Delete deck" }).click();
  expect((await lastRequest(page, "delete_deck"))?.args).toMatchObject({
    request: {
      deck_id: "travel-deck",
      move_notes_to_deck_id: "default-deck",
      confirmation: "Audio",
    },
  });
});
