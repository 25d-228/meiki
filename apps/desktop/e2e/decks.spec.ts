import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

const minimumActionGapPixels = 8;

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
});

async function openDecks(page: import("@playwright/test").Page): Promise<void> {
  const openNavigation = page.getByRole("button", {
    name: "Open navigation",
  });
  if (await openNavigation.isVisible()) await openNavigation.click();
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Decks", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Decks", level: 1 }),
  ).toBeVisible();
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

async function openDeckDeleteAction(
  page: import("@playwright/test").Page,
  deckId: string,
  deckName: string,
): Promise<void> {
  await page
    .getByTestId(`deck-${deckId}`)
    .getByRole("button", { name: `Actions for ${deckName}` })
    .click();
  await page.getByRole("menuitem", { name: "Delete deck" }).click();
}

async function openDeckResetAction(
  page: import("@playwright/test").Page,
  deckId: string,
  deckName: string,
): Promise<void> {
  await page
    .getByTestId(`deck-${deckId}`)
    .getByRole("button", { name: `Actions for ${deckName}` })
    .click();
  await page.getByRole("menuitem", { name: "Reset progress" }).click();
}

async function seedStudyState(
  page: import("@playwright/test").Page,
  queueDeckId: string,
  todayDeckId: string,
): Promise<void> {
  await page.evaluate(
    ({ deckId, selectedTodayDeckId }) => {
      localStorage.setItem(
        "meiki-active-study-queue",
        JSON.stringify({
          version: 2,
          deckId,
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
      sessionStorage.setItem(
        "meiki-active-study-session",
        `session for ${deckId}`,
      );
      localStorage.setItem("meiki-today-deck", selectedTodayDeckId);
    },
    { deckId: queueDeckId, selectedTodayDeckId: todayDeckId },
  );
}

async function deleteDeckRequestCount(
  page: import("@playwright/test").Page,
): Promise<number> {
  return page.evaluate(
    () =>
      (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
        (request) => request.command === "delete_deck",
      ).length,
  );
}

async function batchDeleteRequestCount(
  page: import("@playwright/test").Page,
): Promise<number> {
  return page.evaluate(
    () =>
      (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
        (request) => request.command === "delete_decks",
      ).length,
  );
}

async function resetDeckProgressRequestCount(
  page: import("@playwright/test").Page,
): Promise<number> {
  return page.evaluate(
    () =>
      (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
        (request) => request.command === "reset_deck_progress",
      ).length,
  );
}

async function expectDeckSelectionControls(
  page: import("@playwright/test").Page,
  expectedCount = 5,
): Promise<void> {
  await expect(
    page.getByRole("button", { name: "Select", exact: true }),
  ).toHaveCount(0);
  await expect(page.getByRole("checkbox")).toHaveCount(expectedCount);
}

async function selectDeck(
  page: import("@playwright/test").Page,
  deckName: string,
): Promise<void> {
  await page.getByRole("checkbox", { name: `Select ${deckName}` }).click();
}

type Point = { x: number; y: number };
type PointerModifier = "Shift" | "Control" | "Meta";

async function clickBelowPointerThreshold(
  page: import("@playwright/test").Page,
  point: Point,
): Promise<void> {
  await page.mouse.move(point.x, point.y);
  await page.mouse.down();
  await page.mouse.move(point.x + 2, point.y + 2);
  await page.mouse.up();
}

async function clickDeckBackground(
  page: import("@playwright/test").Page,
  deckId: string,
): Promise<void> {
  const bounds = await page.getByTestId(`deck-${deckId}`).boundingBox();
  if (!bounds) throw new Error("Deck background geometry is unavailable");
  await clickBelowPointerThreshold(page, {
    x: bounds.x + 4,
    y: bounds.y + 4,
  });
}

async function doubleClickDeckBackground(
  page: import("@playwright/test").Page,
  deckId: string,
): Promise<void> {
  const deck = page.getByTestId(`deck-${deckId}`);
  await deck.scrollIntoViewIfNeeded();
  const bounds = await deck.boundingBox();
  if (!bounds) throw new Error("Deck background geometry is unavailable");
  await page.mouse.dblclick(bounds.x + 4, bounds.y + 4);
}

async function beginDeckBackgroundPress(
  page: import("@playwright/test").Page,
  deckId: string,
): Promise<number> {
  const area = page.getByTestId("deck-selection-area");
  await area.evaluate((element) => {
    element.addEventListener(
      "pointerdown",
      (event) => {
        element.setAttribute("data-test-pointer-id", String(event.pointerId));
      },
      { once: true },
    );
  });
  const bounds = await page.getByTestId(`deck-${deckId}`).boundingBox();
  if (!bounds) throw new Error("Deck background geometry is unavailable");
  await page.mouse.move(bounds.x + 4, bounds.y + 4);
  await page.mouse.down();
  const pointerId = await area.getAttribute("data-test-pointer-id");
  if (!pointerId) throw new Error("Deck background pointer is unavailable");
  return Number(pointerId);
}

async function partialDeckDragPoints(
  page: import("@playwright/test").Page,
  deckId: string,
): Promise<{ start: Point; end: Point }> {
  const area = await page.getByTestId("deck-selection-area").boundingBox();
  const deck = await page.getByTestId(`deck-${deckId}`).boundingBox();
  if (!area || !deck) throw new Error("Deck selection geometry is unavailable");
  const gridVisible = await page.getByTestId("deck-grid").isVisible();
  if (gridVisible) {
    const rightGap = area.x + area.width - (deck.x + deck.width);
    if (rightGap >= 10) {
      return {
        start: { x: deck.x + deck.width + 8, y: deck.y + deck.height / 2 },
        end: { x: deck.x + deck.width - 4, y: deck.y + deck.height / 2 },
      };
    }
    return {
      start: { x: deck.x - 8, y: deck.y + deck.height / 2 },
      end: { x: deck.x + 4, y: deck.y + deck.height / 2 },
    };
  }
  const topGap = deck.y - area.y;
  if (topGap >= 8) {
    return {
      start: { x: deck.x + 8, y: deck.y - 4 },
      end: { x: deck.x + 8, y: deck.y + 4 },
    };
  }
  return {
    start: { x: deck.x + 8, y: deck.y + deck.height + 4 },
    end: { x: deck.x + 8, y: deck.y + deck.height - 4 },
  };
}

async function dragAcrossDeckEdge(
  page: import("@playwright/test").Page,
  deckId: string,
  modifier?: PointerModifier,
  releaseModifierAfterPointerDown = false,
): Promise<void> {
  const { start, end } = await partialDeckDragPoints(page, deckId);
  if (modifier) await page.keyboard.down(modifier);
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  if (modifier && releaseModifierAfterPointerDown) {
    await page.keyboard.up(modifier);
  }
  await page.mouse.move(end.x, end.y, { steps: 4 });
  await page.mouse.up();
  if (modifier && !releaseModifierAfterPointerDown) {
    await page.keyboard.up(modifier);
  }
}

async function beginPackagedCompatibleDrag(
  page: import("@playwright/test").Page,
  deckId: string,
): Promise<number> {
  const { start, end } = await partialDeckDragPoints(page, deckId);
  return page.evaluate(
    ({ startPoint, endPoint }) => {
      const area = document.querySelector<HTMLElement>(
        '[data-testid="deck-selection-area"]',
      );
      const origin = document.elementFromPoint(startPoint.x, startPoint.y);
      if (!area || !origin || !area.contains(origin)) {
        throw new Error("Packaged-compatible drag origin is unavailable");
      }
      const pointerId = 47;
      // WKWebView can omit mouse-specific PointerEvent fields for a valid left-button drag.
      origin.dispatchEvent(
        new PointerEvent("pointerdown", {
          bubbles: true,
          cancelable: true,
          button: 0,
          buttons: 1,
          clientX: startPoint.x,
          clientY: startPoint.y,
          pointerId,
        }),
      );
      window.dispatchEvent(
        new PointerEvent("pointermove", {
          bubbles: true,
          cancelable: true,
          button: -1,
          buttons: 1,
          clientX: endPoint.x,
          clientY: endPoint.y,
          pointerId,
        }),
      );
      return pointerId;
    },
    { startPoint: start, endPoint: end },
  );
}

async function finishPackagedCompatibleDrag(
  page: import("@playwright/test").Page,
  pointerId: number,
): Promise<void> {
  await page.evaluate((activePointerId) => {
    window.dispatchEvent(
      new PointerEvent("pointerup", {
        bubbles: true,
        cancelable: true,
        button: 0,
        buttons: 0,
        pointerId: activePointerId,
      }),
    );
  }, pointerId);
}

async function dragFromElement(
  page: import("@playwright/test").Page,
  locator: import("@playwright/test").Locator,
): Promise<void> {
  const origin = await locator.boundingBox();
  const target = await page.getByTestId("deck-listening-deck").boundingBox();
  if (!origin || !target)
    throw new Error("Interactive drag geometry is unavailable");
  await page.mouse.move(
    origin.x + origin.width / 2,
    origin.y + origin.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(target.x + 8, target.y + target.height / 2, {
    steps: 4,
  });
  await page.mouse.up();
}

async function setPointerPlatform(
  page: import("@playwright/test").Page,
  platform: string,
): Promise<void> {
  await page.addInitScript((runtimePlatform) => {
    Object.defineProperty(navigator, "platform", {
      configurable: true,
      get: () => runtimePlatform,
    });
    Object.defineProperty(navigator, "userAgentData", {
      configurable: true,
      get: () => ({ platform: runtimePlatform }),
    });
  }, platform);
}

async function startVerticalEdgeSelection(
  page: import("@playwright/test").Page,
  edge: "top" | "bottom",
): Promise<void> {
  const area = page.getByTestId("deck-selection-area");
  await area.evaluate((element) => {
    element.addEventListener(
      "pointerdown",
      (event) => {
        element.setAttribute("data-test-pointer-id", String(event.pointerId));
      },
      { once: true },
    );
  });
  const point = await page.evaluate((requestedEdge) => {
    const candidates = Array.from(
      document.querySelectorAll<HTMLElement>("[data-deck-selection-id]"),
    )
      .map((element) => {
        const bounds = element.getBoundingClientRect();
        const x = bounds.left + 8;
        const y = Math.max(
          64,
          Math.min(window.innerHeight - 64, bounds.top + bounds.height / 2),
        );
        return { element, bounds, x, y };
      })
      .filter(
        ({ element, bounds, x, y }) =>
          bounds.bottom > 56 &&
          bounds.top < window.innerHeight - 56 &&
          element.contains(document.elementFromPoint(x, y)),
      );
    const candidate =
      requestedEdge === "bottom" ? candidates.at(-1) : candidates.at(0);
    if (!candidate) return null;
    return {
      x: candidate.x,
      y: candidate.y,
      edgeY: requestedEdge === "bottom" ? window.innerHeight - 1 : 1,
    };
  }, edge);
  if (!point)
    throw new Error("No visible deck is available for edge scrolling");
  await page.mouse.move(point.x, point.y);
  await page.mouse.down();
  await page.mouse.move(point.x, point.edgeY, { steps: 4 });
}

async function selectDeckView(
  page: import("@playwright/test").Page,
  view: "Grid" | "List",
): Promise<void> {
  await page
    .getByRole("group", { name: "Deck view" })
    .getByRole("button", { name: view })
    .click();
}

async function deckDetails(
  page: import("@playwright/test").Page,
  deckId: string,
) {
  const deck = page.getByTestId(`deck-${deckId}`);
  return {
    name: (await deck.locator("[data-deck-name]").innerText()).trim(),
    counts: await deck.locator("dl > div").evaluateAll((items) =>
      items.map((item) => ({
        label: item.querySelector("dt")?.textContent?.trim(),
        value: item.querySelector("dd")?.textContent?.trim(),
      })),
    ),
    open: await deck.getByRole("button", { name: "Open" }).count(),
    study: await deck.getByRole("button", { name: /^(Study|Resume)$/ }).count(),
    actions: await deck.getByRole("button", { name: /Actions for/ }).count(),
  };
}

async function confirmJapaneseBundleRemoval(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.getByRole("button", { name: "Bundle actions" }).click();
  await page
    .getByRole("dialog", { name: "Bundle actions" })
    .getByRole("button", { name: /Remove Japanese/ })
    .click();
  await page
    .getByRole("alertdialog", { name: "Remove Japanese?" })
    .getByRole("button", { name: "Remove bundle" })
    .click();
}

test("includes suspended cards in Total and presents the populated default deck as Unsorted", async ({
  page,
}) => {
  await openDecks(page);

  const unsorted = page.getByTestId("deck-default-deck");
  await expect(unsorted.getByText("Unsorted", { exact: true })).toBeVisible();
  await expect(unsorted.locator("dl")).toContainText(
    /Total\s*3\s*Due\s*1\s*New\s*1/,
  );
});

test("defaults to Grid and persists pointer and keyboard view changes", async ({
  page,
}) => {
  await page.evaluate(() => localStorage.removeItem("meiki-decks-view"));
  await page.reload();
  await openDecks(page);
  const viewControl = page.getByRole("group", { name: "Deck view" });
  const grid = viewControl.getByRole("button", { name: "Grid" });
  const list = viewControl.getByRole("button", { name: "List" });
  await expect(grid).toHaveAttribute("aria-pressed", "true");
  await expect(list).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByTestId("deck-grid")).toBeVisible();
  await expect(page.getByTestId("deck-list")).toHaveCount(0);

  await list.click();
  await expect(list).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("deck-list")).toBeVisible();
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-decks-view")),
  ).toBe("list");

  await page.reload();
  await openDecks(page);
  await expect(list).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("deck-list")).toBeVisible();

  await grid.focus();
  await page.keyboard.press("Enter");
  await expect(grid).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("deck-grid")).toBeVisible();
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
  await list.focus();
  await page.keyboard.press("Space");
  await expect(list).toHaveAttribute("aria-pressed", "true");
});

