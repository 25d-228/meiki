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

async function search(page: Page, query: string, expected: string, count = 1) {
  await page.getByRole("searchbox", { name: "Search library" }).fill(query);
  await expect(page.getByText(`${count} matching notes`)).toBeVisible();
  await expect(page.getByText(expected, { exact: true })).toBeVisible();
}

test("searches normalized multilingual fields and filters deterministic results", async ({
  page,
}) => {
  await search(page, "図書館", "日曜日は図書館に行きます");
  await search(page, "كتاب", "أنا أقرأ كتابًا في المكتبة");
  await search(
    page,
    "café",
    "Réviser le ＣＡＦÉ sans modifier le texte stocké",
  );
  await search(
    page,
    " الساعة ",
    "Meetingは الساعة 三時 に始まる — this deliberately long multilingual source keeps 日本語, العربية, and English readable without changing stored text.",
  );

  await page.getByRole("searchbox", { name: "Search library" }).fill("");
  await page.getByRole("button", { name: "Filters" }).click();
  await page.getByLabel("Deck").selectOption("travel-deck");
  await expect(page.getByText("2 matching notes")).toBeVisible();
  await page.getByLabel("Deck").selectOption("");
  await page.getByLabel("Due state").selectOption("due");
  await expect(page.getByText("1 matching notes")).toBeVisible();
  await expect(page.getByText("日曜日は図書館に行きます")).toBeVisible();
  await page.getByLabel("Due state").selectOption("all");
  await page.getByLabel("Card state").selectOption("suspended");
  await expect(
    page.getByText("Réviser le ＣＡＦÉ sans modifier le texte stocké"),
  ).toBeVisible();
  await page.getByLabel("Card state").selectOption("all");
  await page.getByLabel("Language metadata").selectOption("ar");
  await expect(page.getByText("أنا أقرأ كتابًا في المكتبة")).toBeVisible();
  await page.getByLabel("Language metadata").selectOption("");
  await page.getByLabel("Media").selectOption("with_media");
  await expect(page.getByText("日曜日は図書館に行きます")).toBeVisible();

  const hasHorizontalOverflow = await page.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth,
  );
  expect(hasHorizontalOverflow).toBe(false);
});

test("previews generated cards, returns from editing, and confirms cloze deletion", async ({
  page,
}) => {
  await search(page, "図書館", "日曜日は図書館に行きます");
  await page.getByRole("button", { name: "Preview" }).click();
  const preview = page.getByRole("dialog", { name: "Generated cards" });
  await expect(preview.getByText("日曜日は図書館に[…]")).toBeVisible();
  await expect(preview.getByText("行きます", { exact: true })).toBeVisible();
  await preview.getByRole("button", { name: "Close dialog" }).click();

  await page.getByRole("button", { name: "Edit", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Add / Edit", level: 1 }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Return to Library" }).click();
  await expect(
    page.getByRole("heading", { name: "Library", level: 1 }),
  ).toBeVisible();

  await search(page, "図書館", "日曜日は図書館に行きます");
  await page.getByRole("button", { name: "Edit", exact: true }).click();
  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Saving will remove its card");
    await dialog.dismiss();
  });
  await page.getByRole("button", { name: "Convert to text" }).click();
  await expect(page.getByRole("button", { name: /Cloze 1/ })).toBeVisible();

  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Saving will remove its card");
    await dialog.accept();
  });
  await page.getByRole("button", { name: "Convert to text" }).click();
  await expect(page.getByRole("button", { name: /Cloze 1/ })).toHaveCount(0);
});

test("bulk actions report exact counts, support undo, export, trash, and restore", async ({
  page,
}) => {
  const reviewStateBefore = await page.evaluate(() =>
    localStorage.getItem("meiki-e2e-state"),
  );
  await page.getByText("Select this page").click();
  await expect(page.getByText("4 notes selected")).toBeVisible();
  await page.getByRole("button", { name: "Suspend", exact: true }).click();
  await expect(page.getByRole("status")).toContainText(
    "Suspended cards in 4 notes.",
  );
  await expect(page.getByRole("button", { name: "Undo" })).toHaveCount(0);

  await page.getByLabel("Select 日曜日は図書館に行きます").check();
  await page.getByLabel("Select أنا أقرأ كتابًا في المكتبة").check();
  await page.getByLabel("Tag name").fill("Priority");
  await page.getByRole("button", { name: "Add tag" }).click();
  await expect(page.getByRole("status")).toContainText("Tagged 2 notes.");
  await search(page, "priority", "日曜日は図書館に行きます", 2);
  await expect(page.getByText("أنا أقرأ كتابًا في المكتبة")).toBeVisible();

  await page.getByText("Select this page").click();
  await page.getByRole("button", { name: "Export" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Exported 2 notes to /tmp/exports/library-selection-e2e.json",
  );
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-e2e-library-export") ?? "[]"),
    ),
  ).toHaveLength(2);

  await page.getByRole("button", { name: "Move", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Moved 2 notes.");
  await search(page, "priority", "日曜日は図書館に行きます", 2);
  await page.getByText("Select this page").click();
  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Move 2 selected notes to Trash?");
    expect(dialog.message()).toContain("Review history and media stay intact");
    await dialog.accept();
  });
  await page.getByRole("button", { name: "Move to Trash" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Moved 2 notes to Trash.",
  );
  await page.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Undid the last action for 2 notes.",
  );

  await page.getByText("Select this page").click();
  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Move 2 selected notes to Trash?");
    await dialog.accept();
  });
  await page.getByRole("button", { name: "Move to Trash" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Moved 2 notes to Trash.",
  );

  await page.getByRole("searchbox", { name: "Search library" }).fill("");
  await page.getByRole("button", { name: "Filters" }).click();
  await page.getByLabel("Location").selectOption("deleted");
  await expect(page.getByText("2 matching notes")).toBeVisible();
  await expect(page.getByText("1 media")).toBeVisible();
  await page.getByText("Select this page").click();
  await page.getByRole("button", { name: "Restore" }).click();
  await expect(page.getByRole("status")).toContainText("Restored 2 notes.");
  await expect(
    page.getByRole("heading", { name: "Your library is ready" }),
  ).toBeVisible();

  expect(
    await page.evaluate(() => localStorage.getItem("meiki-e2e-state")),
  ).toBe(reviewStateBefore);
});
