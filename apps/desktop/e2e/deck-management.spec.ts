import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
  await openTravelDeck(page);
});

async function openTravelDeck(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Decks", exact: true })
    .click();
  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Open" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Travel phrases", level: 1 }),
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

test("shows one deck's cards and searches mixed-script sentences and answers", async ({
  page,
}) => {
  expect((await lastRequest(page, "get_deck_cards"))?.args).toMatchObject({
    request: { deck_id: "travel-deck", trash: "active" },
  });
  await expect(page.getByTestId("card-card-ar")).toContainText(
    "أنا أقرأ […] في المكتبة",
  );
  await expect(page.getByTestId("card-card-ar")).toContainText("كتابًا");
  await expect(page.getByTestId("card-travel-new-card")).toContainText(
    "Take the […] train",
  );

  await page.getByLabel("Search cards").fill("كتابًا");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.getByTestId("card-card-ar")).toBeVisible();
  await expect(page.getByTestId("card-travel-new-card")).toHaveCount(0);
  expect((await lastRequest(page, "get_deck_cards"))?.args).toMatchObject({
    request: { deck_id: "travel-deck", query: "كتابًا" },
  });

  await expect(page.getByText("Library", { exact: true })).toHaveCount(0);
  await expect(page.getByText(/source note/i)).toHaveCount(0);
  expect(await lastRequest(page, "get_library")).toBeUndefined();
});

test("adds and edits from the opened deck and returns after cancel or save", async ({
  page,
}) => {
  await page.getByRole("button", { name: "Add card" }).click();
  await expect(page.getByLabel("Deck")).toHaveValue("travel-deck");
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(
    page.getByRole("heading", { name: "Travel phrases", level: 1 }),
  ).toBeVisible();

  await page
    .getByTestId("card-card-ar")
    .getByRole("button", { name: "Edit" })
    .click();
  expect(
    (await lastRequest(page, "get_authoring_draft_for_card"))?.args,
  ).toMatchObject({ cardId: "card-ar" });
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Travel phrases", level: 1 }),
  ).toBeVisible();
});

test("moves, suspends, trashes, and restores the selected card identity", async ({
  page,
}) => {
  const card = page.getByTestId("card-card-ar");
  await card.getByRole("button", { name: "Move", exact: true }).click();
  const moveDialog = page.getByRole("dialog", { name: "Move card" });
  await moveDialog.getByLabel("Destination deck").selectOption("default-deck");
  await moveDialog.getByRole("button", { name: "Move card" }).click();
  expect(
    (await lastRequest(page, "apply_deck_card_action"))?.args,
  ).toMatchObject({
    request: {
      deck_id: "travel-deck",
      card_ids: ["card-ar"],
      action: "move",
      destination_deck_id: "default-deck",
    },
  });

  await card.getByRole("button", { name: "Suspend" }).click();
  expect(
    (await lastRequest(page, "apply_deck_card_action"))?.args,
  ).toMatchObject({ request: { card_ids: ["card-ar"], action: "suspend" } });

  await card.getByRole("button", { name: "Move to Trash" }).click();
  expect(
    (await lastRequest(page, "apply_deck_card_action"))?.args,
  ).toMatchObject({ request: { card_ids: ["card-ar"], action: "trash" } });

  await page.getByRole("button", { name: "Show Trash" }).click();
  const trashed = page.getByTestId("card-trashed-card");
  await expect(trashed).toBeVisible();
  await trashed.getByRole("button", { name: "Restore" }).click();
  expect(
    (await lastRequest(page, "apply_deck_card_action"))?.args,
  ).toMatchObject({
    request: { card_ids: ["trashed-card"], action: "restore" },
  });
});
