import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function openToday(page: import("@playwright/test").Page): Promise<void> {
  const heading = page.getByRole("heading", { name: "Today", level: 1 });
  if (!(await heading.isVisible())) {
    const openNavigation = page.getByRole("button", {
      name: "Open navigation",
    });
    if (await openNavigation.isVisible()) await openNavigation.click();
    await page.getByRole("button", { name: "Today", exact: true }).click();
  }
  await expect(heading).toBeVisible();
}

function statusMessage(page: import("@playwright/test").Page, text: string) {
  return page
    .getByTestId("app-shell")
    .getByRole("status")
    .filter({ hasText: text });
}

test("shows empty, overdue, and capped workload states", async ({ page }) => {
  await page.goto("/?today=empty");
  await openToday(page);
  await expect(page.getByText("You’re caught up")).toBeVisible();
  await expect(page.getByText(/Next review:/)).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Start study" }),
  ).toBeDisabled();

  await page.goto("/?today=overdue");
  await openToday(page);
  await expect(page.getByText("2 due and 1 new.")).toBeVisible();
  await expect(
    page.getByRole("status").getByText("1 overdue review"),
  ).toBeVisible();

  await page.goto("/?today=capped");
  await openToday(page);
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
  await expect(page.getByText("New-card intake capped")).toBeVisible();
  await expect(statusMessage(page, "2 new cards are deferred")).toBeVisible();
  await expect(page.getByRole("button", { name: "Start study" })).toBeEnabled();

  await page.goto("/?today=backlog");
  await openToday(page);
  await expect(page.getByText("Due work exceeds today’s budget")).toBeVisible();
  await expect(
    statusMessage(page, "Every due review remains available."),
  ).toBeVisible();
});

test("filters by deck and explains a time-budget change", async ({ page }) => {
  await page.goto("/?today=budget");
  await openToday(page);
  await expect(page.getByText("1 due and 3 new.")).toBeVisible();

  await page.getByLabel("Deck").selectOption("travel-deck");
  await expect(page.locator(".queue > .eyebrow")).toHaveText("Travel phrases");
  await expect(
    page.locator("header").getByText("Today · Travel phrases"),
  ).toBeVisible();

  await page
    .locator("#main-content")
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await page.getByLabel("Daily study hours").fill("0");
  await page.getByLabel("Daily study minutes").fill("1");
  await page.getByRole("button", { name: "Preview policy" }).click();
  await page.getByRole("button", { name: "Save preferences" }).click();
  await expect(page.getByText("Scheduling preferences saved.")).toBeVisible();

  await page.getByRole("button", { name: "Today", exact: true }).click();
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
  await expect(statusMessage(page, "2 new cards are deferred")).toBeVisible();
  await expect(
    statusMessage(page, "Due reviews were not deferred."),
  ).toBeVisible();
});

test("resumes an interrupted queue without duplicating or losing cards", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/?today=normal&fixture=longmixed");
  await openToday(page);
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
  await page.getByRole("button", { name: "Start study" }).click();

  await expect(page.getByText(/deliberately long multilingual/)).toBeVisible();
  await page.getByLabel("Your answer").fill("三時");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(() => {
        const stored = localStorage.getItem("meiki-active-study-queue");
        return stored ? JSON.parse(stored).position : null;
      }),
    )
    .toBe(1);

  await page.reload();
  await expect(page.getByText("Resume where you stopped")).toBeVisible();
  await page.getByRole("button", { name: "Resume study" }).click();
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByText("1 card remaining")).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    ),
  ).toBe(false);

  await page.getByLabel("Your answer").fill("三時");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("Enter");
  await page.getByRole("button", { name: "Finish session" }).click();

  await expect(page.getByTestId("app-announcement")).toHaveText(
    "Study queue complete. Returning to Today.",
  );
  await expect(
    page.getByRole("heading", { name: "Today", level: 1 }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-active-study-queue")),
  ).toBeNull();
});
