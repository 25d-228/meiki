import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
  await page
    .locator("#main-content")
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Settings", level: 1 }),
  ).toBeVisible();
});

async function lastRequest(
  page: import("@playwright/test").Page,
  command: string,
) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

test("maps create, rename, and confirmed delete controls", async ({ page }) => {
  await page.goto("/?decks=lifecycle");
  await page
    .locator("#main-content")
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await page.getByLabel("New deck name").fill(" Listening ");
  await page.getByRole("button", { name: "Create deck" }).click();
  await expect(page.getByText("Created deck “Listening”.")).toBeVisible();
  expect((await lastRequest(page, "create_deck"))?.args).toMatchObject({
    request: { name: " Listening " },
  });
  await expect(page.getByLabel("Deck name", { exact: true })).toHaveValue(
    "Listening",
  );

  await page.getByLabel("Deck name", { exact: true }).fill("Audio");
  await page.getByRole("button", { name: "Rename deck" }).click();
  await expect(page.getByText("Renamed deck to “Audio”.")).toBeVisible();
  expect((await lastRequest(page, "rename_deck"))?.args).toMatchObject({
    request: { deck_id: "listening-deck", name: "Audio" },
  });

  await page.getByRole("button", { name: "Delete deck" }).click();
  const confirmation = page.getByRole("alertdialog", {
    name: "Delete this deck?",
  });
  await expect(confirmation).toContainText("Delete deck “Audio”");
  await confirmation.getByRole("button", { name: "Delete deck" }).click();
  await expect(page.getByText("Deleted empty deck “Audio”.")).toBeVisible();
  expect((await lastRequest(page, "delete_deck"))?.args).toMatchObject({
    request: {
      deck_id: "listening-deck",
      confirmation: "Audio",
    },
  });
});

test("maps the selected destination for a non-empty deck", async ({ page }) => {
  await page.getByLabel("Deck to configure").selectOption("travel-deck");
  await expect(page.getByLabel("Deck name", { exact: true })).toHaveValue(
    "Travel phrases",
  );
  await page
    .getByLabel("Move notes before deletion")
    .selectOption("default-deck");

  await page.getByRole("button", { name: "Delete deck" }).click();
  const confirmation = page.getByRole("alertdialog", {
    name: "Delete this deck?",
  });
  await expect(confirmation).toContainText("Delete deck “Travel phrases”");
  await expect(confirmation).toContainText("Move 1 note");
  await confirmation.getByRole("button", { name: "Delete deck" }).click();
  await expect(
    page.getByText("Deleted “Travel phrases” and moved 2 notes."),
  ).toBeVisible();
  expect((await lastRequest(page, "delete_deck"))?.args).toMatchObject({
    request: {
      deck_id: "travel-deck",
      move_notes_to_deck_id: "default-deck",
      confirmation: "Travel phrases",
    },
  });
});