test("keeps deck information and actions identical in Grid and List", async ({
  page,
}) => {
  await openDecks(page);
  const gridDetails = await Promise.all([
    deckDetails(page, "default-deck"),
    deckDetails(page, "travel-deck"),
  ]);
  await selectDeckView(page, "List");
  const listDetails = await Promise.all([
    deckDetails(page, "default-deck"),
    deckDetails(page, "travel-deck"),
  ]);

  expect(listDetails).toEqual(gridDetails);
  expect(listDetails).toEqual([
    {
      name: "Unsorted",
      counts: [
        { label: "Total", value: "3" },
        { label: "Due", value: "1" },
        { label: "New", value: "1" },
      ],
      open: 1,
      study: 1,
      actions: 0,
    },
    {
      name: "Travel phrases",
      counts: [
        { label: "Total", value: "2" },
        { label: "Due", value: "0" },
        { label: "New", value: "1" },
      ],
      open: 1,
      study: 1,
      actions: 1,
    },
  ]);
});

test("aligns every List count column when Study and Resume labels differ", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await seedStudyState(page, "travel-deck", "__all_decks__");
  await openDecks(page);
  await selectDeckView(page, "List");
  await expect(
    page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Resume" }),
  ).toBeVisible();
  await expect(
    page
      .getByTestId("deck-listening-deck")
      .getByRole("button", { name: "Study" }),
  ).toBeVisible();

  const countColumns = await page
    .locator(".deck-list-row")
    .evaluateAll((rows) =>
      rows.map((row) =>
        Array.from(row.querySelectorAll<HTMLElement>(".deck-counts > div")).map(
          (count) => {
            const bounds = count.getBoundingClientRect();
            return { x: bounds.x, width: bounds.width };
          },
        ),
      ),
    );
  expect(countColumns.length).toBeGreaterThan(1);
  const expectedColumns = countColumns[0];
  for (const columns of countColumns.slice(1)) {
    expect(columns).toHaveLength(expectedColumns.length);
    for (const [index, column] of columns.entries()) {
      expect(Math.abs(column.x - expectedColumns[index].x)).toBeLessThanOrEqual(
        0.5,
      );
      expect(
        Math.abs(column.width - expectedColumns[index].width),
      ).toBeLessThanOrEqual(0.5);
    }
  }
});

test("keeps a visible gap between Grid navigation actions", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await seedStudyState(page, "travel-deck", "__all_decks__");
  await openDecks(page);

  for (const { deckId, studyLabel } of [
    { deckId: "travel-deck", studyLabel: "Resume" },
    { deckId: "listening-deck", studyLabel: "Study" },
  ]) {
    const deck = page.getByTestId(`deck-${deckId}`);
    const openBounds = await deck
      .getByRole("button", { name: "Open" })
      .boundingBox();
    const studyBounds = await deck
      .getByRole("button", { name: studyLabel })
      .boundingBox();
    if (!openBounds || !studyBounds) {
      throw new Error("Grid navigation action geometry is unavailable");
    }
    expect(
      studyBounds.x - (openBounds.x + openBounds.width),
    ).toBeGreaterThanOrEqual(minimumActionGapPixels);
  }
});

test("uses the shared single-deck deletion flow from List", async ({
  page,
}) => {
  await openDecks(page);
  await selectDeckView(page, "List");
  await openDeckDeleteAction(page, "travel-deck", "Travel phrases");
  const confirmation = page.getByRole("alertdialog", {
    name: "Delete “Travel phrases”?",
  });
  await expect(confirmation).toContainText(
    "Its 2 cards will be moved to Trash.",
  );
  await confirmation.getByRole("button", { name: "Delete deck" }).click();

  await expect(page.getByTestId("deck-travel-deck")).toHaveCount(0);
  await expect(page.getByTestId("deck-list")).toBeVisible();
  expect(await deleteDeckRequestCount(page)).toBe(1);
});

for (const deckView of ["Grid", "List"] as const) {
  test(`keeps selection controls available and clears selection in ${deckView}`, async ({
    page,
  }) => {
    await page.goto("/?decks=batch");
    await openDecks(page);
    if (deckView === "List") await selectDeckView(page, "List");

    await expectDeckSelectionControls(page);
    await expect(page.getByRole("checkbox")).toHaveCount(5);
    await expect(
      page
        .getByTestId("deck-default-deck")
        .getByRole("checkbox", { name: /Select Unsorted/ }),
    ).toHaveCount(0);
    await selectDeck(page, "Travel phrases");
    await expect(page.getByTestId("deck-selection-count")).toContainText(
      "1 deck selected",
    );
    await page.getByRole("button", { name: "Clear selection" }).click();

    await expect(page.getByRole("checkbox")).toHaveCount(5);
    await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);
    await expect(
      page.getByRole("checkbox", { name: "Select Travel phrases" }),
    ).toHaveAttribute("aria-checked", "false");
  });
}

for (const deckView of ["Grid", "List"] as const) {
  test(`a packaged-compatible left-button drag selects a partial deck intersection in ${deckView}`, async ({
    page,
  }) => {
    await page.goto("/?decks=batch");
    await openDecks(page);
    if (deckView === "List") await selectDeckView(page, "List");

    const pointerId = await beginPackagedCompatibleDrag(page, "travel-deck");

    await expect(page.getByTestId("deck-selection-rectangle")).toBeVisible();
    await expect(
      page.getByRole("checkbox", { name: "Select Travel phrases" }),
    ).toHaveAttribute("aria-checked", "true");
    await expect(page.getByTestId("deck-selection-count")).toContainText(
      "1 deck selected",
    );
    await expect(
      page
        .getByTestId("deck-default-deck")
        .getByRole("checkbox", { name: "Select Unsorted" }),
    ).toHaveCount(0);
    await finishPackagedCompatibleDrag(page, pointerId);
    await expect(page.getByTestId("deck-selection-rectangle")).toHaveCount(0);
    await expect(
      page.getByRole("heading", { name: "Decks", level: 1 }),
    ).toBeVisible();
  });
}

