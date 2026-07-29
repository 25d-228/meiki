import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const initialState = {
      scheduleVersion: 0,
      completedReviews: 0,
      dueAt: "2026-07-29T09:00:00+00:00",
    };
    const readState = () => {
      const value = localStorage.getItem("meiki-e2e-state");
      return value ? JSON.parse(value) : initialState;
    };

    window.__MEIKI_TEST_INVOKE__ = async (command, args) => {
      const state = readState();
      if (command === "initialize_collection" || command === "get_study_card") {
        return {
          card_id: "sample-card",
          card_content_version: 0,
          schedule_version: state.scheduleVersion,
          prompt: "日曜日は図書館に[…]",
          language_tag: "ja",
          direction: "auto",
          due_at: state.dueAt,
          completed_reviews: state.completedReviews,
        };
      }
      if (command === "check_answer") {
        const request = (args as { request: { raw_response: string } }).request;
        const exact =
          request.raw_response.trim().normalize("NFC") === "行きます";
        return {
          card_id: "sample-card",
          card_content_version: 0,
          schedule_version: state.scheduleVersion,
          full_source: "日曜日は図書館に行きます",
          expected_answer: "行きます",
          raw_response: request.raw_response,
          comparison: exact ? "exact" : "incorrect",
          suggested_grade: exact ? "good" : "again",
        };
      }
      if (command === "grade_review") {
        const nextState = {
          scheduleVersion: state.scheduleVersion + 1,
          completedReviews: state.completedReviews + 1,
          dueAt: "2026-08-01T09:00:00+00:00",
        };
        localStorage.setItem("meiki-e2e-state", JSON.stringify(nextState));
        return {
          review_event_id: "review-e2e",
          schedule_version: nextState.scheduleVersion,
          due_at: nextState.dueAt,
          interval_seconds: 259200,
        };
      }
      throw new Error(`Unexpected command: ${command}`);
    };
  });
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
