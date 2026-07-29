import type { Page } from "@playwright/test";

export async function installMockApi(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const initialState = {
      scheduleVersion: 0,
      completedReviews: 0,
      dueAt: "2026-07-29T09:00:00+00:00",
    };
    const fixtures = {
      cjk: {
        prompt: "日曜日は図書館に[…]",
        fullSource: "日曜日は図書館に行きます",
        answer: "行きます",
        languageTag: "ja",
        direction: "auto",
      },
      ltr: {
        prompt: "Le dimanche, je vais à […]",
        fullSource: "Le dimanche, je vais à la bibliothèque",
        answer: "la bibliothèque",
        languageTag: "fr",
        direction: "ltr",
      },
      rtl: {
        prompt: "من هر روز […] می‌خوانم",
        fullSource: "من هر روز کتاب می‌خوانم",
        answer: "کتاب",
        languageTag: "fa",
        direction: "rtl",
      },
      mixed: {
        prompt: "Meetingは الساعة […] に始まる",
        fullSource: "Meetingは الساعة 三時 に始まる",
        answer: "三時",
        languageTag: null,
        direction: "auto",
      },
    } as const;

    const readState = () => {
      const value = localStorage.getItem("meiki-e2e-state");
      return value ? JSON.parse(value) : initialState;
    };
    const selectedFixture = () => {
      const name = new URLSearchParams(location.search).get("fixture") ?? "cjk";
      return fixtures[name as keyof typeof fixtures] ?? fixtures.cjk;
    };

    window.__MEIKI_TEST_INVOKE__ = async (command, args) => {
      const state = readState();
      const fixtureName = new URLSearchParams(location.search).get("fixture");
      const fixture = selectedFixture();
      if (command === "initialize_collection" || command === "get_study_card") {
        if (fixtureName === "error") {
          throw new Error("The local collection is temporarily unavailable.");
        }
        if (fixtureName === "loading") {
          await new Promise((resolve) => setTimeout(resolve, 350));
        }
        return {
          card_id: "sample-card",
          card_content_version: 0,
          schedule_version: state.scheduleVersion,
          prompt: fixture.prompt,
          language_tag: fixture.languageTag,
          direction: fixture.direction,
          due_at: state.dueAt,
          completed_reviews: state.completedReviews,
        };
      }
      if (command === "check_answer") {
        const request = (args as { request: { raw_response: string } }).request;
        const exact =
          request.raw_response.trim().normalize("NFC") === fixture.answer;
        return {
          card_id: "sample-card",
          card_content_version: 0,
          schedule_version: state.scheduleVersion,
          full_source: fixture.fullSource,
          expected_answer: fixture.answer,
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
}
