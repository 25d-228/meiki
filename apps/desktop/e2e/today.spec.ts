import { expect, test, type Locator } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

const minimumCardInsetPixels = 12;

type ElementBounds = NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;

async function visibleBounds(locator: Locator): Promise<ElementBounds> {
  const bounds = await locator.boundingBox();
  expect(bounds).not.toBeNull();
  return bounds!;
}

function expectHorizontalCardInset(
  card: ElementBounds,
  content: ElementBounds,
): void {
  expect(content.x - card.x).toBeGreaterThanOrEqual(minimumCardInsetPixels);
  expect(
    card.x + card.width - (content.x + content.width),
  ).toBeGreaterThanOrEqual(minimumCardInsetPixels);
  expect(content.y).toBeGreaterThanOrEqual(card.y);
  expect(content.y + content.height).toBeLessThanOrEqual(card.y + card.height);
}

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function openToday(page: import("@playwright/test").Page): Promise<void> {
  const heading = page.getByRole("heading", { name: "Today", level: 1 });
  if (!(await heading.isVisible())) {
    const openNavigation = page.getByRole("button", {
      name: "Open navigation",
    });
    if (await openNavigation.isVisible()) await openNavigation.click();
    await page.getByRole("button", { name: "Today", exact: true }).click();
  }
  await expect(heading).toBeVisible();
}

function statusMessage(page: import("@playwright/test").Page, text: string) {
  return page
    .getByTestId("app-shell")
    .getByRole("status")
    .filter({ hasText: text });
}

async function lastRequest(
  page: import("@playwright/test").Page,
  command: string,
) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

async function commandCount(
  page: import("@playwright/test").Page,
  command: string,
): Promise<number> {
  return page.evaluate(
    (name) =>
      (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
        (request) => request.command === name,
      ).length,
    command,
  );
}

async function releaseTodayRequest(
  page: import("@playwright/test").Page,
  kind: "overview" | "statistics",
): Promise<void> {
  await page.evaluate((requestKind) => {
    window.dispatchEvent(new Event(`meiki-e2e-release-today-${requestKind}`));
  }, kind);
}

async function installSavedQueue(
  page: import("@playwright/test").Page,
  deckId: string,
  cardIds = ["due-card", "new-card"],
  position = 0,
  pendingReview = false,
): Promise<void> {
  await page.addInitScript(
    ({ savedDeckId, savedCardIds, savedPosition, hasPendingReview }) => {
      const pendingCardId = savedCardIds[savedPosition];
      localStorage.setItem(
        "meiki-active-study-queue",
        JSON.stringify({
          version: 2,
          deckId: savedDeckId,
          entries: savedCardIds.map((cardId) => ({
            card_id: cardId,
            card_content_version: 0,
            schedule_version: 0,
          })),
          position: savedPosition,
          startedAtMs: 1_700_000_000_000,
          pendingReview:
            hasPendingReview && pendingCardId
              ? {
                  review_event_id: "pending-before-queue-switch",
                  card_id: pendingCardId,
                  card_content_version: 0,
                  schedule_version: 0,
                  raw_response: "行きます",
                  chosen_grade: "good",
                  response_duration_ms: 1_000,
                }
              : null,
        }),
      );
    },
    {
      savedDeckId: deckId,
      savedCardIds: cardIds,
      savedPosition: position,
      hasPendingReview: pendingReview,
    },
  );
}

test("shows empty, overdue, and capped workload states", async ({ page }) => {
  await page.goto("/?today=empty");
  await openToday(page);
  await expect(page.getByText("You’re caught up")).toBeVisible();
  await expect(page.getByText(/Next review:/)).toBeVisible();
  await expect(page.getByText("No active reviews yet.")).toBeVisible();
  await expect(page.getByText("No reviews", { exact: true })).toHaveCount(2);
  await expect(page.getByRole("button", { name: "Start study" })).toBeEnabled();

  await page.goto("/?today=overdue");
  await openToday(page);
  await expect(page.getByText("2 due and 1 new.")).toBeVisible();
  await expect(
    page.getByRole("status").getByText("1 overdue review"),
  ).toBeVisible();

  await page.goto("/?today=capped");
  await openToday(page);
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
  await expect(page.getByText("New-card intake capped")).toBeVisible();
  await expect(statusMessage(page, "2 new cards are deferred")).toBeVisible();
  await expect(page.getByRole("button", { name: "Start study" })).toBeEnabled();

  await page.goto("/?today=backlog");
  await openToday(page);
  await expect(page.getByText("Due work exceeds today’s budget")).toBeVisible();
  await expect(
    statusMessage(page, "Every due review remains available."),
  ).toBeVisible();
});

