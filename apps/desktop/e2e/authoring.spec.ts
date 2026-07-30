import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function openEditor(page: Page, scenario = "cjk"): Promise<void> {
  await page.goto(`/?authoring=${scenario}`);
  await page.getByRole("button", { name: "Add / Edit" }).click();
  await expect(
    page.getByRole("heading", { name: "Add / Edit", level: 1 }),
  ).toBeVisible();
}

async function sourceTextarea(page: Page) {
  return page.locator(".segment-text").last();
}

async function selectRange(
  page: Page,
  start: number,
  end: number,
): Promise<void> {
  const textarea = await sourceTextarea(page);
  await textarea.evaluate(
    (element, range) => {
      const input = element as HTMLTextAreaElement;
      input.focus();
      input.setSelectionRange(range.start, range.end);
      input.dispatchEvent(new Event("select", { bubbles: true }));
    },
    { start, end },
  );
}

async function lastRequest(page: Page, command: string) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

test("maps a UTF-16 cloze request, renders its preview, and saves the edited DTO", async ({
  page,
}) => {
  await openEditor(page);
  const source = await sourceTextarea(page);
  await source.fill("日曜日は図書館に行きます");
  await selectRange(page, 4, 7);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await expect(page.getByRole("button", { name: /Cloze 1/ })).toContainText(
    "図書館",
  );
  expect((await lastRequest(page, "make_cloze"))?.args).toMatchObject({
    request: {
      segment_id: "segment-fixture",
      selection_start_utf16: 4,
      selection_end_utf16: 7,
    },
  });

  await page.getByRole("button", { name: "Optional cloze details" }).click();
  await page.getByLabel("Accepted answers").fill("ライブラリ");
  await page.getByLabel("Hint").fill("place for books");
  await page.getByLabel("Answer matching").selectOption("forgiving");
  await page.getByLabel("Explanation").fill("**Use** the fixture explanation.");
  await page.getByRole("button", { name: "Add annotation" }).click();
  await page.getByLabel("Annotation 1 label").fill("Reading");
  await page.getByLabel("Annotation 1 value").fill("としょかん");

  await page.getByRole("button", { name: "Preview" }).click();
  const dialog = page.getByRole("dialog", { name: "Card preview" });
  await expect(dialog.getByText("日曜日は[…]に行きます")).toBeVisible();
  await expect(dialog.getByText("Use", { exact: true })).toHaveCSS(
    "font-weight",
    /^(700|bold)$/,
  );
  await dialog.getByRole("button", { name: "Done" }).click();

  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(
    page.getByText("Source note saved on this device."),
  ).toBeVisible();
  expect((await lastRequest(page, "save_authoring_draft"))?.args).toMatchObject(
    {
      draft: {
        clozes: [
          {
            accepted_answers: ["ライブラリ"],
            hint: "place for books",
            matching_policy: "forgiving",
          },
        ],
      },
    },
  );
});

test("maps the selected authoring deck into the save request", async ({
  page,
}) => {
  await openEditor(page, "listening");
  await page.getByLabel("Author in deck").selectOption("travel-deck");
  const source = await sourceTextarea(page);
  await source.fill("Listen carefully");
  await selectRange(page, 0, 6);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await page.getByRole("button", { name: "Save", exact: true }).click();
  expect((await lastRequest(page, "save_authoring_draft"))?.args).toMatchObject(
    {
      draft: { deck_id: "travel-deck" },
    },
  );
});

test("maps role-specific file picks and retains edited media metadata", async ({
  page,
}) => {
  await openEditor(page, "media");
  const source = await sourceTextarea(page);
  await source.fill("図書館");
  await selectRange(page, 0, 3);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await page.getByRole("button", { name: "Local media" }).click();

  await page.getByRole("button", { name: "Add prompt audio" }).click();
  await page.getByRole("button", { name: "Add answer audio" }).click();
  await page.getByRole("button", { name: "Add reveal image" }).click();
  await expect(page.locator("audio")).toHaveCount(2);
  await expect(page.locator("img")).toHaveCount(1);
  await page
    .getByLabel("Reveal image alternative text")
    .fill("Library shelves with study tables");
  await page.getByRole("button", { name: "Save", exact: true }).click();

  const saved = (await lastRequest(page, "save_authoring_draft"))?.args as {
    draft: {
      clozes: Array<{
        media: Array<{ role: string; alt_text: string | null }>;
      }>;
    };
  };
  expect(saved.draft.clozes[0].media.map((item) => item.role)).toEqual([
    "prompt_audio",
    "answer_audio",
    "reveal_image",
  ]);
  expect(saved.draft.clozes[0].media[2].alt_text).toBe(
    "Library shelves with study tables",
  );
});

const multilingualCases = [
  {
    scenario: "rtl",
    text: "أنا أقرأ كتابًا",
    answer: "كتابًا",
    direction: "rtl",
  },
  {
    scenario: "devanagari",
    text: "मैं पुस्तक पढ़ता हूँ",
    answer: "पुस्तक",
    direction: "ltr",
  },
  {
    scenario: "ltr",
    text: "Réviser le café",
    answer: "café",
    direction: "ltr",
  },
  {
    scenario: "han",
    text: "学习漢字",
    answer: "漢字",
    direction: "auto",
  },
  {
    scenario: "mixed",
    text: "Meetingは الساعة 三時",
    answer: "三時",
    direction: "auto",
  },
] as const;

for (const example of multilingualCases) {
  test(`renders a ${example.direction} fixture after mapping its selection: ${example.answer}`, async ({
    page,
  }) => {
    await openEditor(page, example.scenario);
    const source = await sourceTextarea(page);
    await source.fill(example.text);
    await page.getByLabel("Direction").last().selectOption(example.direction);
    const start = example.text.indexOf(example.answer);
    await selectRange(page, start, start + example.answer.length);
    await page.getByRole("button", { name: "Make cloze" }).click();
    await expect(page.getByRole("button", { name: /Cloze 1/ })).toContainText(
      example.answer,
    );
    await page.getByRole("button", { name: "Preview" }).click();
    await expect(
      page.getByRole("dialog").locator(".dialog-prompt"),
    ).toHaveAttribute("dir", example.direction);
  });
}

test("renders application validation errors without implementing them in the browser", async ({
  page,
}) => {
  await openEditor(page, "boundary-error");
  const source = await sourceTextarea(page);
  await source.fill("e\u0301clair");
  await selectRange(page, 0, 1);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "splits an extended grapheme cluster",
  );

  await openEditor(page, "save-error");
  const nextSource = await sourceTextarea(page);
  await nextSource.fill("Remember café");
  await selectRange(page, 9, 13);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await page.getByRole("button", { name: "Optional cloze details" }).click();
  await page.getByLabel("Explanation").fill("<script>alert(1)</script>");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "not raw HTML or executable links",
  );
});

test("unsaved navigation never interrupts active IME composition", async ({
  page,
}) => {
  await openEditor(page);
  const source = await sourceTextarea(page);
  await source.fill("入力中");
  await source.dispatchEvent("compositionstart", { data: "中" });
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Add / Edit", level: 1 }),
  ).toBeVisible();

  await source.dispatchEvent("compositionend", { data: "中" });
  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("discard unsaved changes");
    await dialog.dismiss();
  });
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Add / Edit", level: 1 }),
  ).toBeVisible();
});