for (const { deckView, deckId, deckName } of [
  {
    deckView: "Grid",
    deckId: "travel-deck",
    deckName: "Travel phrases",
  },
  {
    deckView: "List",
    deckId: "deck:ja-JP:00",
    deckName: "Japanese 00 — Kana, sound, and Japanese input",
  },
] as const) {
  test(`a ${deckView} background double-click opens the clicked deck once`, async ({
    page,
  }) => {
    await page.goto("/?bundleRemoval=installed&decks=batch");
    await openDecks(page);
    if (deckView === "List") await selectDeckView(page, "List");
    await selectDeck(page, "Archived phrases");
    await page
      .getByTestId("deck-listening-deck")
      .getByRole("button", { name: "Open" })
      .focus();

    await doubleClickDeckBackground(page, deckId);

    await expect(
      page.getByRole("heading", { name: deckName, level: 1 }),
    ).toBeVisible();
    const deckRequests = await page.evaluate(() =>
      (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
        (request) => request.command === "get_deck_cards",
      ),
    );
    expect(deckRequests).toHaveLength(1);
    expect(deckRequests[0]?.args).toMatchObject({
      request: { deck_id: deckId },
    });

    if (deckId === "deck:ja-JP:00") {
      await page.getByRole("button", { name: "Delete deck" }).click();
      await expect(
        page.getByRole("alertdialog", { name: `Delete “${deckName}”?` }),
      ).toContainText(
        "Bundled cards in this deck will be permanently removed. Personal cards will be moved to Trash.",
      );
    }
  });
}

test("a background double-click opens visible Unsorted", async ({ page }) => {
  await openDecks(page);

  await doubleClickDeckBackground(page, "default-deck");

  await expect(
    page.getByRole("heading", { name: "Unsorted", level: 1 }),
  ).toBeVisible();
  expect((await lastRequest(page, "get_deck_cards"))?.args).toMatchObject({
    request: { deck_id: "default-deck" },
  });
});

for (const deckView of ["Grid", "List"] as const) {
  test(`double-clicking a ${deckView} selection control does not open its deck`, async ({
    page,
  }) => {
    await page.goto("/?decks=batch");
    await openDecks(page);
    if (deckView === "List") await selectDeckView(page, "List");

    await page
      .getByRole("checkbox", { name: "Select Travel phrases" })
      .dblclick();

    await expect(
      page.getByRole("heading", { name: "Decks", level: 1 }),
    ).toBeVisible();
    expect(await lastRequest(page, "get_deck_cards")).toBeUndefined();
  });
}

for (const deckView of ["Grid", "List"] as const) {
  test(`background clicks replace selection and empty space clears it in ${deckView}`, async ({
    page,
  }) => {
    await page.goto("/?decks=batch");
    await openDecks(page);
    if (deckView === "List") await selectDeckView(page, "List");
    await selectDeck(page, "Travel phrases");
    await selectDeck(page, "Listening practice");

    await clickDeckBackground(page, "travel-deck");
    await expect(
      page.getByRole("checkbox", { name: "Select Travel phrases" }),
    ).toHaveAttribute("aria-checked", "true");
    await expect(page.getByRole("checkbox", { checked: true })).toHaveCount(1);

    await selectDeck(page, "Listening practice");
    await clickDeckBackground(page, "archive-deck");
    await expect(
      page.getByRole("checkbox", { name: "Select Archived phrases" }),
    ).toHaveAttribute("aria-checked", "true");
    await expect(page.getByRole("checkbox", { checked: true })).toHaveCount(1);

    await selectDeck(page, "Travel phrases");
    const { start } = await partialDeckDragPoints(page, "travel-deck");
    await clickBelowPointerThreshold(page, start);
    await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);

    await selectDeck(page, "Travel phrases");
    await selectDeck(page, "Listening practice");
    await clickDeckBackground(page, "default-deck");
    await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);
  });

  test(`interactive deck controls keep their native selection behavior in ${deckView}`, async ({
    page,
  }) => {
    await page.goto("/?decks=batch");
    await openDecks(page);
    if (deckView === "List") await selectDeckView(page, "List");
    await selectDeck(page, "Travel phrases");
    await selectDeck(page, "Listening practice");

    await page
      .getByRole("button", { name: "Actions for Travel phrases" })
      .click();
    await expect(
      page.getByRole("menuitem", { name: "Delete deck" }),
    ).toBeVisible();
    await expect(page.getByRole("checkbox", { checked: true })).toHaveCount(2);
    await page.keyboard.press("Escape");

    await selectDeck(page, "Travel phrases");
    await expect(
      page.getByRole("checkbox", { name: "Select Travel phrases" }),
    ).toHaveAttribute("aria-checked", "false");
    await expect(
      page.getByRole("checkbox", { name: "Select Listening practice" }),
    ).toHaveAttribute("aria-checked", "true");
  });
}

for (const cancellation of ["pointer cancel", "window blur"] as const) {
  test(`a pending background click is discarded on ${cancellation}`, async ({
    page,
  }) => {
    await page.goto("/?decks=batch");
    await openDecks(page);
    await selectDeck(page, "Travel phrases");
    await selectDeck(page, "Listening practice");
    const pointerId = await beginDeckBackgroundPress(page, "travel-deck");

    if (cancellation === "pointer cancel") {
      await page.evaluate((activePointerId) => {
        window.dispatchEvent(
          new PointerEvent("pointercancel", {
            bubbles: true,
            pointerId: activePointerId,
            pointerType: "mouse",
          }),
        );
      }, pointerId);
    } else {
      await page.evaluate(() => window.dispatchEvent(new Event("blur")));
    }
    await page.mouse.up();

    await expect(page.getByRole("checkbox", { checked: true })).toHaveCount(2);
    await expect(page.getByTestId("deck-selection-count")).toContainText(
      "2 decks selected",
    );
  });
}

test("the rectangle stays clipped to Decks without horizontal overflow", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await openDecks(page);
  const area = page.getByTestId("deck-selection-area");
  const areaBefore = await area.boundingBox();
  if (!areaBefore) throw new Error("Deck selection area is unavailable");
  await page.mouse.move(areaBefore.x + 2, areaBefore.y + 2);
  await page.mouse.down();
  await page.mouse.move(
    areaBefore.x + areaBefore.width + 200,
    areaBefore.y + 40,
  );

  const rectangle = await page
    .getByTestId("deck-selection-rectangle")
    .boundingBox();
  const areaAfter = await area.boundingBox();
  if (!rectangle || !areaAfter) {
    throw new Error("Deck selection rectangle geometry is unavailable");
  }
  expect(rectangle.x).toBeGreaterThanOrEqual(areaAfter.x);
  expect(rectangle.x + rectangle.width).toBeLessThanOrEqual(
    areaAfter.x + areaAfter.width,
  );
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.mouse.up();
});

test("plain and Shift rectangle drags use the pointer-down selection snapshot", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await openDecks(page);
  await selectDeck(page, "Travel phrases");

  await dragAcrossDeckEdge(page, "listening-deck");
  await expect(
    page.getByRole("checkbox", { name: "Select Travel phrases" }),
  ).toHaveAttribute("aria-checked", "false");
  await expect(
    page.getByRole("checkbox", { name: "Select Listening practice" }),
  ).toHaveAttribute("aria-checked", "true");

  await dragAcrossDeckEdge(page, "travel-deck", "Shift", true);
  await expect(
    page.getByRole("checkbox", { name: "Select Travel phrases" }),
  ).toHaveAttribute("aria-checked", "true");
  await expect(
    page.getByRole("checkbox", { name: "Select Listening practice" }),
  ).toHaveAttribute("aria-checked", "true");
  await expect(page.getByTestId("deck-selection-count")).toContainText(
    "2 decks selected",
  );
});

for (const modifierCase of [
  { platform: "macOS", modifier: "Meta", label: "Command on macOS" },
  { platform: "Windows", modifier: "Control", label: "Ctrl outside macOS" },
] as const) {
  test(`${modifierCase.label} toggles intersections from the pointer-down snapshot`, async ({
    page,
  }) => {
    await setPointerPlatform(page, modifierCase.platform);
    await page.goto("/?decks=batch");
    await openDecks(page);
    await selectDeck(page, "Travel phrases");
    await selectDeck(page, "Listening practice");

    await dragAcrossDeckEdge(page, "travel-deck", modifierCase.modifier, true);

    await expect(
      page.getByRole("checkbox", { name: "Select Travel phrases" }),
    ).toHaveAttribute("aria-checked", "false");
    await expect(
      page.getByRole("checkbox", { name: "Select Listening practice" }),
    ).toHaveAttribute("aria-checked", "true");
    await expect(page.getByTestId("deck-selection-count")).toContainText(
      "1 deck selected",
    );
  });
}

test("a rectangle excludes Unsorted while selecting every deletable deck", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await openDecks(page);
  const area = await page.getByTestId("deck-selection-area").boundingBox();
  if (!area) throw new Error("Deck selection area is unavailable");
  await page.mouse.move(area.x + 1, area.y + 1);
  await page.mouse.down();
  await page.mouse.move(area.x + area.width - 1, area.y + area.height - 1, {
    steps: 8,
  });
  await page.mouse.up();

  await expect(page.getByRole("checkbox", { checked: true })).toHaveCount(5);
  await expect(page.getByTestId("deck-selection-count")).toContainText(
    "5 decks selected",
  );
  await expect(page.getByTestId("deck-default-deck")).toHaveAttribute(
    "data-selected",
    "false",
  );
});

