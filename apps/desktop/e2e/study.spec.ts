import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function openStudy(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await page.getByRole("button", { name: "Study", exact: true }).click();
}

async function lastRequest(page: Page, command: string) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

test("renders empty and nothing-due DTO states with their UI actions", async ({
  page,
}) => {
  await openStudy(page, "/?today=empty&collection=empty");
  await expect(
    page.getByRole("heading", { name: "Your collection is empty" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Create a cloze" }),
  ).toBeVisible();

  await openStudy(page, "/?today=empty");
  await expect(
    page.getByRole("heading", { name: "Nothing is due" }),
  ).toBeVisible();
  await expect(page.getByText(/Your next review is due/)).toBeVisible();
});

test("maps answer and grade requests and renders returned DTOs", async ({
  page,
}) => {
  await openStudy(page, "/");
  await expect(page.getByText("日曜日は図書館に[…]")).toBeVisible();

  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await expect(page.getByText("Expected answer")).toBeVisible();
  await expect(page.getByText("exact", { exact: true })).toBeVisible();
  expect((await lastRequest(page, "check_answer"))?.args).toMatchObject({
    request: { card_id: "due-card", raw_response: "行きます" },
  });

  await page.keyboard.press("4");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
  const grade = await lastRequest(page, "grade_review");
  expect(grade?.args).toMatchObject({
    request: {
      card_id: "due-card",
      raw_response: "行きます",
      chosen_grade: "easy",
    },
  });
  expect(
    (grade?.args as { request: { response_duration_ms: number } }).request
      .response_duration_ms,
  ).toBeGreaterThanOrEqual(0);
});

test("renders ready and missing media DTOs without autoplay", async ({
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
  await expect(
    page.getByRole("img", { name: "A quiet library reading room" }),
  ).toBeVisible();

  await openStudy(page, "/?media=missing");
  await expect(page.getByText("Media file is missing")).toBeVisible();
  await expect(page.getByLabel("Your answer")).toBeEnabled();
});

test("accepts asset media and rejects remote or unsupported sources", async ({
  page,
}) => {
  await openStudy(page, "/?media=asset");
  await expect(page.locator("audio")).toHaveAttribute("src", /^asset:/);
  await expect(page.getByLabel("Your answer")).toBeEnabled();

  for (const media of ["remote-http", "remote-https", "unsupported"]) {
    await openStudy(page, `/?media=${media}`);
    await expect(page.locator("audio")).toHaveCount(0);
    await expect(page.getByText("Media format is unsupported")).toBeVisible();
    await expect(page.getByLabel("Your answer")).toBeEnabled();
  }
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

test("preserves multilingual and multi-code-point input in command requests", async ({
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
    await input.press("Enter");
    await expect(page.getByText("exact", { exact: true })).toBeVisible();
    expect((await lastRequest(page, "check_answer"))?.args).toMatchObject({
      request: { raw_response: response },
    });
  }
});

test("renders difference semantics without altering raw input", async ({
  page,
}) => {
  await openStudy(page, "/?answer=wrong");
  await page.getByLabel("Your answer").fill(" 図書館 ");
  await page.getByLabel("Your answer").press("Enter");

  const difference = page.getByTestId("answer-difference");
  await expect(difference.locator("del")).toHaveText("行きます");
  await expect(difference.locator("ins")).toHaveText("図書館");
  await expect(
    page.locator(".answer-comparison strong").nth(1),
  ).toHaveJSProperty("textContent", " 図書館 ");
});

test("maps keyboard grading and undo without browser persistence logic", async ({
  page,
}) => {
  await openStudy(page, "/");
  await page.keyboard.type("行きます");
  await page.keyboard.press("Enter");
  await page.keyboard.press("4");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();

  await page.keyboard.press("ControlOrMeta+z");
  await expect(page.getByText("Last review undone.")).toBeVisible();
  expect((await lastRequest(page, "undo_review"))?.args).toMatchObject({
    request: {
      card_id: "due-card",
      review_event_id: expect.any(String),
      undo_event_id: expect.any(String),
    },
  });
  await expect(page.getByLabel("Your answer")).toBeFocused();
});

test("offers UI retries for interrupted command responses", async ({
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

  await openStudy(page, "/?failure=grade");
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

test("replays a pending review request from a restart fixture", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem(
      "meiki-active-study-queue",
      JSON.stringify({
        version: 2,
        deckId: "__all_decks__",
        entries: [
          {
            card_id: "due-card",
            card_content_version: 0,
            schedule_version: 0,
          },
          {
            card_id: "new-card",
            card_content_version: 0,
            schedule_version: 0,
          },
        ],
        position: 0,
        startedAtMs: 1_700_000_000_000,
        pendingReview: {
          review_event_id: "pending-on-restart",
          card_id: "due-card",
          card_content_version: 0,
          schedule_version: 0,
          raw_response: "行きます",
          chosen_grade: "good",
          response_duration_ms: 1_000,
        },
      }),
    );
  });
  await page.goto("/?reconcile=second");
  await page.getByRole("button", { name: "Resume study" }).click();
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  expect((await lastRequest(page, "grade_review"))?.args).toMatchObject({
    request: { review_event_id: "pending-on-restart" },
  });
});

test("maps edit and suspend controls while retaining the reveal UI", async ({
  page,
}) => {
  await openStudy(page, "/");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("e");

  await expect(page.getByRole("heading", { name: "Add / Edit" })).toBeVisible();
  await page.getByRole("button", { name: "Return to study" }).click();
  await expect(page.getByText("Expected answer")).toBeVisible();

  await page.keyboard.press("s");
  await expect(
    page.getByRole("heading", { name: "Card suspended" }),
  ).toBeVisible();
  expect((await lastRequest(page, "suspend_card"))?.args).toMatchObject({
    request: { card_id: "due-card" },
  });
});
