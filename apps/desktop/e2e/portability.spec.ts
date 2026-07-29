import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
  await page
    .locator("#main-content")
    .getByRole("button", { name: "Settings", exact: true })
    .click();
});

test("exports and previews a confirmed versioned archive import", async ({
  page,
}) => {
  await page.getByRole("button", { name: "Export full collection" }).click();
  await expect(
    page.getByText(/Exported \d+ notes and 1 media objects/),
  ).toBeVisible();

  await page.getByRole("button", { name: "Preview an import" }).click();
  const dialog = page.getByRole("dialog", {
    name: "Preview archive import",
  });
  await expect(dialog).toContainText(
    "Validated 2 note(s), 2 card(s), and 1 media object(s).",
  );
  await expect(dialog.getByText("Version 1")).toBeVisible();
  await dialog.getByLabel("Import mode").selectOption("replace");
  await dialog.getByLabel("Type REPLACE to confirm").fill("REPLACE");
  await dialog.getByRole("button", { name: "Import archive" }).click();
  await expect(page.getByText(/Imported 2 notes/)).toBeVisible();
});