test("loads local statistics without runtime network requests", async ({
  page,
}) => {
  const runtimeRequests: string[] = [];
  page.on("request", (request) => {
    if (["fetch", "xhr"].includes(request.resourceType())) {
      runtimeRequests.push(request.url());
    }
  });
  await page.goto("/");
  await openToday(page);
  await expect(page.getByText("Cards learned today")).toBeVisible();
  expect(runtimeRequests).toEqual([]);
});

test("renders warm Today data immediately and refreshes it once in the background", async ({
  page,
}) => {
  await page.goto("/?todayWarm=controlled");
  await openToday(page);
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
  await expect(
    page
      .locator("[data-statistics-summary-card]")
      .filter({ hasText: "Cards learned today" }),
  ).toContainText("2");

  await page.getByRole("button", { name: "Decks", exact: true }).click();
  await page.getByRole("button", { name: "Today", exact: true }).click();

  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
  await expect(page.getByText("Planning today…")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Start study" })).toBeEnabled();
  await expect.poll(() => commandCount(page, "get_today_overview")).toBe(2);
  expect(await commandCount(page, "get_today_statistics")).toBe(1);

  await releaseTodayRequest(page, "overview");
  await expect(page.getByText("7 due and 4 new.")).toBeVisible();
  await expect(page.getByText("Loading statistics…")).toBeVisible();
  await expect(page.getByRole("button", { name: "Start study" })).toBeEnabled();
  await expect.poll(() => commandCount(page, "get_today_statistics")).toBe(2);
  await releaseTodayRequest(page, "statistics");

  await expect(
    page
      .locator("[data-statistics-summary-card]")
      .filter({ hasText: "Cards learned today" }),
  ).toContainText("17");
  expect(await commandCount(page, "get_today_overview")).toBe(2);
  expect(await commandCount(page, "get_today_statistics")).toBe(2);
});

test("keeps the bounded cold Today request sequence", async ({ page }) => {
  await page.goto("/?todayWarm=cold");
  await openToday(page);

  await expect(page.getByText("Planning today…")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Start study" }),
  ).toBeDisabled();
  await expect.poll(() => commandCount(page, "get_today_overview")).toBe(1);
  expect(await commandCount(page, "get_today_statistics")).toBe(0);

  await releaseTodayRequest(page, "overview");
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Start study" })).toBeEnabled();
  await expect.poll(() => commandCount(page, "get_today_statistics")).toBe(1);
  await releaseTodayRequest(page, "statistics");
  await expect(page.getByText("Cards learned today")).toBeVisible();
});

test("does not let an older warm response replace a newer deck selection", async ({
  page,
}) => {
  await page.goto("/?todayWarm=stale");
  await openToday(page);
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();

  await page.getByRole("button", { name: "Decks", exact: true }).click();
  await page.getByRole("button", { name: "Today", exact: true }).click();
  await expect.poll(() => commandCount(page, "get_today_overview")).toBe(2);
  await page.getByLabel("Deck").selectOption("travel-deck");
  await expect(page.getByText("Travel phrases", { exact: true })).toBeVisible();

  await releaseTodayRequest(page, "overview");
  await expect(page.getByLabel("Deck")).toHaveValue("travel-deck");
  await expect(page.getByText("Travel phrases", { exact: true })).toBeVisible();
  expect((await lastRequest(page, "get_today_statistics"))?.args).toMatchObject(
    {
      request: { deck_id: "travel-deck" },
    },
  );
});

test("never reuses another deck scope while a focused load is delayed", async ({
  page,
}) => {
  await page.goto("/?todayWarm=scope");
  await openToday(page);
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
  await expect(page.getByText("Cards learned today")).toBeVisible();

  await page.getByLabel("Deck").selectOption("travel-deck");
  await expect(page.getByText("Planning today…")).toBeVisible();
  await expect(page.getByText("Cards learned today")).toHaveCount(0);
  await expect.poll(() => commandCount(page, "get_today_overview")).toBe(2);
  await releaseTodayRequest(page, "overview");

  await expect(page.getByText("Travel phrases", { exact: true })).toBeVisible();
  await expect(
    page
      .locator("[data-statistics-summary-card]")
      .filter({ hasText: "Cards learned today" }),
  ).toContainText("1");
});

test("preserves warm results and offers one retry after background failure", async ({
  page,
}) => {
  await page.goto("/?todayWarm=failure");
  await openToday(page);
  await expect(page.getByText("Cards learned today")).toBeVisible();

  await page.getByRole("button", { name: "Decks", exact: true }).click();
  await page.getByRole("button", { name: "Today", exact: true }).click();

  const alert = page.getByRole("alert");
  await expect(alert).toContainText("Today could not be refreshed");
  await expect(alert).toContainText("Showing the last successful results.");
  await expect(alert).not.toContainText("database");
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
  await expect(page.getByText("Cards learned today")).toBeVisible();
  await alert.getByRole("button", { name: "Try again" }).click();
  await expect(alert).toHaveCount(0);
  await expect.poll(() => commandCount(page, "get_today_overview")).toBe(3);
});

test("keeps warm charts when their background refresh fails", async ({
  page,
}) => {
  await page.goto("/?todayWarm=failure-statistics");
  await openToday(page);
  const learnedCards = page
    .locator("[data-statistics-summary-card]")
    .filter({ hasText: "Cards learned today" });
  await expect(learnedCards).toContainText("2");

  await page.getByRole("button", { name: "Decks", exact: true }).click();
  await page.getByRole("button", { name: "Today", exact: true }).click();

  const alert = page.getByRole("alert");
  await expect(alert).toContainText("Today could not be refreshed");
  await expect(alert).not.toContainText("database");
  await expect(learnedCards).toContainText("2");
  await expect(page.getByText("Review statistics are unavailable")).toHaveCount(
    0,
  );
});

test("does not reuse warm Today data after the local study day changes", async ({
  page,
}) => {
  await page.clock.install({ time: new Date("2026-08-29T03:59:00") });
  await page.goto("/?todayWarm=study-day");
  await openToday(page);
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();

  await page.getByRole("button", { name: "Decks", exact: true }).click();
  await page.clock.setFixedTime(new Date("2026-08-29T04:01:00"));
  await page.getByRole("button", { name: "Today", exact: true }).click();

  await expect(page.getByText("Planning today…")).toBeVisible();
  await expect(page.getByText("Cards learned today")).toHaveCount(0);
  await expect.poll(() => commandCount(page, "get_today_overview")).toBe(2);
  await releaseTodayRequest(page, "overview");
  await expect(page.getByText("1 due and 1 new.")).toBeVisible();
});

test("shows scoped summaries and accessible bounded activity charts", async ({
  page,
}) => {
  await page.goto("/?today=normal");
  await openToday(page);

  await expect(page.getByText("Cards learned today")).toBeVisible();
  await expect(page.getByText("Reviews today")).toBeVisible();
  await expect(page.getByText("60%")).toBeVisible();
  await expect(page.getByText("40%")).toBeVisible();
  await expect(page.getByText("3 days")).toBeVisible();
  const activity = page.getByRole("img", {
    name: /Daily review activity from/,
  });
  const accuracy = page.getByRole("img", {
    name: /Daily correct and error reviews from/,
  });
  await expect(activity).toBeVisible();
  await expect(accuracy).toBeVisible();
  await expect(activity.locator("rect")).toHaveCount(364);
  await expect(accuracy.locator("rect.correct-bar")).toHaveCount(30);
  await expect(accuracy.locator("rect.error-bar")).toHaveCount(30);
  expect(await page.locator(".statistics [tabindex]").count()).toBe(0);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);

  await page.getByLabel("Deck").selectOption("travel-deck");
  await expect(
    page.locator(".statistics-summary").getByText("2", { exact: true }),
  ).toBeVisible();
  expect((await lastRequest(page, "get_today_statistics"))?.args).toMatchObject(
    {
      request: {
        deck_id: "travel-deck",
        day_boundary_minutes: 240,
      },
    },
  );
});