test("interactive origins never begin rectangle selection", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await openDecks(page);
  const travel = page.getByTestId("deck-travel-deck");
  await dragFromElement(
    page,
    travel.getByRole("checkbox", { name: "Select Travel phrases" }),
  );
  await dragFromElement(page, travel.getByRole("button", { name: "Open" }));
  await dragFromElement(
    page,
    travel.getByRole("button", { name: "Actions for Travel phrases" }),
  );
  await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);

  await page.getByTestId("deck-selection-area").evaluate((area) => {
    const fixtures = document.createElement("div");
    fixtures.dataset.testid = "interactive-selection-origins";
    fixtures.setAttribute(
      "style",
      "position:absolute;z-index:20;left:4px;top:4px;display:flex;gap:2px",
    );
    for (const tag of ["a", "input", "select", "textarea"] as const) {
      const element = document.createElement(tag);
      element.dataset.interactiveOrigin = tag;
      if (element instanceof HTMLAnchorElement) element.href = "#decks-title";
      element.setAttribute("style", "width:24px;height:24px");
      fixtures.append(element);
    }
    const editable = document.createElement("span");
    editable.dataset.interactiveOrigin = "contenteditable";
    editable.contentEditable = "true";
    editable.setAttribute("style", "display:block;width:24px;height:24px");
    fixtures.append(editable);
    const label = document.createElement("label");
    label.dataset.interactiveOrigin = "label";
    label.setAttribute("style", "display:block;width:24px;height:24px");
    fixtures.append(label);
    const focusable = document.createElement("span");
    focusable.dataset.interactiveOrigin = "tabindex";
    focusable.tabIndex = 0;
    focusable.setAttribute("style", "display:block;width:24px;height:24px");
    fixtures.append(focusable);
    const overlayControl = document.createElement("span");
    overlayControl.dataset.interactiveOrigin = "overlay";
    overlayControl.dataset.deckSelectionInteractive = "true";
    overlayControl.setAttribute(
      "style",
      "display:block;width:24px;height:24px",
    );
    fixtures.append(overlayControl);
    area.append(fixtures);
  });

  for (const origin of [
    "a",
    "input",
    "select",
    "textarea",
    "contenteditable",
    "label",
    "tabindex",
    "overlay",
  ]) {
    const interactiveOrigin = page.locator(
      `[data-interactive-origin="${origin}"]`,
    );
    await dragFromElement(page, interactiveOrigin);
    await expect(page.getByTestId("deck-selection-rectangle")).toHaveCount(0);
    await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);
    await interactiveOrigin.click();
    await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);
  }
});

test("single-deck actions remain available with selected decks", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await openDecks(page);
  await selectDeck(page, "Travel phrases");
  await openDeckDeleteAction(page, "listening-deck", "Listening practice");
  await expect(
    page.getByRole("alertdialog", { name: "Delete “Listening practice”?" }),
  ).toBeVisible();
});

for (const edge of ["bottom", "top"] as const) {
  test(`${edge}-edge scrolling continues rectangle selection and stops on release`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 700, height: 420 });
    await page.goto("/?decks=scroll");
    await openDecks(page);
    await selectDeckView(page, "List");
    if (edge === "top") {
      await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
    } else {
      await page.getByTestId("deck-travel-deck").scrollIntoViewIfNeeded();
    }
    const scrollBefore = await page.evaluate(() => window.scrollY);

    await startVerticalEdgeSelection(page, edge);
    const scrollPosition = expect.poll(() =>
      page.evaluate(() => window.scrollY),
    );
    if (edge === "bottom") {
      await scrollPosition.toBeGreaterThan(scrollBefore);
    } else {
      await scrollPosition.toBeLessThan(scrollBefore);
    }
    await expect
      .poll(() => page.getByRole("checkbox", { checked: true }).count())
      .toBeGreaterThan(0);
    await page.mouse.up();
    const scrollAfterRelease = await page.evaluate(() => window.scrollY);
    await page.waitForTimeout(100);
    expect(await page.evaluate(() => window.scrollY)).toBe(scrollAfterRelease);
  });
}

test("edge scrolling uses the nearest vertical Decks container only", async ({
  page,
}) => {
  await page.setViewportSize({ width: 900, height: 700 });
  await page.goto("/?decks=scroll");
  await openDecks(page);
  await selectDeckView(page, "List");
  const main = page.locator("#main-content");
  await main.evaluate((element) => {
    element.style.height = "520px";
    element.style.minHeight = "0";
    element.style.overflowX = "hidden";
    element.style.overflowY = "auto";
  });
  const windowScrollBefore = await page.evaluate(() => window.scrollY);
  await startVerticalEdgeSelection(page, "bottom");
  await expect
    .poll(() => main.evaluate((element) => element.scrollTop))
    .toBeGreaterThan(0);
  expect(await main.evaluate((element) => element.scrollLeft)).toBe(0);
  expect(await page.evaluate(() => window.scrollY)).toBe(windowScrollBefore);
  await page.mouse.up();
});

for (const stopCase of ["pointer cancel", "window blur", "unmount"] as const) {
  test(`edge scrolling stops immediately on ${stopCase}`, async ({ page }) => {
    await page.setViewportSize({ width: 700, height: 420 });
    await page.goto("/?decks=scroll");
    await openDecks(page);
    await selectDeckView(page, "List");
    await page.getByTestId("deck-travel-deck").scrollIntoViewIfNeeded();
    const scrollBefore = await page.evaluate(() => window.scrollY);
    await startVerticalEdgeSelection(page, "bottom");
    await expect
      .poll(() => page.evaluate(() => window.scrollY))
      .toBeGreaterThan(scrollBefore);

    if (stopCase === "pointer cancel") {
      await page.getByTestId("deck-selection-area").evaluate((area) => {
        const pointerId = Number(area.getAttribute("data-test-pointer-id"));
        area.dispatchEvent(
          new PointerEvent("pointercancel", {
            bubbles: true,
            pointerId,
            pointerType: "mouse",
          }),
        );
      });
    } else if (stopCase === "window blur") {
      await page.evaluate(() => window.dispatchEvent(new Event("blur")));
    } else {
      await page
        .getByRole("button", { name: "Open navigation" })
        .evaluate((button: HTMLButtonElement) => button.click());
      await page
        .getByRole("navigation", { name: "Primary navigation" })
        .getByRole("button", { name: "Today", exact: true })
        .evaluate((button: HTMLButtonElement) => button.click());
      await expect(
        page.getByRole("heading", { name: "Today", level: 1 }),
      ).toBeVisible();
    }
    const stoppedAt = await page.evaluate(() => window.scrollY);
    await page.waitForTimeout(100);
    expect(await page.evaluate(() => window.scrollY)).toBe(stoppedAt);
    await expect(page.getByTestId("deck-selection-rectangle")).toHaveCount(0);
    await page.mouse.up();
  });
}

test("keeps selected decks while switching between Grid and List", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await openDecks(page);
  await expectDeckSelectionControls(page);
  await selectDeck(page, "Travel phrases");
  await selectDeck(page, "Japanese 00 — Kana, sound, and Japanese input");
  await expect(page.getByTestId("deck-selection-count")).toContainText(
    "2 decks selected",
  );

  await selectDeckView(page, "List");
  await expect(
    page.getByRole("checkbox", { name: "Select Travel phrases" }),
  ).toHaveAttribute("aria-checked", "true");
  await expect(
    page.getByRole("checkbox", {
      name: "Select Japanese 00 — Kana, sound, and Japanese input",
    }),
  ).toHaveAttribute("aria-checked", "true");
  await selectDeckView(page, "Grid");
  await expect(page.getByTestId("deck-selection-count")).toContainText(
    "2 decks selected",
  );
});

test("deletes several ordinary decks with one batch command", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await openDecks(page);
  await expectDeckSelectionControls(page);
  for (const deckName of [
    "Travel phrases",
    "Listening practice",
    "Archived phrases",
  ]) {
    await selectDeck(page, deckName);
  }
  await page.getByRole("button", { name: "Delete selected" }).click();
  const confirmation = page.getByRole("alertdialog", {
    name: "Delete 3 selected decks?",
  });
  await expect(confirmation).toContainText(
    "7 cards in 3 ordinary decks will be moved to Trash.",
  );
  await confirmation.getByRole("button", { name: "Delete selected" }).click();

  await expect(
    page.getByTestId("deletion-activity").getByText("Deleted 3 decks."),
  ).toBeVisible();
  expect(await batchDeleteRequestCount(page)).toBe(1);
  expect(await deleteDeckRequestCount(page)).toBe(0);
  expect((await lastRequest(page, "delete_decks"))?.args).toMatchObject({
    request: {
      deck_ids: ["travel-deck", "listening-deck", "archive-deck"],
    },
  });
  await expect(page.getByTestId("deck-travel-deck")).toHaveCount(0);
  await expect(page.getByTestId("deck-listening-deck")).toHaveCount(0);
  await expect(page.getByTestId("deck-archive-deck")).toHaveCount(0);
});

test("confirms several bundle stages once with permanent-content copy", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await openDecks(page);
  await expectDeckSelectionControls(page);
  await selectDeck(page, "Japanese 00 — Kana, sound, and Japanese input");
  await selectDeck(page, "Japanese 01 — N5 / A1 foundation");
  await page.getByRole("button", { name: "Delete selected" }).click();

  const confirmation = page.getByRole("alertdialog", {
    name: "Delete 2 selected decks?",
  });
  await expect(confirmation).toContainText(
    "Bundled content in 2 bundle stages will be permanently removed.",
  );
  await expect(confirmation).toContainText(
    "Personal cards in those stages will be moved to Trash.",
  );
  await confirmation.getByRole("button", { name: "Delete selected" }).click();
  await expect(
    page.getByTestId("deletion-activity").getByText("Deleted 2 decks."),
  ).toBeVisible();
  expect(await batchDeleteRequestCount(page)).toBe(1);
});

test("mixed deletion clears only removed focused queue and Today state", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await seedStudyState(page, "travel-deck", "deck:ja-JP:00");
  await page.evaluate(() => localStorage.setItem("meiki-decks-view", "list"));
  await openDecks(page);
  await expectDeckSelectionControls(page);
  await selectDeck(page, "Travel phrases");
  await selectDeck(page, "Japanese 00 — Kana, sound, and Japanese input");
  await page.getByRole("button", { name: "Delete selected" }).click();

  const confirmation = page.getByRole("alertdialog", {
    name: "Delete 2 selected decks?",
  });
  await expect(confirmation).toContainText(
    "2 cards in 1 ordinary deck will be moved to Trash.",
  );
  await expect(confirmation).toContainText(
    "Bundled content in 1 bundle stage will be permanently removed.",
  );
  await confirmation.getByRole("button", { name: "Delete selected" }).click();
  await expect(
    page.getByTestId("deletion-activity").getByText("Deleted 2 decks."),
  ).toBeVisible();

  expect(
    await page.evaluate(() => ({
      queue: localStorage.getItem("meiki-active-study-queue"),
      session: sessionStorage.getItem("meiki-active-study-session"),
      today: localStorage.getItem("meiki-today-deck"),
      view: localStorage.getItem("meiki-decks-view"),
    })),
  ).toEqual({
    queue: null,
    session: null,
    today: "__all_decks__",
    view: "list",
  });
  await expect(page.getByRole("checkbox")).toHaveCount(3);
  await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);
  await expect(page.getByTestId("deck-list")).toBeVisible();
});

