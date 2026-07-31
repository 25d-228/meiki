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

async function latestRequest(
  page: import("@playwright/test").Page,
  name: string,
) {
  return page.evaluate((command) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === command).at(-1);
  }, name);
}

test("adds a pristine deck and keeps replacement explicitly destructive", async ({
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
    'Ready to add deck "Japanese Foundation 1" with 1 note(s), 1 card(s), and 1 media object(s).',
  );
  await expect(dialog.getByText("Version 4")).toBeVisible();
  await expect(
    dialog.getByText("Japanese Foundation 1", { exact: true }),
  ).toBeVisible();
  await dialog.getByRole("button", { name: "Add deck" }).click();
  await expect(
    page.getByText(/Added deck “Japanese Foundation 1” with 1 card/),
  ).toBeVisible();
  expect(await latestRequest(page, "add_archive_deck")).toMatchObject({
    args: {
      request: {
        path: "/tmp/exports/meiki-e2e.meiki",
      },
    },
  });

  await page.getByRole("button", { name: "Preview an import" }).click();
  const replacement = page.getByRole("dialog", {
    name: "Preview archive import",
  });
  await replacement
    .getByLabel("Type REPLACE to replace collection")
    .fill("REPLACE");
  await replacement.getByRole("button", { name: "Replace collection" }).click();
  await expect(page.getByText(/Imported 2 notes/)).toBeVisible();
});
