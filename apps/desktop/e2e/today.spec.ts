import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

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
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-active-study-queue")),
  ).toBeNull();
});