test("batch deletion preserves all-decks queue and unrelated Today state", async ({
  page,
}) => {
  await page.goto("/?decks=batch");
  await seedStudyState(page, "__all_decks__", "archive-deck");
  await openDecks(page);
  await expectDeckSelectionControls(page);
  await selectDeck(page, "Listening practice");
  await page.getByRole("button", { name: "Delete selected" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete 1 selected deck?" })
    .getByRole("button", { name: "Delete selected" })
    .click();
  await expect(
    page.getByTestId("deletion-activity").getByText("Deleted 1 deck."),
  ).toBeVisible();

  expect(
    await page.evaluate(() => ({
      queue: JSON.parse(
        localStorage.getItem("meiki-active-study-queue") ?? "null",
      ) as { deckId?: string } | null,
      session: sessionStorage.getItem("meiki-active-study-session"),
      today: localStorage.getItem("meiki-today-deck"),
    })),
  ).toMatchObject({
    queue: { deckId: "__all_decks__" },
    session: "session for __all_decks__",
    today: "archive-deck",
  });
});

test("pre-commit batch failure preserves decks, selection, queue, and Today", async ({
  page,
}) => {
  await page.goto("/?decks=batch&batchDeletion=precommit-failure");
  await seedStudyState(page, "travel-deck", "travel-deck");
  await openDecks(page);
  await expectDeckSelectionControls(page);
  await selectDeck(page, "Travel phrases");
  await selectDeck(page, "Listening practice");
  await page.getByRole("button", { name: "Delete selected" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete 2 selected decks?" })
    .getByRole("button", { name: "Delete selected" })
    .click();

  const failure = page.getByRole("dialog", { name: "Decks were not deleted" });
  await expect(failure).toContainText("Your collection was left unchanged.");
  await failure.locator('[data-slot="dialog-footer"] button').click();
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
  await expect(page.getByTestId("deck-listening-deck")).toBeVisible();
  await expect(page.getByTestId("deck-selection-count")).toContainText(
    "2 decks selected",
  );
  expect(
    await page.evaluate(() => ({
      queue: JSON.parse(
        localStorage.getItem("meiki-active-study-queue") ?? "null",
      ) as { deckId?: string } | null,
      session: sessionStorage.getItem("meiki-active-study-session"),
      today: localStorage.getItem("meiki-today-deck"),
    })),
  ).toMatchObject({
    queue: { deckId: "travel-deck" },
    session: "session for travel-deck",
    today: "travel-deck",
  });
});

test("post-commit cleanup failure reports that every selected deck was deleted", async ({
  page,
}) => {
  await page.goto("/?decks=batch&batchDeletion=postcommit-failure");
  await openDecks(page);
  await expectDeckSelectionControls(page);
  await selectDeck(page, "Travel phrases");
  await selectDeck(page, "Japanese 00 — Kana, sound, and Japanese input");
  await page.getByRole("button", { name: "Delete selected" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete 2 selected decks?" })
    .getByRole("button", { name: "Delete selected" })
    .click();

  const warning = page.getByRole("dialog", { name: "Decks deleted" });
  await expect(warning).toContainText(
    "Decks deleted, but some unused audio could not be cleaned up.",
  );
  await warning.locator('[data-slot="dialog-footer"] button').click();
  await expect(page.getByTestId("deletion-activity")).toContainText(
    "Decks deleted, but some unused audio could not be cleaned up.",
  );
  await expect(page.getByTestId("deck-travel-deck")).toHaveCount(0);
  await expect(page.getByTestId("deck-deck:ja-JP:00")).toHaveCount(0);
});

test("batch deletion progress is semantic and monotonic", async ({ page }) => {
  await page.goto("/?decks=batch&batchDeletion=progress");
  await openDecks(page);
  await expectDeckSelectionControls(page);
  await selectDeck(page, "Travel phrases");
  await selectDeck(page, "Japanese 00 — Kana, sound, and Japanese input");
  await page.getByRole("button", { name: "Delete selected" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete 2 selected decks?" })
    .getByRole("button", { name: "Delete selected" })
    .click();

  const progress = page.getByRole("dialog", { name: "Deleting 2 decks" });
  await expect(progress.getByText("Preparing", { exact: true })).toBeVisible();
  await expect(
    progress.getByRole("progressbar", { name: "Preparing" }),
  ).not.toHaveAttribute("aria-valuenow");
  await expect(
    progress.getByText("Removing cards", { exact: true }),
  ).toBeVisible();
  await expect(
    progress.getByRole("progressbar", { name: "Removing cards" }),
  ).toHaveAttribute("aria-valuenow", "302");
  await expect(
    progress.getByText("Cleaning audio", { exact: true }),
  ).toBeVisible();
  await expect(
    progress.getByRole("progressbar", { name: "Cleaning audio" }),
  ).toHaveAttribute("aria-valuenow", "300");
  await expect(progress.getByText("Finalizing", { exact: true })).toBeVisible();
  await expect(
    page.getByTestId("deletion-activity").getByText("Deleted 2 decks."),
  ).toBeVisible();
});

for (const deckView of ["Grid", "List"] as const) {
  test(`keeps loading, empty, error, session, notice, and bundle states in ${deckView}`, async ({
    page,
  }) => {
    await page.evaluate((view) => {
      localStorage.setItem("meiki-decks-view", view.toLocaleLowerCase());
    }, deckView);

    await page.goto("/?decks=loading");
    await openDecks(page);
    await expect(page.getByText("Loading decks…")).toBeVisible();
    await expect(
      page
        .getByRole("group", { name: "Deck view" })
        .getByRole("button", { name: deckView }),
    ).toHaveAttribute("aria-pressed", "true");

    await page.goto("/?decks=empty");
    await openDecks(page);
    await expect(
      page.getByRole("heading", { name: "Create your first deck" }),
    ).toBeVisible();

    await page.goto("/?decks=error");
    await openDecks(page);
    await expect(page.getByRole("alert")).toContainText(
      "The local collection is temporarily unavailable.",
    );

    await seedStudyState(page, "__all_decks__", "__all_decks__");
    await page.goto("/?bundleRemoval=installed&today=empty");
    await openDecks(page);
    await expect(page.getByText(/A saved session is active/)).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Bundle actions" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Import bundle" }),
    ).toBeVisible();
    await page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Study" })
      .click();
    await expect(
      page.getByText("Travel phrases has no cards ready to study."),
    ).toBeVisible();
  });
}

test("wraps long names without horizontal overflow in Grid or narrow List", async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 720 });
  await page.goto("/?decks=long-name");
  await openDecks(page);
  const longName =
    "Travel phrases for an exceptionally long multilingual journey through 日本語 and العربية";
  const gridDeck = page.getByTestId("deck-travel-deck");
  await expect(gridDeck.locator("[data-deck-name]")).toHaveText(longName);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);

  await selectDeckView(page, "List");
  const listDeck = page.getByTestId("deck-travel-deck");
  const nameBounds = await listDeck.locator("[data-deck-name]").boundingBox();
  const countBounds = await listDeck.locator("dl").boundingBox();
  const actionBounds = await listDeck
    .locator(".deck-list-actions")
    .boundingBox();
  expect(
    nameBounds &&
      countBounds &&
      countBounds.y >= nameBounds.y + nameBounds.height,
  ).toBe(true);
  expect(
    countBounds &&
      actionBounds &&
      actionBounds.y >= countBounds.y + countBounds.height,
  ).toBe(true);
  await expect(
    listDeck.getByRole("button", { name: `Actions for ${longName}` }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});

test("opens each deletable deck's reset action by keyboard and keeps Unsorted without actions", async ({
  page,
}) => {
  await openDecks(page);

  const actions = page.getByRole("button", {
    name: "Actions for Travel phrases",
  });
  await actions.focus();
  await page.keyboard.press("Enter");
  const resetAction = page.getByRole("menuitem", { name: "Reset progress" });
  await expect(resetAction).toBeVisible();
  await expect(resetAction).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const deleteAction = page.getByRole("menuitem", { name: "Delete deck" });
  await expect(deleteAction).toBeVisible();
  await expect(deleteAction).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(actions).toBeFocused();
  await expect(
    page
      .getByTestId("deck-default-deck")
      .getByRole("button", { name: /Actions for/ }),
  ).toHaveCount(0);
});

test("offers Reset progress for ordinary and bundle decks in Grid and List", async ({
  page,
}) => {
  await page.goto("/?bundleRemoval=installed");
  await openDecks(page);
  for (const view of ["Grid", "List"] as const) {
    if (view === "List") await selectDeckView(page, view);
    for (const [deckId, deckName] of [
      ["travel-deck", "Travel phrases"],
      ["deck:ja-JP:00", "Japanese 00 — Kana, sound, and Japanese input"],
    ] as const) {
      await page
        .getByTestId(`deck-${deckId}`)
        .getByRole("button", { name: `Actions for ${deckName}` })
        .click();
      await expect(
        page.getByRole("menuitem", { name: "Reset progress" }),
      ).toBeVisible();
      await page.keyboard.press("Escape");
    }
  }
});

test("confirms one deck reset, refreshes Decks and Today, and preserves Today selection", async ({
  page,
}) => {
  await page.evaluate(() => {
    localStorage.setItem("meiki-today-deck", "travel-deck");
  });
  await openDecks(page);
  await openDeckResetAction(page, "travel-deck", "Travel phrases");

  const confirmation = page.getByRole("alertdialog", {
    name: "Reset progress for “Travel phrases”?",
  });
  await expect(confirmation).toContainText(
    "Reviewed cards in this deck will become new again.",
  );
  await expect(confirmation).toContainText(
    "Cards, notes, media, suspension, deck settings, and bundle membership remain unchanged.",
  );
  await confirmation.getByRole("button", { name: "Reset progress" }).click();

  await expect(
    page.getByText("Reset progress for Travel phrases."),
  ).toBeVisible();
  const resetDeck = page.getByTestId("deck-travel-deck");
  await expect(
    resetDeck.locator("dl div").filter({ hasText: "Due" }).locator("dd"),
  ).toHaveText("0");
  await expect(
    resetDeck.locator("dl div").filter({ hasText: "New" }).locator("dd"),
  ).toHaveText("2");
  expect(await resetDeckProgressRequestCount(page)).toBe(1);
  expect((await lastRequest(page, "reset_deck_progress"))?.args).toMatchObject({
    request: { deck_id: "travel-deck" },
  });
  expect(await lastRequest(page, "undo_review")).toBeUndefined();
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-today-deck")),
  ).toBe("travel-deck");

  await page.getByRole("button", { name: "Today", exact: true }).click();
  await expect(page.getByLabel("Deck")).toHaveValue("travel-deck");
  await expect(page.getByText("0 due and 2 new.")).toBeVisible();
  await expect(page.getByText("No active reviews yet.")).toBeVisible();
  expect((await lastRequest(page, "get_today_statistics"))?.args).toMatchObject(
    {
      request: { deck_id: "travel-deck" },
    },
  );
});

test("reports a reset no-op without changing deck counts", async ({ page }) => {
  await page.goto("/?deckReset=no-progress");
  await openDecks(page);
  await openDeckResetAction(page, "travel-deck", "Travel phrases");
  await page
    .getByRole("alertdialog", {
      name: "Reset progress for “Travel phrases”?",
    })
    .getByRole("button", { name: "Reset progress" })
    .click();

  await expect(
    page.getByText("There is no progress to reset in Travel phrases."),
  ).toBeVisible();
  await expect(
    page
      .getByTestId("deck-travel-deck")
      .locator("dl div")
      .filter({ hasText: "New" })
      .locator("dd"),
  ).toHaveText("1");
  expect(await resetDeckProgressRequestCount(page)).toBe(1);
});

for (const queueDeckId of [
  "travel-deck",
  "__all_decks__",
  "default-deck",
] as const) {
  test(`cleans the ${queueDeckId} queue correctly after reset`, async ({
    page,
  }) => {
    await seedStudyState(page, queueDeckId, "travel-deck");
    await page.goto("/");
    await openDecks(page);
    await openDeckResetAction(page, "travel-deck", "Travel phrases");
    await page
      .getByRole("alertdialog", {
        name: "Reset progress for “Travel phrases”?",
      })
      .getByRole("button", { name: "Reset progress" })
      .click();

    const queue = await page.evaluate(() =>
      localStorage.getItem("meiki-active-study-queue"),
    );
    const session = await page.evaluate(() =>
      sessionStorage.getItem("meiki-active-study-session"),
    );
    if (queueDeckId === "default-deck") {
      expect(JSON.parse(queue ?? "null")).toMatchObject({
        deckId: "default-deck",
      });
      expect(session).toBe("session for default-deck");
    } else {
      expect(queue).toBeNull();
      expect(session).toBeNull();
    }
    expect(
      await page.evaluate(() => localStorage.getItem("meiki-today-deck")),
    ).toBe("travel-deck");
  });
}

test("keeps queue state on a concise reset failure", async ({ page }) => {
  await seedStudyState(page, "travel-deck", "travel-deck");
  const queueBefore = await page.evaluate(() =>
    localStorage.getItem("meiki-active-study-queue"),
  );
  await page.goto("/?deckReset=failure");
  await openDecks(page);
  await openDeckResetAction(page, "travel-deck", "Travel phrases");
  const confirmation = page.getByRole("alertdialog", {
    name: "Reset progress for “Travel phrases”?",
  });
  await confirmation.getByRole("button", { name: "Reset progress" }).click();

  await expect(confirmation.getByRole("alert")).toContainText(
    "Could not reset progress for Travel phrases. Try again.",
  );
  await expect(confirmation).not.toContainText("schedule version");
  await expect(confirmation).not.toContainText("raw fixture id");
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-active-study-queue")),
  ).toBe(queueBefore);
  expect(await resetDeckProgressRequestCount(page)).toBe(1);
});

test("submits a rapid repeated reset confirmation only once", async ({
  page,
}) => {
  await page.goto("/?deckReset=controlled");
  await openDecks(page);
  await openDeckResetAction(page, "travel-deck", "Travel phrases");
  const confirmation = page.getByRole("alertdialog", {
    name: "Reset progress for “Travel phrases”?",
  });
  const resetButton = confirmation.getByRole("button", {
    name: "Reset progress",
  });
  await resetButton.evaluate((button) => {
    button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
  await expect(
    confirmation.getByRole("button", { name: "Resetting…" }),
  ).toBeDisabled();
  expect(await resetDeckProgressRequestCount(page)).toBe(1);
  await page.evaluate(() => {
    window.dispatchEvent(new Event("meiki-e2e-release-deck-reset"));
  });
  await expect(
    page.getByText("Reset progress for Travel phrases."),
  ).toBeVisible();
  expect(await resetDeckProgressRequestCount(page)).toBe(1);
});

test("deletes an ordinary deck from its card once and refreshes Decks in place", async ({
  page,
}) => {
  await openDecks(page);
  await openDeckDeleteAction(page, "travel-deck", "Travel phrases");

  const confirmation = page.getByRole("alertdialog", {
    name: "Delete “Travel phrases”?",
  });
  await expect(confirmation).toContainText(
    "Its 2 cards will be moved to Trash.",
  );
  await expect(confirmation.getByRole("textbox")).toHaveCount(0);
  await confirmation.getByRole("button", { name: "Delete deck" }).click();

  await expect(page.getByTestId("deck-travel-deck")).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Decks", level: 1 }),
  ).toBeVisible();
  await expect(
    page.getByTestId("deletion-activity").getByText("Deleted Travel phrases."),
  ).toBeVisible();
  expect(await deleteDeckRequestCount(page)).toBe(1);
  expect((await lastRequest(page, "delete_deck"))?.args).toMatchObject({
    request: {
      deck_id: "travel-deck",
      move_cards_to_deck_id: null,
      confirmation: "Travel phrases",
    },
  });
});

test("clears selected decks and a pending batch snapshot after one selected deck is deleted", async ({
  page,
}) => {
  await page.goto("/?decks=batch&deckDeletion=postcommit-failure");
  await openDecks(page);
  await expectDeckSelectionControls(page);
  await selectDeck(page, "Travel phrases");
  await selectDeck(page, "Listening practice");
  await page.getByRole("button", { name: "Delete selected" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete 2 selected decks?" })
    .getByRole("button", { name: "Cancel" })
    .click();

  await openDeckDeleteAction(page, "travel-deck", "Travel phrases");
  await page
    .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
    .getByRole("button", { name: "Delete deck" })
    .click();

  const warning = page.getByRole("dialog", { name: "Deck deleted" });
  await expect(warning).toContainText(
    "Deck deleted, but some unused audio could not be cleaned up.",
  );
  await expect(page.getByTestId("deck-travel-deck")).toHaveCount(0);
  await warning.getByRole("button", { name: "Close" }).last().click();
  await expect(page.getByTestId("deck-listening-deck")).toBeVisible();
  await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Select", exact: true }),
  ).toHaveCount(0);

  await expect(
    page.getByRole("checkbox", { name: "Select Listening practice" }),
  ).toHaveAttribute("aria-checked", "false");
  await selectDeck(page, "Listening practice");
  await page.getByRole("button", { name: "Delete selected" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete 1 selected deck?" })
    .getByRole("button", { name: "Delete selected" })
    .click();

  expect(await batchDeleteRequestCount(page)).toBe(1);
  expect((await lastRequest(page, "delete_decks"))?.args).toMatchObject({
    request: { deck_ids: ["listening-deck"] },
  });
});

test("keeps bundle-stage deletion copy and Move cards instead behavior on Decks", async ({
  page,
}) => {
  await page.goto("/?bundleRemoval=installed");
  await openDecks(page);
  await openDeckDeleteAction(
    page,
    "deck:ja-JP:00",
    "Japanese 00 — Kana, sound, and Japanese input",
  );

  const confirmation = page.getByRole("alertdialog", {
    name: "Delete “Japanese 00 — Kana, sound, and Japanese input”?",
  });
  await expect(confirmation).toContainText(
    "Bundled cards in this deck will be permanently removed. Personal cards will be moved to Trash.",
  );
  await confirmation
    .getByRole("button", { name: "Move cards instead" })
    .click();
  const moveDialog = page.getByRole("dialog", { name: "Move cards instead" });
  await expect(moveDialog).toContainText(
    "Move active cards to another deck, then delete “Japanese 00 — Kana, sound, and Japanese input”.",
  );
  await expect(moveDialog.locator('option[value="default-deck"]')).toHaveCount(
    1,
  );
  await moveDialog.getByLabel("Destination deck").selectOption("default-deck");
  await moveDialog
    .getByRole("button", { name: "Move cards and delete" })
    .click();

  await expect(page.getByTestId("deck-deck:ja-JP:00")).toHaveCount(0);
  expect(await deleteDeckRequestCount(page)).toBe(1);
  expect((await lastRequest(page, "delete_deck"))?.args).toMatchObject({
    request: {
      deck_id: "deck:ja-JP:00",
      move_cards_to_deck_id: "default-deck",
    },
  });
});

test("shows the shared monotonic deletion progress from a deck card", async ({
  page,
}) => {
  await page.goto("/?deckDeletion=progress");
  await openDecks(page);
  await openDeckDeleteAction(page, "travel-deck", "Travel phrases");
  await page
    .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
    .getByRole("button", { name: "Delete deck" })
    .click();

  const dialog = page.getByRole("dialog", {
    name: "Deleting “Travel phrases”",
  });
  const progressbar = dialog.getByRole("progressbar");
  await expect(dialog).toContainText("Preparing");
  await expect(progressbar).not.toHaveAttribute("aria-valuenow");
  await expect(dialog).toContainText("Removing cards");
  await expect(dialog).toContainText("0 / 3,000");
  await expect(dialog).toContainText("3,000 / 3,000");
  await expect(dialog).toContainText("Cleaning audio");
  await expect(dialog).toContainText("2,999 / 2,999");
  await expect(dialog).toContainText("Finalizing");
  await expect(progressbar).not.toHaveAttribute("aria-valuenow");
  await expect(page.getByTestId("deck-travel-deck")).toHaveCount(0);
});

test("preserves queue, session, Today selection, and deck after a pre-commit failure", async ({
  page,
}) => {
  await seedStudyState(page, "travel-deck", "travel-deck");
  const queueBefore = await page.evaluate(() =>
    localStorage.getItem("meiki-active-study-queue"),
  );
  const sessionBefore = await page.evaluate(() =>
    sessionStorage.getItem("meiki-active-study-session"),
  );
  await page.goto("/?deckDeletion=precommit-failure");
  await openDecks(page);
  await expectDeckSelectionControls(page, 1);
  await selectDeck(page, "Travel phrases");
  await openDeckDeleteAction(page, "travel-deck", "Travel phrases");
  await page
    .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
    .getByRole("button", { name: "Delete deck" })
    .click();

  const failure = page.getByRole("dialog", { name: "Deck was not deleted" });
  await expect(failure).toContainText("Could not delete the deck. Try again.");
  await expect(failure).not.toContainText("raw fixture id");
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
  await expect(page.getByTestId("deck-selection-count")).toContainText(
    "1 deck selected",
  );
  await expect(
    page.getByRole("checkbox", { name: "Select Travel phrases" }),
  ).toHaveAttribute("aria-checked", "true");
  expect(await deleteDeckRequestCount(page)).toBe(1);
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-active-study-queue")),
  ).toBe(queueBefore);
  expect(
    await page.evaluate(() =>
      sessionStorage.getItem("meiki-active-study-session"),
    ),
  ).toBe(sessionBefore);
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-today-deck")),
  ).toBe("travel-deck");
});

test("refreshes the deleted deck while preserving the post-commit cleanup warning", async ({
  page,
}) => {
  await page.goto("/?deckDeletion=postcommit-failure");
  await openDecks(page);
  await openDeckDeleteAction(page, "travel-deck", "Travel phrases");
  await page
    .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
    .getByRole("button", { name: "Delete deck" })
    .click();

  const warning = page.getByRole("dialog", { name: "Deck deleted" });
  await expect(warning).toContainText(
    "Deck deleted, but some unused audio could not be cleaned up.",
  );
  await expect(page.getByTestId("deck-travel-deck")).toHaveCount(0);
  await warning.getByRole("button", { name: "Close" }).last().click();
  await expect(
    page.getByRole("heading", { name: "Decks", level: 1 }),
  ).toBeVisible();
  expect(await deleteDeckRequestCount(page)).toBe(1);
});

test("clears only the deleted deck's focused queue and resets its Today selection", async ({
  page,
}) => {
  await seedStudyState(page, "travel-deck", "travel-deck");
  await page.goto("/");
  await openDecks(page);
  await openDeckDeleteAction(page, "travel-deck", "Travel phrases");
  await page
    .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
    .getByRole("button", { name: "Delete deck" })
    .click();

  expect(
    await page.evaluate(() => localStorage.getItem("meiki-active-study-queue")),
  ).toBeNull();
  expect(
    await page.evaluate(() =>
      sessionStorage.getItem("meiki-active-study-session"),
    ),
  ).toBeNull();
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-today-deck")),
  ).toBe("__all_decks__");
});

for (const preservedQueue of ["__all_decks__", "default-deck"] as const) {
  test(`preserves the ${preservedQueue} queue and unrelated Today state`, async ({
    page,
  }) => {
    await seedStudyState(page, preservedQueue, "default-deck");
    await page.goto("/");
    await openDecks(page);
    await openDeckDeleteAction(page, "travel-deck", "Travel phrases");
    await page
      .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
      .getByRole("button", { name: "Delete deck" })
      .click();

    expect(
      await page.evaluate(() =>
        JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
      ),
    ).toMatchObject({ deckId: preservedQueue, position: 0 });
    expect(
      await page.evaluate(() =>
        sessionStorage.getItem("meiki-active-study-session"),
      ),
    ).toBe(`session for ${preservedQueue}`);
    expect(
      await page.evaluate(() => localStorage.getItem("meiki-today-deck")),
    ).toBe("default-deck");
  });
}

test("hides the empty internal default deck", async ({ page }) => {
  await page.goto("/?decks=empty-default");
  await openDecks(page);

  await expect(page.getByTestId("deck-default-deck")).toHaveCount(0);
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
});

test("offers hidden empty Unsorted as the direct deletion destination", async ({
  page,
}) => {
  await page.goto("/?decks=empty-default");
  await openDecks(page);
  await expect(page.getByTestId("deck-default-deck")).toHaveCount(0);
  await expect(page.locator(".deck-grid").getByTestId(/^deck-/)).toHaveCount(1);
  await openDeckDeleteAction(page, "travel-deck", "Travel phrases");
  await page
    .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
    .getByRole("button", { name: "Move cards instead" })
    .click();

  const moveDialog = page.getByRole("dialog", { name: "Move cards instead" });
  const destination = moveDialog.getByLabel("Destination deck");
  await expect(destination.locator('option[value="default-deck"]')).toHaveText(
    "Unsorted",
  );
  await destination.selectOption("default-deck");
  await moveDialog
    .getByRole("button", { name: "Move cards and delete" })
    .click();

  await expect(page.getByTestId("deck-travel-deck")).toHaveCount(0);
  expect(await deleteDeckRequestCount(page)).toBe(1);
  expect((await lastRequest(page, "delete_deck"))?.args).toMatchObject({
    request: {
      deck_id: "travel-deck",
      move_cards_to_deck_id: "default-deck",
    },
  });
});

test("previews and adds the complete Japanese bundle in ordered progress stages", async ({
  page,
}) => {
  await openDecks(page);
  await page.getByRole("button", { name: "Import bundle" }).click();

  const dialog = page.getByRole("dialog", { name: "Import bundle" });
  await expect(dialog.getByText("Japanese", { exact: true })).toBeVisible();
  await expect(dialog.getByText("9,700", { exact: true })).toHaveCount(2);
  const bundleDecks = dialog.getByRole("list", { name: "Bundle decks" });
  await expect(bundleDecks.getByRole("listitem")).toHaveCount(6);
  await expect(bundleDecks.getByRole("listitem").nth(0)).toContainText(
    /Japanese 00 — Kana, sound, and Japanese input\s+300\s+cards\s+Will add/,
  );
  await expect(bundleDecks.getByRole("listitem").nth(5)).toContainText(
    /Japanese 05 — N1 \/ balanced C1 bridge\s+3,000\s+cards\s+Will add/,
  );

  await dialog.getByRole("button", { name: "Add bundle" }).click();
  await expect(
    dialog.getByText("Preparing decks", { exact: true }),
  ).toBeVisible();
  await expect(dialog.getByText("Adding cards", { exact: true })).toBeVisible();
  await expect(dialog.getByText("Adding audio", { exact: true })).toBeVisible();

  await expect(
    page
      .getByTestId("bundle-import-activity")
      .getByText("Added Japanese with 6 decks."),
  ).toBeVisible();
  const stage = page.getByTestId("deck-deck:ja-JP:05");
  await expect(stage).toContainText(/3000\s*cards/);
  await stage.getByRole("button", { name: "Study" }).click();
  expect((await lastRequest(page, "prepare_study"))?.args).toMatchObject({
    request: { deck_id: "deck:ja-JP:05" },
  });
});

test("marks installed bundle decks and disables an already installed bundle", async ({
  page,
}) => {
  await page.goto("/?bundle=partial");
  await openDecks(page);
  await page.getByRole("button", { name: "Import bundle" }).click();
  let dialog = page.getByRole("dialog", { name: "Import bundle" });
  await expect(dialog.getByText("Installed", { exact: true })).toHaveCount(2);
  await expect(dialog.getByText("Will add", { exact: true })).toHaveCount(4);

  await page.keyboard.press("Escape");
  await page.goto("/?bundle=installed");
  await openDecks(page);
  await page.getByRole("button", { name: "Import bundle" }).click();
  dialog = page.getByRole("dialog", { name: "Import bundle" });
  await expect(
    dialog.getByText("Japanese is already installed", { exact: true }),
  ).toBeVisible();
  await expect(
    dialog.getByRole("button", { name: "Add bundle" }),
  ).toBeDisabled();
});

test("reports installation when existing decks only need bundle associations", async ({
  page,
}) => {
  await page.goto("/?bundle=unassociated");
  await openDecks(page);
  await page.getByRole("button", { name: "Import bundle" }).click();

  const dialog = page.getByRole("dialog", { name: "Import bundle" });
  await expect(dialog.getByText("Installed", { exact: true })).toHaveCount(6);
  await expect(
    dialog.getByRole("button", { name: "Add bundle" }),
  ).toBeEnabled();
  await dialog.getByRole("button", { name: "Add bundle" }).click();

  await expect(
    page
      .getByTestId("bundle-import-activity")
      .getByText("Japanese is now installed."),
  ).toBeVisible();
  await expect(page.getByText(/Added Japanese with 0 decks/)).toHaveCount(0);
});

test("removes an installed bundle after one confirmation and leaves unrelated decks", async ({
  page,
}) => {
  await page.goto("/?bundleRemoval=installed");
  await openDecks(page);
  await page.getByRole("button", { name: "Bundle actions" }).click();
  const actions = page.getByRole("dialog", { name: "Bundle actions" });
  const removeJapanese = actions.getByRole("button", {
    name: /Remove Japanese/,
  });
  await expect(removeJapanese).toContainText(/6\s*decks, 9,700\s*cards/);
  await removeJapanese.click();

  const confirmation = page.getByRole("alertdialog", {
    name: "Remove Japanese?",
  });
  await expect(confirmation).toContainText(
    /This permanently removes bundled content from 6 decks\. Personal cards in those decks move to Trash\./,
  );
  await confirmation.getByRole("button", { name: "Cancel" }).click();
  await expect(lastRequest(page, "remove_bundle")).resolves.toBeUndefined();

  await confirmJapaneseBundleRemoval(page);

  const progress = page.getByRole("dialog", { name: "Removing Japanese" });
  await expect(progress.getByRole("status")).toContainText(
    /Removing cards\s*300 \/ 9,700/,
  );
  await expect(progress.getByRole("progressbar")).toHaveAttribute(
    "aria-valuemax",
    "9700",
  );
  await expect(
    page
      .getByTestId("deletion-activity")
      .getByText("Removed Japanese with 6 decks."),
  ).toBeVisible();
  await expect(page.getByTestId("deck-deck:ja-JP:05")).toHaveCount(0);
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Bundle actions" }),
  ).toHaveCount(0);
  expect((await lastRequest(page, "remove_bundle"))?.args).toMatchObject({
    request: {
      language_tag: "ja-JP",
      expected_decks: 6,
      expected_cards: 9_700,
    },
  });
});

test("exports an installed bundle from its language actions", async ({
  page,
}) => {
  await page.goto("/?bundleRemoval=installed");
  await openDecks(page);
  await page.getByRole("button", { name: "Bundle actions" }).click();
  await page
    .getByRole("dialog", { name: "Bundle actions" })
    .getByRole("button", { name: "Export Japanese" })
    .click();

  await expect(
    page.getByText(
      "Exported Japanese with 6 decks and 9,700 cards to /tmp/exports/meiki-bundle-e2e.meiki.",
    ),
  ).toBeVisible();
  expect((await lastRequest(page, "export_bundle"))?.args).toMatchObject({
    request: { language_tag: "ja-JP" },
  });
});

test("preserves an all-decks study queue when its bundle decks are removed", async ({
  page,
}) => {
  await page.evaluate(() => {
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
  await page.goto("/?bundleRemoval=installed");
  await openDecks(page);
  await expect(page.getByText(/A saved session is active/)).toBeVisible();

  await confirmJapaneseBundleRemoval(page);
  await expect(page.getByTestId("deletion-activity")).toContainText(
    "Removed Japanese with 6 decks.",
  );
  await expect(page.getByText(/A saved session is active/)).toBeVisible();
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({ deckId: "__all_decks__", position: 0 });
});

test("resets a removed Today deck selection to All decks", async ({ page }) => {
  await page.goto("/?bundleRemoval=installed");
  await openDecks(page);
  await page.evaluate(() => {
    localStorage.setItem("meiki-today-deck", "deck:ja-JP:05");
  });

  await confirmJapaneseBundleRemoval(page);
  await expect(page.getByTestId("deletion-activity")).toContainText(
    "Removed Japanese with 6 decks.",
  );
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-today-deck")),
  ).toBe("__all_decks__");

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
  expect((await lastRequest(page, "get_today_overview"))?.args).toMatchObject({
    request: { deck_id: "__all_decks__" },
  });
});

test("keeps Unsorted visible when active cards exist but hides rename and delete", async ({
  page,
}) => {
  await openDecks(page);
  await page
    .getByTestId("deck-default-deck")
    .getByRole("button", { name: "Open" })
    .click();

  await expect(
    page.getByRole("heading", { name: "Unsorted", level: 1 }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Rename deck" })).toHaveCount(
    0,
  );
  await expect(page.getByRole("button", { name: "Delete deck" })).toHaveCount(
    0,
  );
  await expect(
    page.getByRole("button", { name: "Daily time", exact: true }),
  ).toHaveCount(0);
});

test("creates a deck from its name only", async ({ page }) => {
  await page.goto("/?decks=lifecycle");
  await openDecks(page);
  await page.getByRole("button", { name: "New deck" }).click();
  const dialog = page.getByRole("dialog", { name: "New deck" });
  await expect(dialog.getByRole("textbox")).toHaveCount(1);
  await dialog.getByLabel("Name").fill(" Listening ");
  await dialog.getByRole("button", { name: "Create deck" }).click();

  await expect(page.getByText("Created deck “Listening”.")).toBeVisible();
  await expect(page.getByTestId("deck-listening-deck")).toBeVisible();
  expect((await lastRequest(page, "create_deck"))?.args).toMatchObject({
    request: { name: " Listening " },
  });
});

test("starts and resumes a study queue restricted to one deck", async ({
  page,
}) => {
  await openDecks(page);
  const travelDeck = page.getByTestId("deck-travel-deck");
  await travelDeck.getByRole("button", { name: "Study" }).click();
  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
  expect((await lastRequest(page, "prepare_study"))?.args).toMatchObject({
    request: { deck_id: "travel-deck" },
  });

  await openDecks(page);
  await expect(
    page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Resume" }),
  ).toBeVisible();
  await selectDeckView(page, "List");
  await expect(
    page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Resume" }),
  ).toBeVisible();
  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Resume" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
});

test("replaces a bundle-stage queue while preserving its completed review", async ({
  page,
}) => {
  await page.goto("/?bundleRemoval=installed");
  await openDecks(page);
  const stage00 = page.getByTestId("deck-deck:ja-JP:00");
  const stage01 = page.getByTestId("deck-deck:ja-JP:01");
  const stage02 = page.getByTestId("deck-deck:ja-JP:02");
  await stage00.getByRole("button", { name: "Study" }).click();
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.getByRole("button", { name: /Good/ }).click();
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByTestId("review-saved-status")).toBeVisible();
  await page.getByLabel("Your answer").fill("unfinished response");

  await openDecks(page);
  await page.evaluate(() => {
    sessionStorage.setItem(
      "meiki-active-study-session",
      "abandoned bundle-stage session",
    );
  });
  await expect(stage00.getByRole("button", { name: "Resume" })).toBeEnabled();
  await expect(stage01.getByRole("button", { name: "Study" })).toBeEnabled();
  await expect(stage02.getByRole("button", { name: "Study" })).toBeEnabled();
  await stage02.getByRole("button", { name: "Study" }).click();

  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
  await expect(page.getByLabel("Your answer")).toHaveValue("");
  expect(
    await page.evaluate(() =>
      sessionStorage.getItem("meiki-active-study-session"),
    ),
  ).toBeNull();
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({ deckId: "deck:ja-JP:02", position: 0 });
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-e2e-committed-reviews") ?? "[]"),
    ),
  ).toEqual([
    expect.objectContaining({
      card_id: "due-card",
      chosen_grade: "good",
      schedule_version: 1,
    }),
  ]);
  expect(
    await page.evaluate(
      () =>
        (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
          (request) => request.command === "grade_review",
        ).length,
    ),
  ).toBe(1);
});

