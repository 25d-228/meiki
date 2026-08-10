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

test("opens each deletable deck's actions by keyboard and keeps Unsorted non-deletable", async ({
  page,
}) => {
  await openDecks(page);

  const actions = page.getByRole("button", {
    name: "Actions for Travel phrases",
  });
  await actions.focus();
  await page.keyboard.press("Enter");
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
  await expect(page.getByText("Deleted Travel phrases.")).toBeVisible();
  expect(await deleteDeckRequestCount(page)).toBe(1);
  expect((await lastRequest(page, "delete_deck"))?.args).toMatchObject({
    request: {
      deck_id: "travel-deck",
      move_cards_to_deck_id: null,
      confirmation: "Travel phrases",
    },
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
  await openDeckDeleteAction(page, "travel-deck", "Travel phrases");
  await page
    .getByRole("alertdialog", { name: "Delete “Travel phrases”?" })
    .getByRole("button", { name: "Delete deck" })
    .click();

  const failure = page.getByRole("dialog", { name: "Deck was not deleted" });
  await expect(failure).toContainText("Could not delete the deck. Try again.");
  await expect(failure).not.toContainText("raw fixture id");
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
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
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Continue" }).click();
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
