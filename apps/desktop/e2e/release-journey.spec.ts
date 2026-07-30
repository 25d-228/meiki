import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

async function selectRange(
  page: Page,
  start: number,
  end: number,
): Promise<void> {
  await page
    .locator(".segment-text")
    .last()
    .evaluate(
      (element, range) => {
        const input = element as HTMLTextAreaElement;
        input.focus();
        input.setSelectionRange(range.start, range.end);
        input.dispatchEvent(new Event("select", { bubbles: true }));
      },
      { start, end },
    );
}

test("desktop shell reaches its primary action within the startup budget", async ({
  page,
}) => {
  await installMockApi(page);
  await page.goto("/");
  await expect(
    page.locator("#main-content").getByRole("button", { name: "Start study" }),
  ).toBeVisible();
  const navigation = await page.evaluate(() => {
    const [entry] = performance.getEntriesByType(
      "navigation",
    ) as PerformanceNavigationTiming[];
    return entry.domContentLoadedEventEnd - entry.startTime;
  });
  expect(navigation).toBeLessThanOrEqual(2_000);
});

test("offline first-release journey creates, studies, undoes, restarts, exports, and restores", async ({
  context,
  page,
}) => {
  await installMockApi(page);
  await page.goto("/");
  await context.setOffline(true);
  await page.evaluate(() => window.dispatchEvent(new Event("offline")));
  await expect(page.getByText("You are offline")).toBeVisible();

  await page.getByRole("button", { name: "Add / Edit" }).click();
  const source = page.locator(".segment-text").last();
  await source.fill("日曜日は図書館に行きます");
  await selectRange(page, 8, 12);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await page.getByLabel("Accepted answers").fill("ゆきます");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(
    page.getByText("Source note saved on this device."),
  ).toBeVisible();

  await page.getByRole("button", { name: "Study", exact: true }).click();
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await expect(page.getByText("exact", { exact: true })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
  await page.keyboard.press("ControlOrMeta+z");
  await expect(page.getByText("Last review undone.")).toBeVisible();

  await context.setOffline(false);
  await page.reload();
  await context.setOffline(true);
  await page.evaluate(() => window.dispatchEvent(new Event("offline")));
  await expect(page.getByText("You are offline")).toBeVisible();
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expect(page.getByLabel("Your answer")).toBeFocused();
  await expect(page.getByText("2 cards remaining")).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem("meiki-e2e-state") ?? "{}")
          .completedReviews,
    ),
  ).toBe(0);

  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("button", { name: "Export full collection" }).click();
  await expect(page.getByText(/Exported \d+ notes/)).toBeVisible();
  await page.getByRole("button", { name: "Preview an import" }).click();
  const dialog = page.getByRole("dialog", {
    name: "Preview archive import",
  });
  await dialog.getByLabel("Import mode").selectOption("replace");
  await dialog.getByLabel("Type REPLACE to confirm").fill("REPLACE");
  await dialog.getByRole("button", { name: "Import archive" }).click();
  await expect(page.getByText(/Imported 2 notes/)).toBeVisible();
});
