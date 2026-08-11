import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function enableVimKeybindings(page: Page): Promise<void> {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-vim-keybindings", "true");
  });
}

async function navigate(page: Page, screen: string): Promise<void> {
  const openNavigation = page.getByRole("button", { name: "Open navigation" });
  if (await openNavigation.isVisible()) await openNavigation.click();
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: screen, exact: true })
    .click();
}

async function openDecks(page: Page, route = "/"): Promise<void> {
  await page.goto(route);
  await navigate(page, "Decks");
  await expect(
    page.getByRole("heading", { name: "Decks", level: 1 }),
  ).toBeVisible();
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
}

async function openStudy(page: Page, route = "/"): Promise<void> {
  await page.goto(route);
  await page.getByRole("button", { name: /^(Start|Resume) study$/ }).click();
  await expect(page.getByLabel("Your answer")).toBeVisible();
}

async function requestCount(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (name) =>
      (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
        (request) => request.command === name,
      ).length,
    command,
  );
}

async function lastRequest(page: Page, command: string) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

test("Vim keybindings default Off, persist locally, and describe the enabled mapping", async ({
  page,
}) => {
  await page.goto("/");
  await navigate(page, "Settings");
  const toggle = page.getByRole("switch", { name: "Vim keybindings" });

  await expect(toggle).not.toBeChecked();
  await expect(page.getByTestId("vim-keybindings-summary")).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-vim-keybindings")),
  ).toBeNull();

  await toggle.click();
  await expect(toggle).toBeChecked();
  await expect(page.getByTestId("vim-keybindings-summary")).toContainText(
    /NORMAL.*INSERT/s,
  );
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-vim-keybindings")),
  ).toBe("true");

  await page.reload();
  await navigate(page, "Settings");
  await expect(
    page.getByRole("switch", { name: "Vim keybindings" }),
  ).toBeChecked();
});

test("disabled Vim commands leave Decks, Typing, and Study behavior unchanged", async ({
  page,
}) => {
  await openDecks(page);
  await page.locator("#main-content").focus();
  for (const key of ["j", "k", "o", "s", "x", "Enter"]) {
    await page.keyboard.press(key);
  }
  await expect(page.getByText(/selected$/)).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Decks", level: 1 }),
  ).toBeVisible();
  expect(await requestCount(page, "prepare_study")).toBe(0);

  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Open" })
    .click();
  await page.locator("#main-content").focus();
  for (const key of ["j", "k", "o", "Enter"]) {
    await page.keyboard.press(key);
  }
  await expect(
    page.getByRole("heading", { name: "Travel phrases", level: 1 }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Add / Edit card", level: 1 }),
  ).toHaveCount(0);

  await navigate(page, "Typing");
  await page.locator("#main-content").focus();
  for (const key of ["Enter", "h", "l", "r", "i"]) {
    await page.keyboard.press(key);
  }
  await expect(page.getByLabel("Practice input")).toHaveCount(0);
  await expect(page.getByLabel(/Vim mode/)).toHaveCount(0);
  await page.getByRole("button", { name: "Start practice" }).click();
  const practiceInput = page.getByLabel("Practice input");
  await page.locator("#main-content").focus();
  for (const key of ["h", "l", "r", "i"]) {
    await page.keyboard.press(key);
  }
  await expect(practiceInput).not.toBeFocused();
  await expect(
    page.getByRole("heading", { name: "Basic consonants", level: 2 }),
  ).toBeVisible();

  await navigate(page, "Today");
  await page.getByRole("button", { name: "Start study" }).click();
  const answer = page.getByLabel("Your answer");
  await answer.fill("行きます");
  await answer.press("Enter");
  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();
  await page.locator("#main-content").focus();
  for (const key of ["Enter", "r", "u", "i", "1", "2", "3", "4"]) {
    await page.keyboard.press(key);
  }
  expect(await requestCount(page, "grade_review")).toBe(0);
  expect(await requestCount(page, "undo_review")).toBe(0);
  await page.getByRole("button", { name: /^Good/ }).click();
  await page.keyboard.press("ControlOrMeta+z");
  await expect(page.getByText("Last review undone.")).toBeVisible();
});