for (const layout of [
  { name: "desktop", width: 1_440, height: 900 },
  { name: "narrow", width: 640, height: 720 },
] as const) {
  test(`keeps every statistics card inset in the ${layout.name} layout`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: layout.width, height: layout.height });
    await page.goto("/?today=normal");
    await openToday(page);

    const summaryCards = page.locator("[data-statistics-summary-card]");
    await expect(summaryCards).toHaveCount(5);
    for (let index = 0; index < 5; index += 1) {
      const card = summaryCards.nth(index);
      const cardBounds = await visibleBounds(card);
      const labelBounds = await visibleBounds(
        card.locator("[data-statistics-summary-label]"),
      );
      const valueBounds = await visibleBounds(
        card.locator("[data-statistics-summary-value]"),
      );
      expectHorizontalCardInset(cardBounds, labelBounds);
      expectHorizontalCardInset(cardBounds, valueBounds);
      expect(labelBounds.y - cardBounds.y).toBeGreaterThanOrEqual(
        minimumCardInsetPixels,
      );
      expect(
        cardBounds.y + cardBounds.height - (valueBounds.y + valueBounds.height),
      ).toBeGreaterThanOrEqual(minimumCardInsetPixels);
    }

    const chartCards = page.locator("[data-statistics-chart-card]");
    await expect(chartCards).toHaveCount(2);
    for (const chart of [
      {
        index: 0,
        heading: "Review activity",
        image: /Daily review activity from/,
        legend: "Activity intensity legend",
      },
      {
        index: 1,
        heading: "Correct and error reviews",
        image: /Daily correct and error reviews from/,
        legend: "Review result legend",
      },
    ] as const) {
      const card = chartCards.nth(chart.index);
      const cardBounds = await visibleBounds(card);
      const headingBounds = await visibleBounds(
        card.getByRole("heading", { name: chart.heading }),
      );
      const chartBounds = await visibleBounds(
        card.getByRole("img", { name: chart.image }),
      );
      const legendBounds = await visibleBounds(
        card.getByRole("group", { name: chart.legend }),
      );
      expectHorizontalCardInset(cardBounds, headingBounds);
      expectHorizontalCardInset(cardBounds, chartBounds);
      expectHorizontalCardInset(cardBounds, legendBounds);
      expect(headingBounds.y - cardBounds.y).toBeGreaterThanOrEqual(
        minimumCardInsetPixels,
      );
      expect(
        cardBounds.y +
          cardBounds.height -
          (legendBounds.y + legendBounds.height),
      ).toBeGreaterThanOrEqual(minimumCardInsetPixels);
    }

    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);

    if (layout.name === "narrow") {
      await page.goto("/?today=empty");
      await openToday(page);
      const noReviewCards = page
        .locator("[data-statistics-summary-card]")
        .filter({ hasText: "No reviews" });
      await expect(noReviewCards).toHaveCount(2);
      for (let index = 0; index < 2; index += 1) {
        const card = noReviewCards.nth(index);
        const value = card.locator("[data-statistics-summary-value]");
        expectHorizontalCardInset(
          await visibleBounds(card),
          await visibleBounds(value),
        );
        expect(
          await value.evaluate(
            (element) => element.scrollWidth <= element.clientWidth,
          ),
        ).toBe(true);
      }
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
    }
  });
}