test("keeps only an empty deck disabled while another queue is saved", async ({
  page,
}) => {
  await page.goto("/?bundleRemoval=installed&emptyDeck=default-deck");
  await openDecks(page);
  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Study" })
    .click();
  await openDecks(page);

  await expect(
    page
      .getByTestId("deck-travel-deck")
      .getByRole("button", { name: "Resume" }),
  ).toBeEnabled();
  await expect(
    page
      .getByTestId("deck-default-deck")
      .getByRole("button", { name: "Study" }),
  ).toBeDisabled();
  await expect(
    page
      .getByTestId("deck-deck:ja-JP:01")
      .getByRole("button", { name: "Study" }),
  ).toBeEnabled();
});

test("manages deck identity and daily time without Settings deck controls", async ({
  page,
}) => {
  await page.goto("/?decks=lifecycle");
  await openDecks(page);
  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Open" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Travel phrases", level: 1 }),
  ).toBeVisible();
  expect((await lastRequest(page, "get_deck_cards"))?.args).toMatchObject({
    request: { deck_id: "travel-deck" },
  });
  await page.getByRole("button", { name: "Add card" }).click();
  await expect(page.getByLabel("Deck")).toHaveValue("travel-deck");
  await page.getByRole("button", { name: "Cancel" }).click();

  await page.getByRole("button", { name: "Rename deck" }).click();
  const renameDialog = page.getByRole("dialog", { name: "Rename deck" });
  await renameDialog.getByLabel("Name").fill("Audio");
  await renameDialog.getByRole("button", { name: "Rename deck" }).click();
  await expect(
    page.getByRole("heading", { name: "Audio", level: 1 }),
  ).toBeVisible();
  expect((await lastRequest(page, "rename_deck"))?.args).toMatchObject({
    request: { deck_id: "travel-deck", name: "Audio" },
  });

  await page.getByRole("button", { name: "Daily time", exact: true }).click();
  const timeDialog = page.getByRole("dialog", { name: "Daily time for Audio" });
  await timeDialog.getByRole("switch").click();
  await timeDialog.getByLabel("Minutes per day").fill("45");
  await timeDialog.getByRole("button", { name: "Save daily time" }).click();
  expect(
    (await lastRequest(page, "update_scheduler_settings"))?.args,
  ).toMatchObject({
    request: {
      deck_id: "travel-deck",
      deck_daily_time_budget_minutes: 45,
    },
  });

  await page.getByRole("button", { name: "Daily time", exact: true }).click();
  await page
    .getByRole("dialog", { name: "Daily time for Audio" })
    .getByRole("switch")
    .click();
  await page.getByRole("button", { name: "Save daily time" }).click();
  expect(
    (await lastRequest(page, "update_scheduler_settings"))?.args,
  ).toMatchObject({
    request: {
      deck_id: "travel-deck",
      deck_daily_time_budget_minutes: null,
    },
  });

  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await expect(page.getByLabel("Deck to configure")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Rename deck" })).toHaveCount(
    0,
  );
  await expect(page.getByRole("button", { name: "Delete deck" })).toHaveCount(
    0,
  );
  await expect(page.getByText("Override for this deck")).toHaveCount(0);
});

test("collection Settings ignores and clears a legacy Unsorted time override", async ({
  page,
}) => {
  await page.goto("/?settings=legacy-default-override");
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Save preferences" }),
  ).toBeEnabled();

  expect(
    (await lastRequest(page, "preview_scheduler_policy"))?.args,
  ).toMatchObject({
    request: {
      deck_id: "default-deck",
      deck_daily_time_budget_minutes: null,
    },
  });
  await expect(page.getByText(/Collection budget/).first()).toBeVisible();
  await page.getByRole("button", { name: "Save preferences" }).click();
  expect(
    (await lastRequest(page, "update_scheduler_settings"))?.args,
  ).toMatchObject({
    request: {
      deck_id: "default-deck",
      deck_daily_time_budget_minutes: null,
    },
  });
});
