import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Add / Edit" }).click();
  await expect(
    page.getByRole("heading", { name: "Add / Edit", level: 1 }),
  ).toBeVisible();
});

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

test("authors multiple stable Japanese clozes, previews each, and saves", async ({
  page,
}) => {
  const source = await sourceTextarea(page);
  await source.fill("日曜日は図書館に行きます");
  await selectRange(page, 4, 7);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await expect(page.getByRole("button", { name: /Cloze 1/ })).toContainText(
    "図書館",
  );

  await selectRange(page, 1, 5);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await expect(page.getByRole("button", { name: /Cloze 2/ })).toContainText(
    "行きます",
  );

  await page.getByLabel("Accepted answers").fill("ゆきます\n行く");
  await page.getByLabel("Hint").fill("polite present");
  await page.getByLabel("Answer matching").selectOption("forgiving");
  await page.getByLabel("Explanation").fill("**Use** the polite present form.");
  await page.getByRole("button", { name: "Add annotation" }).click();
  await page.getByLabel("Annotation 1 label").fill("Reading");
  await page.getByLabel("Annotation 1 value").fill("いきます");

  await page.getByRole("button", { name: "Preview" }).click();
  const dialog = page.getByRole("dialog", { name: "Card preview" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("日曜日は図書館に[…]")).toBeVisible();
  await expect(dialog.getByText("Use", { exact: true })).toHaveCSS(
    "font-weight",
    /^(700|bold)$/,
  );
  await dialog.getByRole("tab", { name: "Card 1" }).click();
  await expect(dialog.getByText("日曜日は[…]に行きます")).toBeVisible();
  await dialog.getByRole("button", { name: "Done" }).click();

  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(
    page.getByText("Source note saved on this device."),
  ).toBeVisible();
});

const multilingualCases = [
  { text: "أنا أقرأ كتابًا", answer: "كتابًا", direction: "rtl" },
  { text: "मैं पुस्तक पढ़ता हूँ", answer: "पुस्तक", direction: "ltr" },
  { text: "Réviser le café", answer: "café", direction: "ltr" },
  { text: "学习漢字", answer: "漢字", direction: "auto" },
  { text: "Meetingは الساعة 三時", answer: "三時", direction: "auto" },
] as const;

for (const example of multilingualCases) {
  test(`authors a grapheme-safe ${example.direction} cloze: ${example.answer}`, async ({
    page,
  }) => {
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
    const prompt = page.getByRole("dialog").locator(".dialog-prompt");
    await expect(prompt).toHaveAttribute("dir", example.direction);
    await expect(
      page.getByRole("dialog").locator(".preview-shell"),
    ).toHaveAttribute("dir", "ltr");
  });
}

test("rejects a UTF-16 range that splits a combining grapheme", async ({
  page,
}) => {
  const source = await sourceTextarea(page);
  await source.fill("e\u0301clair");
  await selectRange(page, 0, 1);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "splits an extended grapheme cluster",
  );
  await expect(page.getByRole("button", { name: /Cloze/ })).toHaveCount(0);
});

test("raw executable HTML is not saved", async ({ page }) => {
  const source = await sourceTextarea(page);
  await source.fill("Remember café");
  await selectRange(page, 9, 13);
  await page.getByRole("button", { name: "Make cloze" }).click();
  await page.getByLabel("Explanation").fill("<script>alert(1)</script>");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "not raw HTML or executable links",
  );
});

test("unsaved navigation never interrupts active IME composition", async ({
  page,
}) => {
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