test("Decks supports bounded roving commands in Grid and List without blocking rectangle selection", async ({
  page,
}) => {
  await enableVimKeybindings(page);
  await openDecks(page);
  const unsorted = page.getByTestId("deck-default-deck");
  const travel = page.getByTestId("deck-travel-deck");

  await expect(page.getByLabel("Vim mode NORMAL")).toHaveText("NORMAL");
  await page.locator("#main-content").focus();
  await page.keyboard.press("j");
  await expect(travel).toBeFocused();
  await page.keyboard.press("k");
  await expect(unsorted).toBeFocused();
  await page.keyboard.press("x");
  await expect(page.getByText(/selected$/)).toHaveCount(0);

  await page.keyboard.press("j");
  await page.keyboard.press("x");
  await expect(page.getByTestId("deck-selection-count")).toHaveText(
    "1 deck selected",
  );
  await page.getByRole("button", { name: "List" }).click();
  await expect(travel).toHaveAttribute("data-vim-focused", "true");
  await expect(travel).toHaveAttribute("tabindex", "0");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(travel).toBeFocused();

  await page.keyboard.press("Control+x");
  await page.keyboard.press("Meta+x");
  await page.keyboard.press("Alt+s");
  await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Decks", level: 1 }),
  ).toBeVisible();
  const shadowInputHost = await page.evaluateHandle(() => {
    const host = document.createElement("div");
    host.dataset.testid = "vim-shadow-input";
    const input = document.createElement("input");
    host.attachShadow({ mode: "open" }).append(input);
    document.querySelector("#main-content")?.append(host);
    input.focus();
    return host;
  });
  await page.keyboard.press("x");
  await expect(page.getByTestId("deck-selection-count")).toHaveCount(0);
  await shadowInputHost.evaluate((host) => host.remove());
  await page.locator("#main-content").focus();
  await page.keyboard.press("Escape");
  await expect(travel).toBeFocused();

  const bounds = await travel.boundingBox();
  if (!bounds) throw new Error("Travel deck geometry is unavailable");
  await page.mouse.move(bounds.x + 12, bounds.y + bounds.height / 2);
  await page.mouse.down();
  await page.mouse.move(bounds.x + 24, bounds.y + bounds.height / 2, {
    steps: 3,
  });
  await page.mouse.up();
  await expect(page.getByTestId("deck-selection-count")).toHaveText(
    "1 deck selected",
  );

  await travel
    .getByRole("button", { name: "Actions for Travel phrases" })
    .click();
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  await page.keyboard.press("o");
  await expect(menu).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menu).toBeHidden();
  await page.keyboard.press("o");
  await expect(
    page.getByRole("heading", { name: "Travel phrases", level: 1 }),
  ).toBeVisible();
});

test("Decks Vim study respects empty decks and uses the existing study flow", async ({
  page,
}) => {
  await enableVimKeybindings(page);
  await openDecks(page, "/?bundleRemoval=installed&emptyDeck=travel-deck");
  await page.locator("#main-content").focus();
  await page.keyboard.press("j");
  await page.keyboard.press("s");
  expect(await requestCount(page, "prepare_study")).toBe(0);

  await openDecks(page);
  await page.locator("#main-content").focus();
  await page.keyboard.press("j");
  await page.keyboard.press("s");
  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
});

test("opened-deck cards use roving edit commands while search and overlays retain input ownership", async ({
  page,
}) => {
  await enableVimKeybindings(page);
  await openDecks(page);
  await page.locator("#main-content").focus();
  await page.keyboard.press("j");
  await page.keyboard.press("o");

  const firstCard = page.getByTestId("card-card-ar");
  const secondCard = page.getByTestId("card-travel-new-card");
  await expect(page.getByLabel("Vim mode NORMAL")).toBeVisible();
  await page.locator("#main-content").focus();
  await page.keyboard.press("j");
  await expect(secondCard).toBeFocused();

  const search = page.getByRole("searchbox", { name: "Search cards" });
  await search.focus();
  await page.keyboard.press("k");
  await expect(search).toHaveValue("k");
  await expect(secondCard).toHaveAttribute("data-vim-focused", "true");
  await search.fill("");

  await firstCard.getByRole("button", { name: "Move", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Move card" });
  await page.keyboard.press("o");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Cancel" }).click();

  await expect(page.getByRole("checkbox")).toHaveCount(0);
  await page.locator("#main-content").focus();
  await page.keyboard.press("o");
  await expect(
    page.getByRole("heading", { name: "Add / Edit card", level: 1 }),
  ).toBeVisible();
  expect(
    (await lastRequest(page, "get_authoring_draft_for_card"))?.args,
  ).toMatchObject({ cardId: "travel-new-card" });
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Travel phrases", level: 1 }),
  ).toBeVisible();
  await page.locator("#main-content").focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Add / Edit card", level: 1 }),
  ).toBeVisible();
  expect(
    (await lastRequest(page, "get_authoring_draft_for_card"))?.args,
  ).toMatchObject({ cardId: "card-ar" });
});

