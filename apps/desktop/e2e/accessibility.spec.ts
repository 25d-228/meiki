import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
});

async function expectNoAccessibilityViolations(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(
    result.violations,
    result.violations
      .map(
        (violation) =>
          `${violation.id}: ${violation.nodes
            .map((node) => node.target.join(" "))
            .join(", ")}`,
      )
      .join("\n"),
  ).toEqual([]);
}

for (const screen of [
  "Today",
  "Study",
  "Library",
  "Add / Edit",
  "Settings",
] as const) {
  test(`${screen} has no automated WCAG A/AA violations`, async ({ page }) => {
    await page
      .getByRole("navigation", { name: "Primary navigation" })
      .getByRole("button", { name: screen, exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name: screen, level: 1 }),
    ).toBeVisible();
    await expectNoAccessibilityViolations(page);
  });
}

test("skip navigation, focus transfer, and live study states are exposed", async ({
  page,
}) => {
  await page.keyboard.press("Tab");
  const skipLink = page.getByRole("link", { name: "Skip to content" });
  await expect(skipLink).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("#main-content")).toBeFocused();

  await page.getByRole("button", { name: "Library", exact: true }).click();
  await expect(page.locator("#main-content")).toBeFocused();
  await page.getByRole("button", { name: "Study", exact: true }).click();
  const answer = page.getByLabel("Your answer");
  await expect(answer).toBeFocused();
  await answer.fill("行きます");
  await answer.press("Enter");
  await expect(page.locator(".reveal[aria-live='polite']")).toContainText(
    "Expected answer",
  );
  await page.keyboard.press("Enter");
  await expect(
    page.locator(".complete-state[aria-live='polite']"),
  ).toContainText("Review saved");
});

test("RTL learning content does not reverse application controls", async ({
  page,
}) => {
  await page.goto("/?fixture=rtl");
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expect(page.locator("#study-prompt")).toHaveAttribute("dir", "rtl");
  await expect(page.locator(".app-frame")).toHaveAttribute("dir", "ltr");
  await expectNoAccessibilityViolations(page);
});
