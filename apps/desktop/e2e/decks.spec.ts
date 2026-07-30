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

test("creates, renames, and safely deletes an empty flat deck", async ({
  page,
}) => {
  await page.getByLabel("New deck name").fill(" Listening ");
  await page.getByRole("button", { name: "Create deck" }).click();
  await expect(page.getByText("Created deck “Listening”.")).toBeVisible();
  await expect(page.getByLabel("Deck name", { exact: true })).toHaveValue(
    "Listening",
  );

  await page.getByLabel("Deck name", { exact: true }).fill("Audio");
  await page.getByRole("button", { name: "Rename deck" }).click();
  await expect(page.getByText("Renamed deck to “Audio”.")).toBeVisible();

  await page.getByRole("button", { name: "Delete deck" }).click();
  const confirmation = page.getByRole("alertdialog", {
    name: "Delete this deck?",
  });
  await expect(confirmation).toContainText("Delete deck “Audio”");
  await confirmation.getByRole("button", { name: "Delete deck" }).click();
  await expect(page.getByText("Deleted empty deck “Audio”.")).toBeVisible();
  await expect(
    page.getByLabel("Deck to configure").getByRole("option", { name: /Audio/ }),
  ).toHaveCount(0);
});

test("moves notes before deleting a non-empty deck", async ({ page }) => {
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
  await expect(confirmation).toContainText("Move 2 notes");
  await confirmation.getByRole("button", { name: "Delete deck" }).click();
  await expect(
    page.getByText("Deleted “Travel phrases” and moved 2 notes."),
  ).toBeVisible();

  await page.getByRole("button", { name: "Library", exact: true }).click();
  await page.getByRole("searchbox", { name: "Search library" }).fill("كتاب");
  const movedNote = page
    .locator(".note-list > li")
    .filter({ hasText: "أنا أقرأ كتابًا في المكتبة" });
  await expect(movedNote).toBeVisible();
  await expect(movedNote.getByText("Default", { exact: true })).toBeVisible();
});
