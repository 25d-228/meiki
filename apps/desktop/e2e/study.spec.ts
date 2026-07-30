import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function openStudy(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await page.getByRole("button", { name: "Study", exact: true }).click();
}

async function seedPendingReview(page: Page, eventId: string): Promise<void> {
  await page.evaluate((reviewEventId) => {
    const key = "meiki-active-study-queue";
    const queue = JSON.parse(localStorage.getItem(key) ?? "null") as {
      entries: Array<{
        card_id: string;
        card_content_version: number;
        schedule_version: number;
      }>;
      position: number;
      pendingReview: unknown;
    };
    const current = queue.entries[queue.position];
    queue.pendingReview = {
      review_event_id: reviewEventId,
      card_id: current.card_id,
      card_content_version: current.card_content_version,
      schedule_version: current.schedule_version,
      raw_response: "行きます",
      chosen_grade: "good",
      response_duration_ms: 1_000,
    };
    localStorage.setItem(key, JSON.stringify(queue));
  }, eventId);
}

async function invokePendingReview(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const queue = JSON.parse(
      localStorage.getItem("meiki-active-study-queue") ?? "null",
    ) as { pendingReview: unknown };
    await window.__MEIKI_TEST_INVOKE__?.("grade_review", {
      request: queue.pendingReview,
    });
  });
}

async function expectOneReviewAndNextCard(page: Page): Promise<void> {
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByText("1 card remaining")).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          Object.keys(
            JSON.parse(
              localStorage.getItem("meiki-e2e-review-events") ?? "{}",
            ) as Record<string, unknown>,
          ).length,
      ),
    )
    .toBe(1);
}

test("keeps a clean collection empty and links Study to cloze authoring", async ({
  page,
}) => {
  await page.goto("/?today=empty&collection=empty");
  await expect(
    page.getByRole("heading", { name: "Today", level: 1 }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Library", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Library", exact: true, level: 1 }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Your library is ready" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Settings", exact: true, level: 1 }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Your collection is empty" }),
  ).toBeVisible();
  const primaryActions = page.locator("[data-primary-action]");
  await expect(primaryActions).toHaveCount(1);
  await expect(
    page.getByRole("button", { name: "Create a cloze" }),
  ).toBeVisible();
  await expect(page.getByText("日曜日は図書館に行きます")).toHaveCount(0);

  await page.getByRole("button", { name: "Create a cloze" }).click();
  await expect(
    page.getByRole("heading", { name: "Add / Edit", level: 1 }),
  ).toBeVisible();
});

test("shows the next due time when the collection has no eligible card", async ({
  page,
}) => {
  await openStudy(page, "/?today=empty");
  await expect(
    page.getByRole("heading", { name: "Nothing is due" }),
  ).toBeVisible();
  await expect(page.getByText(/Your next review is due/)).toBeVisible();
  await page.getByRole("button", { name: "Return to Today" }).click();
  await expect(
    page.getByRole("heading", { name: "Today", level: 1 }),
  ).toBeVisible();
});

test("checks, grades, and resumes at the next eligible card", async ({
  page,
}) => {
  await openStudy(page, "/");
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
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expectOneReviewAndNextCard(page);
});

test("plays prompt and answer audio, reveals an image, and tolerates missing media", async ({
  page,
}) => {
  await openStudy(page, "/?media=ready");
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

  await openStudy(page, "/?media=missing");
  await expect(page.getByText("Media file is missing")).toBeVisible();
  await expect(page.getByLabel("Your answer")).toBeEnabled();
});

test("does not submit Enter during IME composition", async ({ page }) => {
  await openStudy(page, "/");
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
    await openStudy(page, `/?fixture=${fixture}`);
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
  await openStudy(page, "/?fixture=cjk");
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
  await openStudy(page, "/?fixture=cjk");
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
  await expect(page.getByText("2 cards remaining")).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem("meiki-e2e-state") ?? "{}")
          .completedReviews,
    ),
  ).toBe(0);
  await expect(page.getByLabel("Your answer")).toBeFocused();
});

test("retries interrupted checks and commits without losing review state", async ({
  page,
}) => {
  await openStudy(page, "/?fixture=ltr&failure=check");
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

  await openStudy(page, "/?fixture=cjk&failure=grade");
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
  await openStudy(page, "/?fixture=cjk");
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

test("recovers a persisted review command before it was sent", async ({
  page,
}) => {
  await openStudy(page, "/?fixture=cjk");
  await seedPendingReview(page, "crash-before-send");

  await page.reload();
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expectOneReviewAndNextCard(page);
});

test("recovers after send but before the review commit", async ({ page }) => {
  await openStudy(page, "/?fixture=cjk&failure=grade");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    page.getByText("The review commit was interrupted."),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "{}")
          .pendingReview?.review_event_id,
    ),
  ).toBeTruthy();

  await page.goto("/?fixture=cjk");
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expectOneReviewAndNextCard(page);
});

test("recovers after commit but before the response", async ({ page }) => {
  await openStudy(page, "/?fixture=cjk&failure=grade-response");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    page.getByText("The review response was interrupted."),
  ).toBeVisible();

  await page.reload();
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expectOneReviewAndNextCard(page);
});

test("recovers after a response but before local queue advancement", async ({
  page,
}) => {
  await openStudy(page, "/?fixture=cjk");
  await seedPendingReview(page, "crash-after-response");
  await invokePendingReview(page);
  expect(
    await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "{}")
          .position,
    ),
  ).toBe(0);

  await page.reload();
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expectOneReviewAndNextCard(page);
});

test("recovers after local advancement but before the next render", async ({
  page,
}) => {
  await openStudy(page, "/?fixture=cjk");
  await seedPendingReview(page, "crash-after-advance");
  await invokePendingReview(page);
  await page.evaluate(() => {
    const key = "meiki-active-study-queue";
    const queue = JSON.parse(localStorage.getItem(key) ?? "{}");
    queue.position += 1;
    queue.pendingReview = null;
    localStorage.setItem(key, JSON.stringify(queue));
  });

  await page.reload();
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expectOneReviewAndNextCard(page);
});
