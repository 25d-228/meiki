import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function openStudy(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await page.getByRole("button", { name: /^(Start|Resume) study$/ }).click();
}

async function lastRequest(page: Page, command: string) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
}

async function mediaPlayCount(page: Page): Promise<number> {
  return page.evaluate(() =>
    Number(localStorage.getItem("meiki-e2e-media-play-count") ?? "0"),
  );
}

test("renders empty and nothing-due DTO states with their UI actions", async ({
  page,
}) => {
  await openStudy(page, "/?today=empty&collection=empty");
  await expect(
    page.getByRole("heading", { name: "Your collection is empty" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Create a cloze" }),
  ).toBeVisible();

  await openStudy(page, "/?today=empty");
  await expect(
    page.getByRole("heading", { name: "Nothing is due" }),
  ).toBeVisible();
  await expect(page.getByText(/Your next review is due/)).toBeVisible();
});

test("maps answer and grade requests and renders returned DTOs", async ({
  page,
}) => {
  await openStudy(page, "/");
  await expect(page.getByText("日曜日は図書館に[…]")).toBeVisible();

  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await expect(page.getByText("Expected answer")).toBeVisible();
  await expect(page.getByText("exact", { exact: true })).toBeVisible();
  expect((await lastRequest(page, "check_answer"))?.args).toMatchObject({
    request: { card_id: "due-card", raw_response: "行きます" },
  });

  await page.keyboard.press("4");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
  const grade = await lastRequest(page, "grade_review");
  expect(grade?.args).toMatchObject({
    request: {
      card_id: "due-card",
      raw_response: "行きます",
      chosen_grade: "easy",
    },
  });
  expect(
    (grade?.args as { request: { response_duration_ms: number } }).request
      .response_duration_ms,
  ).toBeGreaterThanOrEqual(0);
});

test("autoplays only the first prompt clip by default and never answer audio", async ({
  page,
}) => {
  await openStudy(page, "/?media=multiple");
  await expect(
    page.getByRole("button", { name: "Autoplay on" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("audio")).toHaveCount(2);
  await expect(page.locator("audio").first()).not.toHaveAttribute("controls");
  await expect.poll(() => mediaPlayCount(page)).toBe(1);
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-e2e-media-played-roles") ?? "[]"),
    ),
  ).toEqual(["prompt_audio"]);

  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await expect(page.getByText("日曜日は図書館に行きます")).toBeVisible();
  await expect(
    page.getByRole("img", { name: "A quiet library reading room" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Play audio", exact: true }),
  ).toBeVisible();
  await expect(page.locator("audio")).not.toHaveAttribute("controls");
  await expect.poll(() => mediaPlayCount(page)).toBe(1);
});

test("preserves an explicitly disabled autoplay preference", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-autoplay-prompt-audio", "false");
  });
  await openStudy(page, "/?media=ready");

  await expect(
    page.getByRole("button", { name: "Autoplay off" }),
  ).toHaveAttribute("aria-pressed", "false");
  await expect.poll(() => mediaPlayCount(page)).toBe(0);
});

test("does not autoplay the same prompt again after returning from Edit", async ({
  page,
}) => {
  await openStudy(page, "/?media=ready");
  await expect.poll(() => mediaPlayCount(page)).toBe(1);

  await page.getByRole("button", { name: "Edit note" }).click();
  await expect(
    page.getByRole("heading", { name: "Add / Edit card" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Return to study" }).click();

  await expect(page.getByLabel("Your answer")).toBeVisible();
  await expect.poll(() => mediaPlayCount(page)).toBe(1);
});

test("persists the Study autoplay toggle and applies it to the next card", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-autoplay-prompt-audio", "false");
  });
  await openStudy(page, "/?media=ready&reconcile=request");
  await page.getByRole("button", { name: "Autoplay off" }).click();

  expect(
    await page.evaluate(() =>
      localStorage.getItem("meiki-autoplay-prompt-audio"),
    ),
  ).toBe("true");
  await expect.poll(() => mediaPlayCount(page)).toBe(0);

  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("Enter");
  await page.getByRole("button", { name: "Continue" }).click();

  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect.poll(() => mediaPlayCount(page)).toBe(1);

  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Settings" })
    .click();
  const settingsSwitch = page.getByRole("switch", { name: "Enable" });
  await expect(settingsSwitch).toBeChecked();
  await settingsSwitch.click();
  expect(
    await page.evaluate(() =>
      localStorage.getItem("meiki-autoplay-prompt-audio"),
    ),
  ).toBe("false");
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Today", exact: true })
    .click();
  await page.getByRole("button", { name: "Resume study" }).click();
  await expect(
    page.getByRole("button", { name: "Autoplay off" }),
  ).toHaveAttribute("aria-pressed", "false");
});

test("plays pauses replays and seeks with the custom audio control", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-autoplay-prompt-audio", "false");
  });
  await openStudy(page, "/?media=ready");
  const audio = page.locator("audio");
  const seek = page.getByRole("slider", { name: "Seek prompt.wav" });

  await expect(audio).not.toHaveAttribute("controls");
  await expect(page.getByLabel("Elapsed and total time")).toHaveText(
    "0:00 / 0:01",
  );
  await page.getByRole("button", { name: "Play audio", exact: true }).click();
  await expect(page.getByRole("button", { name: "Pause audio" })).toBeVisible();
  await page.getByRole("button", { name: "Pause audio" }).click();
  expect(
    await page.evaluate(() =>
      Number(localStorage.getItem("meiki-e2e-media-pause-count") ?? "0"),
    ),
  ).toBe(1);

  await seek.focus();
  await page.keyboard.press("End");
  await expect(seek).toHaveAttribute("aria-valuenow", "1");
  await page.getByRole("button", { name: "Replay audio" }).click();
  await expect(seek).toHaveAttribute("aria-valuenow", "0");
  await page.locator("#study-prompt").click();
  await page.keyboard.press("r");
  await expect.poll(() => mediaPlayCount(page)).toBe(3);
});