test("Typing keeps NORMAL and INSERT synchronized across lesson, Retry, and IME-safe input actions", async ({
  page,
}) => {
  await enableVimKeybindings(page);
  await page.addInitScript(() => {
    localStorage.setItem(
      "meiki-typing-completed",
      JSON.stringify(["typing-korean-basic-consonants"]),
    );
  });
  await page.goto("/");
  await navigate(page, "Typing");
  const mode = page.getByRole("status", { name: /Vim mode/ });

  await expect(mode).toHaveAccessibleName("Vim mode NORMAL");
  await page.locator("#main-content").focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Basic consonants", level: 2 }),
  ).toBeVisible();
  await page.keyboard.press("l");
  await expect(
    page.getByRole("heading", { name: "Basic vowels", level: 2 }),
  ).toBeVisible();
  await page.keyboard.press("h");
  await expect(
    page.getByRole("heading", { name: "Basic consonants", level: 2 }),
  ).toBeVisible();

  await page.keyboard.press("r");
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "None yet",
  );
  await page.keyboard.press("i");
  const input = page.getByLabel("Practice input");
  await expect(input).toBeFocused();
  await expect(mode).toHaveAccessibleName("Vim mode INSERT");

  await input.dispatchEvent("compositionstart", { data: "ㅎ" });
  await input.dispatchEvent("keydown", {
    key: "Escape",
    code: "Escape",
    isComposing: true,
  });
  await expect(input).toBeFocused();
  await expect(mode).toHaveAccessibleName("Vim mode INSERT");
  await input.dispatchEvent("compositionend", { data: "ㅎ" });
  await input.press("Escape");
  await expect(input).not.toBeFocused();
  await expect(mode).toHaveAccessibleName("Vim mode NORMAL");

  await page.keyboard.press("h");
  await expect(
    page.getByRole("heading", { name: "Basic consonants", level: 2 }),
  ).toBeVisible();
});

test("Study supports safe mode changes, replay, suggested grading, continuation, and undo", async ({
  page,
}) => {
  await enableVimKeybindings(page);
  await page.addInitScript(() => {
    localStorage.setItem("meiki-autoplay-prompt-audio", "false");
  });
  await openStudy(page, "/?media=ready&reconcile=request");
  const mode = page.getByRole("status", { name: /Vim mode/ });
  const answer = page.getByLabel("Your answer");

  await expect(answer).toBeFocused();
  await expect(mode).toHaveAccessibleName("Vim mode INSERT");
  await answer.fill("行きます");
  await answer.dispatchEvent("compositionstart", { data: "行" });
  await answer.dispatchEvent("keydown", {
    key: "Escape",
    code: "Escape",
    isComposing: true,
  });
  await expect(answer).toBeFocused();
  await answer.dispatchEvent("compositionend", { data: "行" });
  await answer.press("Escape");
  await expect(mode).toHaveAccessibleName("Vim mode NORMAL");
  await page.keyboard.press("i");
  await expect(answer).toBeFocused();
  await answer.press("Escape");

  await page.keyboard.press("Enter");
  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();
  await page.keyboard.press("r");
  expect(
    await page.evaluate(() =>
      Number(localStorage.getItem("meiki-e2e-media-play-count") ?? "0"),
    ),
  ).toBe(1);
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
  await page.keyboard.press("u");
  await expect(page.getByText("Last review undone.")).toBeVisible();
  await expect(answer).toBeFocused();

  await answer.fill("行きます");
  await answer.press("Escape");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(page.getByText(/Second card ·/)).toBeVisible();
});

for (const [key, grade] of [
  ["1", "again"],
  ["2", "hard"],
  ["3", "good"],
  ["4", "easy"],
] as const) {
  test(`Study Vim ${key} submits the ${grade} grade`, async ({ page }) => {
    await enableVimKeybindings(page);
    await openStudy(page);
    const answer = page.getByLabel("Your answer");
    await answer.fill("行きます");
    await answer.press("Escape");
    await page.keyboard.press("Enter");
    await page.keyboard.press(key);
    await expect(
      page.getByRole("heading", { name: "Review saved" }),
    ).toBeVisible();
    expect((await lastRequest(page, "grade_review"))?.args).toMatchObject({
      request: { chosen_grade: grade },
    });
  });
}

test("enabled Vim surfaces remain accessible and avoid narrow overflow", async ({
  page,
}) => {
  await enableVimKeybindings(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await openDecks(page, "/?decks=long-name");
  await expect(page.getByLabel("Vim mode NORMAL")).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    ),
  ).toBe(false);
  await page.waitForTimeout(150);
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa"])
    .analyze();
  expect(results.violations).toEqual([]);
});
