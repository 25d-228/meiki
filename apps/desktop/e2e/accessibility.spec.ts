import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

type Theme = "light" | "dark";
type Screen = "Today" | "Study" | "Library" | "Add / Edit" | "Settings";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
});

async function chooseTheme(page: Page, theme: Theme): Promise<void> {
  await page.getByRole("button", { name: "Theme" }).click();
  await page
    .getByRole("option", { name: new RegExp(`^${theme}$`, "i") })
    .click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
}

async function navigate(page: Page, screen: Screen): Promise<void> {
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: screen, exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: screen, level: 1 }),
  ).toBeVisible();
}

async function openStudyScenario(
  page: Page,
  route: string,
  theme: Theme,
): Promise<void> {
  await page.goto(route);
  await page.evaluate(() => {
    localStorage.removeItem("meiki-active-study-queue");
    localStorage.removeItem("meiki-active-study-session");
  });
  await page.reload();
  await chooseTheme(page, theme);
  await navigate(page, "Study");
}

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

for (const theme of ["light", "dark"] as const) {
  for (const screen of [
    "Today",
    "Study",
    "Library",
    "Add / Edit",
    "Settings",
  ] as const) {
    test(`${screen} in ${theme} has no automated WCAG A/AA violations`, async ({
      page,
    }) => {
      await chooseTheme(page, theme);
      await navigate(page, screen);
      await expectNoAccessibilityViolations(page);
    });
  }
}

for (const theme of ["light", "dark"] as const) {
  test(`loading, empty, error, and stale study states pass axe in ${theme}`, async ({
    page,
  }) => {
    await openStudyScenario(page, "/?fixture=loading", theme);
    await expect(
      page.getByText("Opening your local collection…"),
    ).toBeVisible();
    await expectNoAccessibilityViolations(page);

    await openStudyScenario(page, "/?collection=empty", theme);
    await expect(
      page.getByRole("heading", { name: "Your collection is empty" }),
    ).toBeVisible();
    await expectNoAccessibilityViolations(page);

    await openStudyScenario(page, "/?fixture=error", theme);
    await expect(
      page.getByRole("alert").getByText("The collection could not be opened"),
    ).toBeVisible();
    await expectNoAccessibilityViolations(page);

    await openStudyScenario(page, "/?fixture=stale", theme);
    await expect(page.getByRole("alert")).toContainText(
      "The study queue changed while it was loading.",
    );
    await expectNoAccessibilityViolations(page);
  });

  test(`destructive confirmation and success states pass axe in ${theme}`, async ({
    page,
  }) => {
    await chooseTheme(page, theme);
    await navigate(page, "Library");
    await page.getByText("Select this page").click();
    await page.getByRole("button", { name: "Move to Trash" }).click();
    await expect(
      page.getByRole("alertdialog", {
        name: "Move selected notes to Trash?",
      }),
    ).toBeVisible();
    await expectNoAccessibilityViolations(page);

    await openStudyScenario(page, "/", theme);
    const answer = page.getByLabel("Your answer");
    await answer.fill("行きます");
    await answer.press("Enter");
    await expect(
      page.getByText("Expected answer", { exact: true }),
    ).toBeVisible();
    await page.keyboard.press("Enter");
    await expect(
      page.getByRole("heading", { name: "Review saved" }),
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
  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
});

test("RTL learning content does not reverse application controls", async ({
  page,
}) => {
  await page.goto("/?fixture=rtl");
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expect(page.locator("#study-prompt")).toHaveAttribute("dir", "rtl");
  await expect(page.getByTestId("app-shell")).toHaveAttribute("dir", "ltr");
  await expectNoAccessibilityViolations(page);
});