test("keeps manual audio controls usable when autoplay is blocked", async ({
  page,
}) => {
  await openStudy(page, "/?media=blocked");

  await expect(
    page
      .getByTestId("app-shell")
      .getByRole("status")
      .filter({ hasText: "Prompt audio could not start automatically" }),
  ).toContainText("Use Play to hear it.");
  await expect(page.getByLabel("Your answer")).toBeEnabled();
  await page.getByRole("button", { name: "Play audio", exact: true }).click();
  await expect.poll(() => mediaPlayCount(page)).toBe(1);
  await expect(page.getByRole("button", { name: "Pause audio" })).toBeVisible();
});

test("loads and plays a real MP3 through the browser media engine", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-autoplay-prompt-audio", "false");
  });
  await openStudy(page, "/?media=real-mp3");
  const audio = page.locator("audio");
  await audio.evaluate((element) => {
    element.dataset.playEvents = "0";
    element.addEventListener("play", () => {
      element.dataset.playEvents = String(
        Number(element.dataset.playEvents ?? "0") + 1,
      );
    });
  });

  await expect
    .poll(() => audio.evaluate((element) => element.duration))
    .toBeGreaterThan(0);
  await page.getByRole("button", { name: "Play audio", exact: true }).click();
  await expect
    .poll(() => audio.evaluate((element) => element.currentTime))
    .toBeGreaterThan(0);
  await expect.poll(() => audio.getAttribute("data-play-events")).toBe("1");
  await expect
    .poll(() => audio.evaluate((element) => element.ended))
    .toBe(true);
  await page.getByRole("button", { name: "Play audio", exact: true }).click();
  await expect.poll(() => audio.getAttribute("data-play-events")).toBe("2");
  await expect
    .poll(() => audio.evaluate((element) => element.ended))
    .toBe(true);
  await page.getByRole("button", { name: "Replay audio" }).click();
  await expect.poll(() => audio.getAttribute("data-play-events")).toBe("3");
  expect(await mediaPlayCount(page)).toBe(0);
});

test("restarts audio at the decoder boundary and reports playback failures", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-autoplay-prompt-audio", "false");
  });
  await openStudy(page, "/?media=ready");
  const audio = page.locator("audio");
  await audio.evaluate((element) => {
    element.currentTime = 0.98;
    Object.defineProperty(element, "duration", {
      configurable: true,
      get: () => 1,
    });
    Object.defineProperty(element, "ended", {
      configurable: true,
      get: () => false,
    });
    Object.defineProperty(element, "seeking", {
      configurable: true,
      get: () => true,
    });
    Object.defineProperty(element, "readyState", {
      configurable: true,
      get: () => HTMLMediaElement.HAVE_METADATA,
    });
  });

  await page.getByRole("button", { name: "Play audio", exact: true }).click();
  await expect(audio).toHaveJSProperty("currentTime", 0);
  await expect.poll(() => mediaPlayCount(page)).toBe(0);
  await audio.dispatchEvent("seeked");
  await expect.poll(() => mediaPlayCount(page)).toBe(1);

  await audio.dispatchEvent("error");
  await expect(page.getByRole("alert")).toHaveText("Audio could not load.");

  await openStudy(page, "/?media=playback-error");
  await page.getByRole("button", { name: "Play audio", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveText(
    "Audio could not play. Try again.",
  );
  await expect(page.getByLabel("Your answer")).toBeEnabled();
});

test("keeps answering enabled for missing corrupt and unsupported audio", async ({
  page,
}) => {
  for (const [scenario, message] of [
    ["missing", "Media file is missing"],
    ["corrupt", "Media integrity check failed"],
    ["unsupported", "Media format is unsupported"],
  ] as const) {
    await openStudy(page, `/?media=${scenario}`);
    await expect(page.getByText(message)).toBeVisible();
    await expect(page.getByLabel("Your answer")).toBeEnabled();
  }
});

