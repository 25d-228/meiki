import { expect, test, type Browser, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

type CalendarCase = {
  name: string;
  timezoneId: string;
  now: string;
  boundary: "midnight" | "four";
  start: string;
  end: string;
};

const calendarCases: CalendarCase[] = [
  {
    name: "UTC",
    timezoneId: "UTC",
    now: "2025-01-15T12:00:00.000Z",
    boundary: "four",
    start: "2025-01-15T04:00:00.000Z",
    end: "2025-01-16T04:00:00.000Z",
  },
  {
    name: "Asia/Tokyo",
    timezoneId: "Asia/Tokyo",
    now: "2025-01-15T03:00:00.000Z",
    boundary: "four",
    start: "2025-01-14T19:00:00.000Z",
    end: "2025-01-15T19:00:00.000Z",
  },
  {
    name: "New York spring-forward",
    timezoneId: "America/New_York",
    now: "2025-03-09T16:00:00.000Z",
    boundary: "midnight",
    start: "2025-03-09T05:00:00.000Z",
    end: "2025-03-10T04:00:00.000Z",
  },
  {
    name: "New York fall-back",
    timezoneId: "America/New_York",
    now: "2025-11-02T17:00:00.000Z",
    boundary: "midnight",
    start: "2025-11-02T04:00:00.000Z",
    end: "2025-11-03T05:00:00.000Z",
  },
  {
    name: "India half-hour offset",
    timezoneId: "Asia/Kolkata",
    now: "2025-01-15T06:30:00.000Z",
    boundary: "midnight",
    start: "2025-01-14T18:30:00.000Z",
    end: "2025-01-15T18:30:00.000Z",
  },
  {
    name: "Nepal 45-minute offset",
    timezoneId: "Asia/Kathmandu",
    now: "2025-01-15T06:15:00.000Z",
    boundary: "midnight",
    start: "2025-01-14T18:15:00.000Z",
    end: "2025-01-15T18:15:00.000Z",
  },
];

async function lastRequest(page: Page, command: string) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

async function openFixedCalendar(
  browser: Browser,
  fixture: CalendarCase,
): Promise<{ page: Page; close: () => Promise<void> }> {
  const context = await browser.newContext({ timezoneId: fixture.timezoneId });
  const page = await context.newPage();
  await installMockApi(page);
  await page.clock.setFixedTime(new Date(fixture.now));
  const query =
    fixture.boundary === "midnight" ? "?boundary=midnight" : "?boundary=four";
  await page.goto(`/${query}`);
  await expect(
    page.getByRole("heading", { name: "Today", level: 1 }),
  ).toBeVisible();
  return { page, close: () => context.close() };
}

for (const fixture of calendarCases) {
  test(`maps the fixed local calendar boundary: ${fixture.name}`, async ({
    browser,
  }) => {
    const opened = await openFixedCalendar(browser, fixture);
    try {
      const request = await lastRequest(opened.page, "get_today_overview");
      expect(request?.args).toMatchObject({
        request: {
          now_ms: Date.parse(fixture.now),
          day_start_ms: Date.parse(fixture.start),
          day_end_ms: Date.parse(fixture.end),
        },
      });
      const statisticsRequest = await lastRequest(
        opened.page,
        "get_today_statistics",
      );
      expect(statisticsRequest?.args).toMatchObject({
        request: {
          now_ms: Date.parse(fixture.now),
          day_start_ms: Date.parse(fixture.start),
          day_end_ms: Date.parse(fixture.end),
          day_boundary_minutes: fixture.boundary === "midnight" ? 0 : 240,
        },
      });
    } finally {
      await opened.close();
    }
  });
}

test("an open queue reconciles against the new local day after midnight", async ({
  browser,
}) => {
  const context = await browser.newContext({ timezoneId: "Asia/Tokyo" });
  const page = await context.newPage();
  await installMockApi(page);
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
        ],
        position: 0,
        startedAtMs: 1_700_000_000_000,
        pendingReview: null,
      }),
    );
  });
  await page.clock.setFixedTime(new Date("2025-01-14T18:59:59.999Z"));
  await page.goto("/");
  await expect(page.getByText("Resume where you stopped")).toBeVisible();

  await page.clock.setFixedTime(new Date("2025-01-15T19:00:00.001Z"));
  await page.getByRole("button", { name: "Resume study" }).click();
  await expect(page.getByLabel("Your answer")).toBeVisible();
  const request = await lastRequest(page, "reconcile_study_queue");
  expect(request?.args).toMatchObject({
    request: {
      now_ms: Date.parse("2025-01-15T19:00:00.001Z"),
      day_start_ms: Date.parse("2025-01-15T19:00:00.000Z"),
      day_end_ms: Date.parse("2025-01-16T19:00:00.000Z"),
    },
  });
  await context.close();
});
