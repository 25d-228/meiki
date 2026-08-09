import { expect, test } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
});

async function openDecks(page: import("@playwright/test").Page): Promise<void> {
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

test("hides the empty internal default deck", async ({ page }) => {
  await page.goto("/?decks=empty-default");
  await openDecks(page);

  await expect(page.getByTestId("deck-default-deck")).toHaveCount(0);
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
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

  const progress = page.getByRole("dialog", { name: "Removing bundle" });
  await expect(progress.getByRole("status")).toContainText(/Decks\s*1 \/ 6/);
  await expect(page.getByText("Removed Japanese with 6 decks.")).toBeVisible();
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
  await expect(page.getByText(/Removed Japanese/)).toBeVisible();
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
  await expect(page.getByText(/Removed Japanese/)).toBeVisible();
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-today-deck")),
  ).toBe("__all_decks__");

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
  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Resume" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Study", level: 1 }),
  ).toBeVisible();
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