test("keeps study available when statistics fail and retries only statistics", async ({
  page,
}) => {
  await page.goto("/?failure=statistics");
  await openToday(page);

  await expect(page.getByText("Ready when you are")).toBeVisible();
  await expect(page.getByRole("button", { name: "Start study" })).toBeEnabled();
  const alert = page.getByRole("alert");
  await expect(alert).toContainText("Review statistics are unavailable");
  await expect(alert).not.toContainText("database");
  await alert.getByRole("button", { name: "Try statistics again" }).click();
  await expect(page.getByText("Cards learned today")).toBeVisible();
  await expect(page.getByText("Review statistics are unavailable")).toHaveCount(
    0,
  );
  expect(
    await page.evaluate(
      () =>
        (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
          (request) => request.command === "get_today_overview",
        ).length,
    ),
  ).toBe(1);
});

test("an empty selected deck does not fall back to all-decks statistics", async ({
  page,
}) => {
  await page.goto("/?statistics=focused-empty");
  await openToday(page);
  await expect(page.getByText("5", { exact: true })).toBeVisible();

  await page.getByLabel("Deck").selectOption("travel-deck");
  await expect(page.getByText("No active reviews yet.")).toBeVisible();
  await expect(page.getByText("No reviews", { exact: true })).toHaveCount(2);
  await expect(
    page.locator(".statistics-summary").getByText("5", { exact: true }),
  ).toHaveCount(0);
});

