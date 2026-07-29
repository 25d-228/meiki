import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

test("checks, grades, and restores the walking-skeleton card", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByText("日曜日は図書館に[…]")).toBeVisible();

  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await expect(page.getByText("Expected answer")).toBeVisible();
  await expect(page.getByText("exact", { exact: true })).toBeVisible();

  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();

  await page.reload();
  await expect(page.getByText("1 review saved")).toBeVisible();
});

test("plays prompt and answer audio, reveals an image, and tolerates missing media", async ({
  page,
}) => {
  await page.goto("/?media=ready");
  await expect(
    page.getByRole("button", { name: "Replay audio" }),
  ).toBeVisible();
  await expect(page.locator("audio")).toHaveCount(1);
  await expect(page.locator("audio")).not.toHaveAttribute("autoplay");
  await page.locator("#study-prompt").click();
  await page.keyboard.press("r");
  await expect
    .poll(() =>
      page.evaluate(() =>
        Number(localStorage.getItem("meiki-e2e-media-play-count") ?? "0"),
      ),
    )
    .toBe(1);

  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await expect(page.locator("audio")).toHaveCount(1);
  await expect(
    page.getByRole("img", { name: "A quiet library reading room" }),
  ).toBeVisible();
  await page.keyboard.press("r");
  await expect
    .poll(() =>
      page.evaluate(() =>
        Number(localStorage.getItem("meiki-e2e-media-play-count") ?? "0"),
      ),
    )
    .toBe(2);

  await page.goto("/?media=missing");
  await expect(page.getByText("Media file is missing")).toBeVisible();
  await expect(page.getByLabel("Your answer")).toBeEnabled();
});

test("does not submit Enter during IME composition", async ({ page }) => {
  await page.goto("/");
  const input = page.getByLabel("Your answer");
  await input.dispatchEvent("compositionstart");
  await input.press("Enter");
  await expect(page.getByText("Expected answer")).toBeHidden();

  await input.dispatchEvent("compositionend");
  await page.getByRole("button", { name: /Check answer/ }).click();
  await expect(page.getByText("Expected answer")).toBeVisible();
});

test("preserves multilingual and multi-code-point review input", async ({
  page,
}) => {
  const fixtures = [
    ["cjk", "行きます"],
    ["rtl", "کتاب"],
    ["devanagari", "पुस्तक"],
    ["ltr", " la bibliothe\u{300}que "],
    ["mixed", "三時"],
    ["emoji", "👨‍👩‍👧‍👦"],
  ] as const;

  for (const [fixture, response] of fixtures) {
    await page.goto(`/?fixture=${fixture}`);
    const input = page.getByLabel("Your answer");
    await input.fill(response);
    await expect(input).toHaveValue(response);
    await input.press("Enter");
    await expect(page.getByText("exact", { exact: true })).toBeVisible();
    await expect(page.getByTestId("answer-difference")).toBeVisible();
    if (fixture === "ltr") {
      await expect(page.getByText("Compared as:")).toBeVisible();
      await expect(
        page.getByTestId("answer-difference").locator("bdi"),
      ).toHaveText("la bibliothèque");
    }
  }
});

test("renders grapheme difference semantics without altering raw input", async ({
  page,
}) => {
  await page.goto("/?fixture=cjk");
  await page.getByLabel("Your answer").fill(" 図書館 ");
  await page.getByLabel("Your answer").press("Enter");

  const difference = page.getByTestId("answer-difference");
  await expect(difference.locator("del")).toHaveText("行きます");
  await expect(difference.locator("ins")).toHaveText("図書館");
  await expect(
    page.locator(".answer-comparison strong").nth(1),
  ).toHaveJSProperty("textContent", " 図書館 ");
  await expect(page.getByText("Compared as:")).toBeVisible();
});

test("completes and undoes a review with keyboard-only controls", async ({
  page,
}) => {
  await page.goto("/?fixture=cjk");
  await page.keyboard.type("行きます");
  await page.waitForTimeout(20);
  await page.keyboard.press("Enter");

  await expect(page.locator("#study-prompt mark")).toHaveText("行きます");
  await expect(page.getByRole("button", { name: /Good.*3d/i })).toBeVisible();
  await page.keyboard.press("4");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();

  const request = await page.evaluate(() =>
    JSON.parse(localStorage.getItem("meiki-e2e-last-grade-request") ?? "{}"),
  );
  expect(request.chosen_grade).toBe("easy");
  expect(request.response_duration_ms).toBeGreaterThan(0);

  await page.keyboard.press("ControlOrMeta+z");
  await expect(page.getByText("Last review undone.")).toBeVisible();
  await expect(page.getByText("0 reviews saved")).toBeVisible();
  await expect(page.getByLabel("Your answer")).toBeFocused();
});

test("retries interrupted checks and commits without losing review state", async ({
  page,
}) => {
  await page.goto("/?fixture=ltr&failure=check");
  const input = page.getByLabel("Your answer");
  await input.fill(" la bibliothe\u{300}que ");
  await input.press("Enter");
  await expect(
    page.getByText("The answer check was interrupted."),
  ).toBeVisible();

  await page.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByText("Expected answer")).toBeVisible();
  await expect(
    page.locator(".answer-comparison strong").nth(1),
  ).toHaveJSProperty("textContent", " la bibliothe\u{300}que ");

  await page.goto("/?fixture=cjk&failure=grade");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    page.getByText("The review commit was interrupted."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Try again" }).click();
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
});

test("edits and suspends the active card without losing the reveal", async ({
  page,
}) => {
  await page.goto("/?fixture=cjk");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("e");

  await expect(page.getByRole("heading", { name: "Add / Edit" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Cloze 1 行きます" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Return to study" }).click();
  await expect(page.getByText("Expected answer")).toBeVisible();
  await expect(page.locator(".answer-comparison strong").nth(1)).toHaveText(
    "行きます",
  );

  await page.keyboard.press("s");
  await expect(
    page.getByRole("heading", { name: "Card suspended" }),
  ).toBeVisible();
});