test("accepts asset media and rejects remote or unsupported sources", async ({
  page,
}) => {
  await openStudy(page, "/?media=asset");
  await expect(page.locator("audio")).toHaveAttribute("src", /^asset:/);
  await expect(page.getByLabel("Your answer")).toBeEnabled();

  for (const media of ["remote-http", "remote-https", "unsupported"]) {
    await openStudy(page, `/?media=${media}`);
    await expect(page.locator("audio")).toHaveCount(0);
    await expect(page.getByText("Media format is unsupported")).toBeVisible();
    await expect(page.getByLabel("Your answer")).toBeEnabled();
  }
});

test("does not submit Enter during IME composition", async ({ page }) => {
  await openStudy(page, "/");
  const input = page.getByLabel("Your answer");
  await input.dispatchEvent("compositionstart");
  await input.press("Enter");
  await expect(page.getByText("Expected answer")).toBeHidden();

  await input.dispatchEvent("compositionend");
  await page.getByRole("button", { name: /Check answer/ }).click();
  await expect(page.getByText("Expected answer")).toBeVisible();
});

test("preserves multilingual and multi-code-point input in command requests", async ({
  page,
}) => {
  const fixtures = [
    ["cjk", "行きます"],
    ["rtl", "کتاب"],
    ["devanagari", "पुस्तक"],
    ["ltr", " la bibliothe\u{300}que "],
    ["mixed", "三時"],
    ["emoji", "👨‍👩‍👧‍👦"],
  ] as const;

  for (const [fixture, response] of fixtures) {
    await openStudy(page, `/?fixture=${fixture}`);
    const input = page.getByLabel("Your answer");
    await input.fill(response);
    await input.press("Enter");
    await expect(page.getByText("exact", { exact: true })).toBeVisible();
    expect((await lastRequest(page, "check_answer"))?.args).toMatchObject({
      request: { raw_response: response },
    });
  }
});

test("renders difference semantics without altering raw input", async ({
  page,
}) => {
  await openStudy(page, "/?answer=wrong");
  await page.getByLabel("Your answer").fill(" 図書館 ");
  await page.getByLabel("Your answer").press("Enter");

  const difference = page.getByTestId("answer-difference");
  await expect(difference.locator("del")).toHaveText("行きます");
  await expect(difference.locator("ins")).toHaveText("図書館");
  await expect(
    page.locator(".answer-comparison strong").nth(1),
  ).toHaveJSProperty("textContent", " 図書館 ");
});

test("maps keyboard grading and undo without browser persistence logic", async ({
  page,
}) => {
  await openStudy(page, "/");
  await page.keyboard.type("行きます");
  await page.keyboard.press("Enter");
  await page.keyboard.press("4");
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();

  await page.keyboard.press("ControlOrMeta+z");
  await expect(page.getByText("Last review undone.")).toBeVisible();
  expect((await lastRequest(page, "undo_review"))?.args).toMatchObject({
    request: {
      card_id: "due-card",
      review_event_id: expect.any(String),
      undo_event_id: expect.any(String),
    },
  });
  await expect(page.getByLabel("Your answer")).toBeFocused();
});

test("offers UI retries for interrupted command responses", async ({
  page,
}) => {
  await openStudy(page, "/?fixture=ltr&failure=check");
  const input = page.getByLabel("Your answer");
  await input.fill(" la bibliothe\u{300}que ");
  await input.press("Enter");
  await expect(
    page.getByText("The answer check was interrupted."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByText("Expected answer")).toBeVisible();

  await openStudy(page, "/?failure=grade");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    page.getByText("The review commit was interrupted."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Try again" }).click();
  await expect(
    page.getByRole("heading", { name: "Review saved" }),
  ).toBeVisible();
});

test("replays a pending review request from a restart fixture", async ({
  page,
}) => {
  await page.addInitScript(() => {
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
          {
            card_id: "new-card",
            card_content_version: 0,
            schedule_version: 0,
          },
        ],
        position: 0,
        startedAtMs: 1_700_000_000_000,
        pendingReview: {
          review_event_id: "pending-on-restart",
          card_id: "due-card",
          card_content_version: 0,
          schedule_version: 0,
          raw_response: "行きます",
          chosen_grade: "good",
          response_duration_ms: 1_000,
        },
      }),
    );
  });
  await page.goto("/?reconcile=second");
  await page.getByRole("button", { name: "Resume study" }).click();
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  expect((await lastRequest(page, "grade_review"))?.args).toMatchObject({
    request: { review_event_id: "pending-on-restart" },
  });
});

test("maps edit and suspend controls while retaining the reveal UI", async ({
  page,
}) => {
  await openStudy(page, "/");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.keyboard.press("e");

  await expect(
    page.getByRole("heading", { name: "Add / Edit card" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Return to study" }).click();
  await expect(page.getByText("Expected answer")).toBeVisible();

  await page.keyboard.press("s");
  await expect(
    page.getByRole("heading", { name: "Card suspended" }),
  ).toBeVisible();
  expect((await lastRequest(page, "suspend_card"))?.args).toMatchObject({
    request: { card_id: "due-card" },
  });
});
