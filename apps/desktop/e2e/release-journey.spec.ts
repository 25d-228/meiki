import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

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