test("maps deck and time-budget controls to command requests", async ({
  page,
}) => {
  await page.goto("/?today=budget");
  await openToday(page);
  await expect(page.getByText("1 due and 3 new.")).toBeVisible();

  await page.getByLabel("Deck").selectOption("travel-deck");
  expect((await lastRequest(page, "get_today_overview"))?.args).toMatchObject({
    request: { deck_id: "travel-deck" },
  });
  expect((await lastRequest(page, "get_today_statistics"))?.args).toMatchObject(
    {
      request: { deck_id: "travel-deck" },
    },
  );

  await page
    .locator("#main-content")
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await page.getByLabel("Daily study hours").fill("0");
  await page.getByLabel("Daily study minutes").fill("1");
  await page.getByRole("button", { name: "Preview policy" }).click();
  await page.getByRole("button", { name: "Save preferences" }).click();
  await expect(page.getByText("Scheduling preferences saved.")).toBeVisible();
  expect(
    (await lastRequest(page, "update_scheduler_settings"))?.args,
  ).toMatchObject({
    request: {
      collection_daily_time_budget_minutes: 1,
      deck_id: "default-deck",
    },
  });
});

test("replaces an all-decks queue with a focused queue and discards only its local response", async ({
  page,
}) => {
  await installSavedQueue(page, "__all_decks__");
  await page.addInitScript(() => {
    sessionStorage.setItem(
      "meiki-active-study-session",
      JSON.stringify({ response: "unfinished local response" }),
    );
  });
  await page.goto("/?today=normal");
  await openToday(page);

  const selector = page.getByLabel("Deck");
  await expect(selector).toBeEnabled();
  await expect(
    page.getByRole("button", { name: "Resume study" }),
  ).toBeEnabled();
  await selector.selectOption("travel-deck");
  await expect(page.getByText("Resume where you stopped")).toHaveCount(0);
  await page.getByRole("button", { name: "Start study" }).click();

  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({ deckId: "travel-deck", position: 0, pendingReview: null });
  expect(
    await page.evaluate(() =>
      sessionStorage.getItem("meiki-active-study-session"),
    ),
  ).toBeNull();
  expect(
    await page.evaluate(() =>
      localStorage.getItem("meiki-e2e-committed-reviews"),
    ),
  ).toBeNull();
});

test("replaces a focused queue with a new all-decks queue", async ({
  page,
}) => {
  await installSavedQueue(page, "travel-deck");
  await page.goto("/?today=normal");
  await openToday(page);

  const selector = page.getByLabel("Deck");
  await expect(selector).toBeEnabled();
  await expect(selector).toHaveValue("__all_decks__");
  await expect(page.getByText("Resume where you stopped")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Start study" })).toBeEnabled();
  await page.getByRole("button", { name: "Start study" }).click();

  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({ deckId: "__all_decks__", position: 0 });
});

test("completes one pending review before replacing its queue", async ({
  page,
}) => {
  await installSavedQueue(
    page,
    "__all_decks__",
    ["due-card", "new-card"],
    0,
    true,
  );
  await page.goto("/?today=normal");
  await openToday(page);
  await page.getByLabel("Deck").selectOption("travel-deck");
  await page.getByRole("button", { name: "Start study" }).click();

  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
          (request) => request.command === "grade_review",
        ).length,
    ),
  ).toBe(1);
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-e2e-committed-reviews") ?? "[]"),
    ),
  ).toEqual([
    expect.objectContaining({
      review_event_id: "pending-before-queue-switch",
      schedule_version: 1,
    }),
  ]);
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({ deckId: "travel-deck", position: 0, pendingReview: null });
});

