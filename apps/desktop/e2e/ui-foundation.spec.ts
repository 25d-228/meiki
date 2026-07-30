import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function openStudy(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await page.getByRole("button", { name: "Study", exact: true }).click();
}

test("all primary screens have labelled responsive shells", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");

  const openNavigation = page.getByRole("button", {
    name: "Open navigation",
  });
  await expect(openNavigation).toBeVisible();
  const screens = [
    ["Today", "Today"],
    ["Study", "Study"],
    ["Library", "Library"],
    ["Add / Edit", "Add / Edit"],
    ["Settings", "Settings"],
  ] as const;

  for (const [navigationName, headingName] of screens) {
    await openNavigation.click();
    await page
      .getByRole("navigation", { name: "Primary navigation" })
      .getByRole("button", { name: navigationName, exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name: headingName, level: 1 }),
    ).toBeVisible();
    await expect(page.locator("main [data-primary-action]")).toHaveCount(1);
    const hasHorizontalOverflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(hasHorizontalOverflow).toBe(false);
  }
});

test("dialog, toolbar, fields, and empty state are keyboard operable", async ({
  page,
}) => {
  await page.goto("/?collection=empty");
  await page.getByRole("button", { name: "Library" }).click();
  await expect(
    page.getByRole("search", { name: "Library tools" }),
  ).toBeVisible();
  await page.getByRole("searchbox", { name: "Search library" }).fill("不存在");
  await expect(
    page.getByRole("heading", { name: "No matching notes" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Add / Edit" }).click();
  const source = page.locator(".segment-text");
  await source.fill("Keyboard");
  await source.evaluate((element) => {
    const textarea = element as HTMLTextAreaElement;
    textarea.focus();
    textarea.setSelectionRange(0, 8);
    textarea.dispatchEvent(new Event("select", { bubbles: true }));
  });
  await page.getByRole("button", { name: "Make cloze" }).click();
  await page.getByRole("button", { name: "Preview" }).click();
  await expect(
    page.getByRole("dialog", { name: "Card preview" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
});

test("light and dark themes preserve text contrast and focus visibility", async ({
  page,
}) => {
  await page.goto("/");

  for (const theme of ["light", "dark"]) {
    await page.getByRole("button", { name: "Theme" }).click();
    await page
      .getByRole("option", {
        name: new RegExp(`^${theme}$`, "i"),
      })
      .click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    await expect(page.getByRole("listbox")).toBeHidden();
    const contrast = await new AxeBuilder({ page })
      .withRules(["color-contrast"])
      .analyze();
    expect(contrast.violations).toEqual([]);
  }

  const studyButton = page.getByRole("button", {
    name: "Study",
    exact: true,
  });
  await page.getByRole("button", { name: "Today", exact: true }).focus();
  await page.keyboard.press("Tab");
  await expect(studyButton).toBeFocused();
  const focusStyle = await studyButton.evaluate((element) => {
    const style = getComputedStyle(element);
    return { boxShadow: style.boxShadow, outlineStyle: style.outlineStyle };
  });
  expect(
    focusStyle.boxShadow !== "none" || focusStyle.outlineStyle !== "none",
  ).toBe(true);
});

test("budget-first scheduler previews before save and keeps expert controls explicit", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Settings" })
    .click();
  await expect(
    page.getByRole("group", { name: "Scheduling mode" }),
  ).toBeVisible();
  await expect(page.getByText("Policy preview", { exact: true })).toBeVisible();
  await page
    .getByRole("group", { name: "Daily study time presets" })
    .getByRole("button", { name: "1 hr", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Save preferences" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Preview policy" }).click();
  await expect(page.getByLabel("Policy explanation")).toContainText(
    "60 min/day",
  );
  await page.getByRole("button", { name: "Save preferences" }).click();
  await expect(page.getByText("Scheduling preferences saved.")).toBeVisible();

  await page.getByRole("switch", { name: "Enable" }).click();
  await page
    .getByRole("group", { name: "Scheduling mode" })
    .getByRole("button", { name: "Expert", exact: true })
    .click();
  await expect(
    page.getByLabel("Target retention (basis points)"),
  ).toBeVisible();
  await expect(page.getByText("fsrs-7", { exact: true })).toHaveCount(0);
  await page.getByLabel("Target retention (basis points)").fill("8750");
  await page.getByLabel("Maximum new cards per day").fill("12");
  await page.getByRole("button", { name: "Preview policy" }).click();
  await page.getByRole("button", { name: "Save preferences" }).click();
  await expect(page.getByText("Scheduling preferences saved.")).toBeVisible();
  expect(
    await page.evaluate(() =>
      localStorage.getItem("meiki-autoplay-prompt-audio"),
    ),
  ).toBe("true");

  await expect(
    page.getByRole("button", { name: "Personalize now" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Export parameters" }).click();
  await expect(page.getByText(/Scheduler parameters exported:/)).toBeVisible();

  await expect(
    page.getByRole("button", { name: "Back up and rebuild schedules" }),
  ).toHaveCount(0);
});

test("reduced motion is explicit", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openStudy(page, "/?fixture=loading");
  await expect(page.getByText("Opening your local collection…")).toBeVisible();
  const animationDuration = await page
    .locator(".spinner")
    .evaluate((element) => getComputedStyle(element).animationDuration);
  expect(Number.parseFloat(animationDuration)).toBeLessThanOrEqual(0.001);
});

test("collection errors expose a labelled retry state", async ({ page }) => {
  await openStudy(page, "/?fixture=error");
  await expect(
    page.getByRole("alert").getByText("The collection could not be opened"),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
});

for (const fixture of ["ltr", "rtl", "cjk", "mixed"] as const) {
  test(`study card visual snapshot: ${fixture}`, async ({ page }) => {
    await page.setViewportSize({ width: 1000, height: 760 });
    await openStudy(page, `/?fixture=${fixture}`);
    const prompt = page.locator("#study-prompt");
    await expect(prompt).toBeVisible();
    if (fixture === "rtl") {
      await expect(prompt).toHaveAttribute("dir", "rtl");
      await expect(page.getByTestId("app-shell")).toHaveAttribute("dir", "ltr");
    }
    await expect(page).toHaveScreenshot(`${fixture}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}
