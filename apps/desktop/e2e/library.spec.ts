import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Library", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Library", level: 1 }),
  ).toBeVisible();
});

async function lastRequest(page: Page, command: string) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

test("maps search and filter controls and renders the returned library DTO", async ({
  page,
}) => {
  await expect(page.getByText("日曜日は図書館に行きます")).toBeVisible();
  await expect(page.getByText("أنا أقرأ كتابًا في المكتبة")).toBeVisible();

  await page.getByRole("searchbox", { name: "Search library" }).fill(" كتاب ");
  await expect
    .poll(async () => (await lastRequest(page, "get_library"))?.args)
    .toMatchObject({ request: { query: " كتاب " } });

  await page.getByRole("button", { name: "Filters" }).click();
  await page.getByLabel("Deck").selectOption("travel-deck");
  await page.getByLabel("Due state").selectOption("scheduled");
  await page.getByLabel("Language metadata").selectOption("ar");
  await page.getByLabel("Media").selectOption("without_media");
  await expect
    .poll(async () => (await lastRequest(page, "get_library"))?.args)
    .toMatchObject({
      request: {
        deck_id: "travel-deck",
        due: "scheduled",
        language_tag: "ar",
        media: "without_media",
      },
    });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    ),
  ).toBe(false);
});

test("renders preview and cloze-removal DTOs from edit controls", async ({
  page,
}) => {
  const note = page
    .locator(".note-list > li")
    .filter({ hasText: "日曜日は図書館に行きます" });
  await note.getByRole("button", { name: "Preview" }).click();
  const preview = page.getByRole("dialog", { name: "Generated cards" });
  await expect(preview.getByText("日曜日は図書館に[…]")).toBeVisible();
  await expect(preview.getByText("行きます", { exact: true })).toBeVisible();
  await preview.getByRole("button", { name: "Close" }).click();

  await note.getByRole("button", { name: "Edit", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Add / Edit", level: 1 }),
  ).toBeVisible();
  await page
    .getByTestId("app-shell")
    .getByRole("button", { name: "Convert to text" })
    .click();
  const confirmation = page.getByRole("alertdialog", {
    name: "Convert this cloze to text?",
  });
  await expect(confirmation).toContainText("Saving will remove this card");
  await confirmation.getByRole("button", { name: "Convert to text" }).click();
  await expect(page.getByRole("button", { name: /Cloze 1/ })).toHaveCount(0);
  expect((await lastRequest(page, "remove_cloze"))?.args).toMatchObject({
    request: {
      cloze_id: "cloze-fixture",
      confirm_card_deletion: true,
    },
  });
});

test("maps selected bulk actions after showing destructive confirmation", async ({
  page,
}) => {
  await page.getByText("Select this page").click();
  await expect(page.getByText("2 notes selected")).toBeVisible();
  await page.getByRole("button", { name: "Suspend", exact: true }).click();
  await expect(
    page
      .getByTestId("app-shell")
      .getByRole("status")
      .filter({ hasText: "Suspended cards in 2 notes." }),
  ).toBeVisible();
  expect(
    (await lastRequest(page, "apply_library_bulk_action"))?.args,
  ).toMatchObject({
    request: {
      source_ids: ["sample-source", "source-ar"],
      action: "suspend",
    },
  });

  await page.getByText("Select this page").click();
  await page.getByRole("button", { name: "Move to Trash" }).click();
  const confirmation = page.getByRole("alertdialog", {
    name: "Move selected notes to Trash?",
  });
  await expect(confirmation).toContainText(
    "Review history and media stay intact",
  );
  await confirmation.getByRole("button", { name: "Move to Trash" }).click();
  expect(
    (await lastRequest(page, "apply_library_bulk_action"))?.args,
  ).toMatchObject({ request: { action: "delete" } });
});