test("preserves the old queue on a mismatched pending-review response and retries idempotently", async ({
  page,
}) => {
  await installSavedQueue(
    page,
    "__all_decks__",
    ["due-card", "new-card"],
    0,
    true,
  );
  await page.addInitScript(() => {
    sessionStorage.setItem("meiki-active-study-session", "saved card session");
  });
  await page.goto("/?today=normal&failure=queue-switch-mismatch");
  await openToday(page);
  await page.getByLabel("Deck").selectOption("travel-deck");
  await page.getByRole("button", { name: "Start study" }).click();

  const alert = page.getByRole("alert");
  await expect(alert).toContainText("The saved review could not be completed");
  await expect(alert).toContainText(
    "The recovered review command did not match.",
  );
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({
    deckId: "__all_decks__",
    position: 0,
    pendingReview: { review_event_id: "pending-before-queue-switch" },
  });
  expect(
    await page.evaluate(() =>
      sessionStorage.getItem("meiki-active-study-session"),
    ),
  ).toBe("saved card session");
  await alert.getByRole("button", { name: "Try again" }).click();

  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => {
      const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
      return requests
        .filter((request) => request.command === "grade_review")
        .map(
          (request) =>
            (request.args as { request: { review_event_id: string } }).request
              .review_event_id,
        );
    }),
  ).toEqual(["pending-before-queue-switch", "pending-before-queue-switch"]);
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-e2e-committed-reviews") ?? "[]"),
    ),
  ).toEqual([
    expect.objectContaining({
      review_event_id: "pending-before-queue-switch",
      card_id: "due-card",
      schedule_version: 1,
    }),
  ]);
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({ deckId: "travel-deck", position: 0, pendingReview: null });
});

test("recovers the selected Today deck after its bundle is removed and the app reloads", async ({
  page,
}) => {
  const removedDeckId = "deck:ja-JP:05";
  await page.goto("/?bundleRemoval=installed");
  await openToday(page);
  await page.getByLabel("Deck").selectOption(removedDeckId);
  await expect(page.getByLabel("Deck")).toHaveValue(removedDeckId);

  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Decks", exact: true })
    .click();
  await page.getByRole("button", { name: "Bundle actions" }).click();
  await page
    .getByRole("dialog", { name: "Bundle actions" })
    .getByRole("button", { name: /Remove Japanese/ })
    .click();
  await page
    .getByRole("alertdialog", { name: "Remove Japanese?" })
    .getByRole("button", { name: "Remove bundle" })
    .click();
  await expect(page.getByTestId("deletion-activity")).toContainText(
    "Removed Japanese with 6 decks.",
  );
  await page
    .getByRole("dialog", { name: "Bundle removed" })
    .getByRole("button", { name: "Close" })
    .last()
    .click();

  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Today", exact: true })
    .click();
  await expect(page.getByLabel("Deck")).toHaveValue("__all_decks__");
  await expect(page.getByRole("alert")).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-today-deck")),
  ).toBe("__all_decks__");
  expect(
    await page.evaluate((deckId) => {
      const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
      const removalIndex = requests
        .map((request) => request.command)
        .lastIndexOf("remove_bundle");
      return requests
        .slice(removalIndex + 1)
        .some(
          (request) =>
            request.command === "get_scheduler_settings" &&
            (request.args as { deckId?: string }).deckId === deckId,
        );
    }, removedDeckId),
  ).toBe(false);

  await page.reload();
  await openToday(page);
  await expect(page.getByLabel("Deck")).toHaveValue("__all_decks__");
  await expect(page.getByRole("alert")).toHaveCount(0);
  expect(
    await page.evaluate(
      (deckId) =>
        (window.__MEIKI_TEST_REQUESTS__ ?? []).some(
          (request) =>
            request.command === "get_scheduler_settings" &&
            (request.args as { deckId?: string }).deckId === deckId,
        ),
      removedDeckId,
    ),
  ).toBe(false);
});

