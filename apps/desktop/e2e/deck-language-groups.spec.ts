import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { expandDeckSection } from "./support/deck-sections";
import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function navigate(page: Page, screen: string): Promise<void> {
  const menu = page.getByRole("button", { name: "Open navigation" });
  if (await menu.isVisible()) await menu.click();
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: screen, exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: screen, level: 1 }),
  ).toBeVisible();
}

for (const view of ["Grid", "List"] as const) {
  test(`${view} groups persisted languages once, starts collapsed, and changes presentation without requests`, async ({
    page,
  }) => {
    const runtimeRequests: string[] = [];
    page.on("request", (request) => {
      if (["fetch", "xhr"].includes(request.resourceType()))
        runtimeRequests.push(request.url());
    });
    await page.goto("/?decks=grouped");
    await navigate(page, "Decks");
    const headers = page.locator("[data-language-disclosure]");
    await expect(headers).toHaveCount(4);
    for (const [index, name] of [
      "French, 1 deck",
      "Spanish, 2 decks",
      "zz, 1 deck",
      "Other decks, 3 decks",
    ].entries()) {
      await expect(headers.nth(index)).toHaveAccessibleName(name);
    }
    const requests = await page.evaluate(
      () => window.__MEIKI_TEST_REQUESTS__?.length,
    );
    await page
      .getByRole("group", { name: "Deck view" })
      .getByRole("button", { name: view })
      .click();
    for (const header of await headers.all())
      await expect(header).toHaveAttribute("aria-expanded", "false");
    await expect(page.locator("[data-vim-deck-item]")).toHaveCount(1);
    await expect(page.getByTestId("deck-default-deck")).toBeVisible();
    await expect(page.getByRole("checkbox")).toHaveCount(0);

    await expandDeckSection(page, "es");
    await expect(page.getByTestId("deck-travel-deck")).toContainText(
      "Renamed Travel",
    );
    await expect(page.getByTestId("deck-listening-deck")).toBeVisible();
    await expect(page.getByTestId("deck-archive-deck")).toHaveCount(0);
    await expandDeckSection(page, "fr");
    await expandDeckSection(page, "zz");
    await expandDeckSection(page, "other");
    await expect(page.locator("[data-vim-deck-item]")).toHaveCount(8);
    expect(
      await page
        .locator("[data-vim-deck-item]")
        .evaluateAll((items) =>
          items.map((item) => item.getAttribute("data-vim-deck-id")),
        ),
    ).toEqual([
      "default-deck",
      "archive-deck",
      "travel-deck",
      "listening-deck",
      "unknown-deck",
      "missing-deck",
      "invalid-deck",
      "undetermined-deck",
    ]);
    expect(
      await page.evaluate(() => window.__MEIKI_TEST_REQUESTS__?.length),
    ).toBe(requests);
    expect(runtimeRequests).toEqual([]);
  });

  test(`${view} collapsing a section clears only its selected IDs before atomic deletion`, async ({
    page,
  }) => {
    await page.goto("/?decks=grouped");
    await navigate(page, "Decks");
    await page
      .getByRole("group", { name: "Deck view" })
      .getByRole("button", { name: view })
      .click();
    await expandDeckSection(page, "es");
    await expandDeckSection(page, "fr");
    await page.getByRole("checkbox", { name: "Select Renamed Travel" }).click();
    await page
      .getByRole("checkbox", { name: "Select French practice" })
      .click();
    await page.locator('[data-language-disclosure="es"]').click();
    await expect(page.getByTestId("deck-selection-count")).toContainText(
      "1 deck selected",
    );
    await expect(
      page.getByRole("checkbox", { name: "Select French practice" }),
    ).toHaveAttribute("aria-checked", "true");
    await expandDeckSection(page, "es");
    await expect(
      page.getByRole("checkbox", { name: "Select Renamed Travel" }),
    ).toHaveAttribute("aria-checked", "false");
    await page
      .getByRole("button", { name: "Delete selected", exact: true })
      .click();
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "Delete selected", exact: true })
      .click();
    await expect(page.getByTestId("deck-archive-deck")).toHaveCount(0);
    await expect(page.locator('[data-language-disclosure="fr"]')).toHaveCount(
      0,
    );
    await expect(
      page.locator('[data-language-disclosure="es"]'),
    ).toHaveAttribute("aria-expanded", "true");
    const requests = await page.evaluate(() =>
      window.__MEIKI_TEST_REQUESTS__?.filter(
        (request) => request.command === "delete_decks",
      ),
    );
    expect(requests).toHaveLength(1);
    expect(requests?.[0].args).toMatchObject({
      request: { deck_ids: ["archive-deck"] },
    });
  });
}

