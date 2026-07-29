import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

test("all primary screens have labelled responsive shells", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");

  await expect(
    page.getByRole("navigation", { name: "Primary navigation" }),
  ).toBeVisible();
  const screens = [
    ["Today", "Today"],
    ["Study", "Study"],
    ["Library", "Library"],
    ["Add / Edit", "Add / Edit"],
    ["Settings", "Settings"],
  ] as const;

  for (const [navigationName, headingName] of screens) {
    await page
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
  await page.goto("/");
  await page.getByRole("button", { name: "Library" }).click();
  await expect(
    page.getByRole("toolbar", { name: "Library tools" }),
  ).toBeVisible();
  await page.getByRole("searchbox", { name: "Search library" }).fill("不存在");
  await expect(
    page.getByRole("heading", { name: "No matching notes" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Add / Edit" }).click();
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
    await page.getByLabel("Theme").selectOption(theme);
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    const contrast = await page.evaluate(() => {
      const style = getComputedStyle(document.documentElement);
      const parse = (value: string) => {
        const hex = value.trim().replace("#", "");
        return [0, 2, 4].map((offset) =>
          Number.parseInt(hex.slice(offset, offset + 2), 16),
        );
      };
      const luminance = (color: number[]) => {
        const channels = color.map((channel) => {
          const normalized = channel / 255;
          return normalized <= 0.03928
            ? normalized / 12.92
            : ((normalized + 0.055) / 1.055) ** 2.4;
        });
        return (
          0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
        );
      };
      const text = luminance(parse(style.getPropertyValue("--color-text")));
      const surface = luminance(
        parse(style.getPropertyValue("--color-surface")),
      );
      return (
        (Math.max(text, surface) + 0.05) / (Math.min(text, surface) + 0.05)
      );
    });
    expect(contrast).toBeGreaterThanOrEqual(4.5);
  }

  const studyButton = page.getByRole("button", { name: "Study" });
  await studyButton.focus();
  expect(
    await studyButton.evaluate(
      (element) => getComputedStyle(element).boxShadow,
    ),
  ).not.toBe("none");
});

test("reduced motion and offline feedback are explicit", async ({
  context,
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/?fixture=loading");
  await expect(page.getByText("Opening your local collection…")).toBeVisible();
  const animationDuration = await page
    .locator(".spinner")
    .evaluate((element) => getComputedStyle(element).animationDuration);
  expect(Number.parseFloat(animationDuration)).toBeLessThanOrEqual(0.001);

  await context.setOffline(true);
  await page.evaluate(() => window.dispatchEvent(new Event("offline")));
  await expect(page.getByText("You are offline")).toBeVisible();
});

test("collection errors expose a labelled retry state", async ({ page }) => {
  await page.goto("/?fixture=error");
  await expect(
    page.getByRole("alert").getByText("The collection could not be opened"),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
});

for (const fixture of ["ltr", "rtl", "cjk", "mixed"] as const) {
  test(`study card visual snapshot: ${fixture}`, async ({ page }) => {
    await page.setViewportSize({ width: 1000, height: 760 });
    await page.goto(`/?fixture=${fixture}`);
    const prompt = page.locator("#study-prompt");
    await expect(prompt).toBeVisible();
    if (fixture === "rtl") {
      await expect(prompt).toHaveAttribute("dir", "rtl");
      await expect(page.locator(".app-frame")).toHaveAttribute("dir", "ltr");
    }
    await expect(page).toHaveScreenshot(`${fixture}.png`, {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.12,
    });
  });
}