test("recovers the selected Today deck after an individual deck is deleted", async ({
  page,
}) => {
  const removedDeckId = "travel-deck";
  await page.goto("/?deckDeletion=focused-session");
  await openToday(page);
  await page.getByLabel("Deck").selectOption(removedDeckId);
  await expect(page.getByLabel("Deck")).toHaveValue(removedDeckId);

  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Decks", exact: true })
    .click();
  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Open" })
    .click();
  await page.getByRole("button", { name: "Delete deck" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
    .getByRole("button", { name: "Delete deck" })
    .click();
  await expect(page.getByTestId("deletion-activity")).toContainText(
    "Deleted Travel phrases.",
  );
  await page
    .getByRole("dialog", { name: "Deck deleted" })
    .getByRole("button", { name: "Close" })
    .last()
    .click();
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Today", exact: true })
    .click();

  await expect(page.getByLabel("Deck")).toHaveValue("__all_decks__");
  await expect(page.getByRole("alert")).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-today-deck")),
  ).toBe("__all_decks__");
  expect(
    await page.evaluate((deckId) => {
      const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
      const deletionIndex = requests
        .map((request) => request.command)
        .lastIndexOf("delete_deck");
      return requests
        .slice(deletionIndex + 1)
        .some(
          (request) =>
            request.command === "get_scheduler_settings" &&
            (request.args as { deckId?: string }).deckId === deckId,
        );
    }, removedDeckId),
  ).toBe(false);
});

test("clears a focused saved queue and per-card session for a removed deck", async ({
  page,
}) => {
  await installSavedQueue(page, "deck:ja-JP:05");
  await page.addInitScript(() => {
    sessionStorage.setItem("meiki-active-study-session", "stale session");
  });
  await page.goto("/?bundleRemoval=removed");
  await openToday(page);

  await expect(page.getByLabel("Deck")).toHaveValue("__all_decks__");
  await expect(page.getByRole("alert")).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-active-study-queue")),
  ).toBeNull();
  expect(
    await page.evaluate(() =>
      sessionStorage.getItem("meiki-active-study-session"),
    ),
  ).toBeNull();
});

test("preserves an all-decks queue and reconciles it after bundle removal", async ({
  page,
}) => {
  await installSavedQueue(page, "__all_decks__");
  await page.addInitScript(() => {
    sessionStorage.setItem(
      "meiki-active-study-session",
      JSON.stringify({
        card: {
          card_id: "new-card",
          card_content_version: 0,
          schedule_version: 0,
        },
        reveal: null,
        result: null,
        response: "",
        view: "prompt",
        responseDurationMs: 0,
        completionKind: null,
      }),
    );
  });
  await page.goto("/?bundleRemoval=removed&reconcile=second");
  await openToday(page);

  await expect(page.getByText("Resume where you stopped")).toBeVisible();
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({ deckId: "__all_decks__", position: 0 });
  expect(
    await page.evaluate(() =>
      sessionStorage.getItem("meiki-active-study-session"),
    ),
  ).not.toBeNull();

  await page.getByRole("button", { name: "Resume study" }).click();
  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
  await expect(page.getByLabel("Your answer")).toBeVisible();
  expect(
    (await lastRequest(page, "reconcile_study_queue"))?.args,
  ).toMatchObject({
    request: { deck_id: "__all_decks__" },
  });
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({
    deckId: "__all_decks__",
    entries: [{ card_id: "new-card" }],
  });
});

test("keeps the retry alert for a genuine Today loading failure", async ({
  page,
}) => {
  await page.goto("/?failure=today");
  await openToday(page);

  const alert = page.getByRole("alert");
  await expect(alert).toContainText("Today’s queue could not be planned");
  await expect(alert).toContainText(
    "The local collection is temporarily unavailable.",
  );
  await expect(alert.getByRole("button", { name: "Try again" })).toBeVisible();
});

test("renders and continues a persisted queue fixture", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await installSavedQueue(page, "__all_decks__", ["due-card", "new-card"], 1);
  await page.goto("/?today=normal&fixture=longmixed&reconcile=second");
  await openToday(page);
  await expect(page.getByText("Resume where you stopped")).toBeVisible();
  await page.getByRole("button", { name: "Resume study" }).click();
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByText("1 card remaining")).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    ),
  ).toBe(false);

  await page.getByLabel("Your answer").fill("三時");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Session complete" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Return to Today" }).click();

  await expect(page.getByTestId("app-announcement")).toHaveText(
    "Study queue complete. Returning to Today.",
  );
  await expect(
    page.getByRole("heading", { name: "Today", level: 1 }),
  ).toBeVisible();
  await expect(page.getByText("Review statistics")).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
          (request) => request.command === "get_today_statistics",
        ).length,
    ),
  ).toBeGreaterThanOrEqual(2);
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-active-study-queue")),
  ).toBeNull();
});
