import { expect, type Page } from "@playwright/test";

export async function expandDeckSection(
  page: Page,
  key: string,
): Promise<void> {
  const disclosure = page.locator(`[data-language-disclosure="${key}"]`);
  await expect(disclosure).toBeVisible();
  if ((await disclosure.getAttribute("aria-expanded")) === "false") {
    await disclosure.click();
  }
  await expect(disclosure).toHaveAttribute("aria-expanded", "true");
}