test("expansion persists through views, opened decks, primary navigation, and reload", async ({
  page,
}) => {
  await page.goto("/?decks=grouped");
  await navigate(page, "Decks");
  await expandDeckSection(page, "es");
  await page
    .getByRole("group", { name: "Deck view" })
    .getByRole("button", { name: "List" })
    .click();
  await page
    .getByTestId("deck-travel-deck")
    .getByRole("button", { name: "Open", exact: true })
    .click();
  await navigate(page, "Decks");
  await expect(page.getByTestId("deck-travel-deck")).toBeVisible();
  await navigate(page, "Settings");
  await navigate(page, "Decks");
  await page.reload();
  await navigate(page, "Decks");
  await expect(page.locator('[data-language-disclosure="es"]')).toHaveAttribute(
    "aria-expanded",
    "true",
  );
  await expect(page.locator('[data-language-disclosure="fr"]')).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  await expect(
    page.getByRole("button", { name: "List", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(
    await page.evaluate(() =>
      JSON.parse(
        localStorage.getItem("meiki-decks-expanded-languages") ?? "null",
      ),
    ),
  ).toEqual(["es"]);
});

for (const value of ["{broken", '"es"', '["es", 3]']) {
  test(`invalid expansion preference ${value} safely loads collapsed sections`, async ({
    page,
  }) => {
    await page.addInitScript(
      (saved) => localStorage.setItem("meiki-decks-expanded-languages", saved),
      value,
    );
    await page.goto("/?decks=grouped");
    await navigate(page, "Decks");
    await expect(
      page.locator('[data-language-disclosure="es"]'),
    ).toHaveAttribute("aria-expanded", "false");
    await expect(
      page.locator('[data-language-disclosure="other"]'),
    ).toHaveAttribute("aria-expanded", "false");
  });
}

for (const view of ["Grid", "List"] as const) {
  test(`${view} visible-only rectangle selection crosses sections and folding cancels its geometry`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1440, height: 1400 });
    await page.goto("/?decks=grouped");
    await navigate(page, "Decks");
    await page.getByRole("button", { name: view, exact: true }).click();
    await expandDeckSection(page, "fr");
    await expandDeckSection(page, "es");
    const first = await page.getByTestId("deck-archive-deck").boundingBox();
    const last = await page.getByTestId("deck-listening-deck").boundingBox();
    if (!first || !last) throw new Error("Expanded deck geometry missing");
    await page.mouse.move(first.x + 2, first.y + 2);
    await page.mouse.down();
    await page.mouse.move(last.x + last.width - 4, last.y + last.height - 2, {
      steps: 4,
    });
    await expect(page.getByRole("checkbox", { checked: true })).toHaveCount(3);
    await expect(page.getByTestId("deck-unknown-deck")).toHaveCount(0);
    // A keyboard fold while the pointer is held must stop the drag before the rows disappear.
    await page.locator('[data-language-disclosure="es"]').focus();
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("deck-selection-rectangle")).toHaveCount(0);
    await page.mouse.up();
    await expect(page.getByRole("checkbox", { checked: true })).toHaveCount(1);
  });
}

test("disclosure ownership and Vim traversal exclude collapsed decks and repair hidden focus", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem("meiki-vim-keybindings", "true"),
  );
  await page.goto("/?decks=grouped");
  await navigate(page, "Decks");
  const spanish = page.locator('[data-language-disclosure="es"]');
  await spanish.focus();
  await page.keyboard.press("Space");
  await expect(spanish).toBeFocused();
  await expect(spanish).toHaveAttribute("aria-expanded", "true");
  await page.locator("#main-content").focus();
  await page.keyboard.press("j");
  await expect(page.getByTestId("deck-travel-deck")).toBeFocused();
  await page.keyboard.press("j");
  await expect(page.getByTestId("deck-listening-deck")).toBeFocused();
  await spanish.focus();
  await page.keyboard.press("Enter");
  await expect(spanish).toBeFocused();
  await page.keyboard.press("o");
  await expect(
    page.getByRole("heading", { name: "Decks", level: 1 }),
  ).toBeVisible();
  await page.keyboard.press("Tab");
  await expect(page.locator('[data-language-disclosure="zz"]')).toBeFocused();
  await page.locator("#main-content").focus();
  await page.keyboard.press("j");
  await expect(page.getByTestId("deck-default-deck")).toBeFocused();
  expect(
    await page.evaluate(() =>
      window.__MEIKI_TEST_REQUESTS__?.filter((request) =>
        ["get_deck_cards", "prepare_study"].includes(request.command),
      ),
    ),
  ).toEqual([]);
});

for (const layout of [
  { width: 1440, height: 900, theme: "light" },
  { width: 640, height: 720, theme: "dark" },
] as const) {
  test(`grouped Decks is accessible and contained at ${layout.width}px in ${layout.theme}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: layout.width, height: layout.height });
    await page.emulateMedia({ colorScheme: layout.theme });
    await page.goto("/?decks=grouped");
    await navigate(page, "Decks");
    for (const view of ["Grid", "List"] as const) {
      await page.getByRole("button", { name: view, exact: true }).click();
      await expandDeckSection(page, "es");
      expect(
        (await new AxeBuilder({ page }).include("main").analyze()).violations,
      ).toEqual([]);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      const header = page.locator('[data-language-disclosure="es"]');
      const geometry = await header.evaluate((element) => {
        const outer = element.getBoundingClientRect();
        return [...element.children].every((child) => {
          const bounds = child.getBoundingClientRect();
          return bounds.left > outer.left && bounds.right < outer.right;
        });
      });
      expect(geometry).toBe(true);
    }
  });
}
