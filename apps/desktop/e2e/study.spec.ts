import { expect, test, type Page } from "@playwright/test";

import { installMockApi } from "./support/mock-api";

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function openStudy(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await page.getByRole("button", { name: /^(Start|Resume) study$/ }).click();
}

async function chooseTheme(page: Page, theme: "light" | "dark"): Promise<void> {
  await page.getByRole("button", { name: "Theme" }).click();
  await page
    .getByRole("option", { name: new RegExp(`^${theme}$`, "i") })
    .click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
}

async function revealAnswer(
  page: Page,
  fixture: string,
  answer: string,
): Promise<void> {
  await openStudy(page, `/?fixture=${fixture}`);
  await page.getByLabel("Your answer").fill(answer);
  await page.getByLabel("Your answer").press("Enter");
  await expect(page.getByText("Expected answer")).toBeVisible();
}

async function lastRequest(page: Page, command: string) {
  return page.evaluate((name) => {
    const requests = window.__MEIKI_TEST_REQUESTS__ ?? [];
    return requests.filter((request) => request.command === name).at(-1);
  }, command);
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
  const correctFeedback = page
    .getByTestId("answer-difference")
    .locator('[data-feedback="correct"]');
  await expect(correctFeedback.locator(".feedback-symbol")).toHaveText("✓");
  await expect(correctFeedback.locator("bdi")).toHaveText("行きます");
  await expect(page.getByText("exact", { exact: true })).toHaveCount(0);
  expect((await lastRequest(page, "check_answer"))?.args).toMatchObject({
    request: { card_id: "due-card", raw_response: "行きます" },
  });

  await page.keyboard.press("4");
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByTestId("review-saved-status")).toContainText(
    "Review saved",
  );
  await expect(page.getByRole("heading", { name: "Review saved" })).toHaveCount(
    0,
  );
  await expect(page.getByText("Next review", { exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Continue" })).toHaveCount(0);
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

test("keeps review undo visible alongside a next-card replay notice", async ({
  page,
}) => {
  await openStudy(page, "/");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.getByRole("button", { name: /^Good/ }).click();
  await expect(page.getByText(/Second card ·/)).toBeVisible();

  const reviewStatus = page.getByTestId("review-saved-status");
  await expect(reviewStatus).toContainText("Review saved");
  await expect(
    reviewStatus.getByRole("button", { name: "Undo review" }),
  ).toBeVisible();

  await page.locator("#main-content").focus();
  await page.keyboard.press("r");
  await expect(
    page
      .getByRole("status")
      .filter({ hasText: "No playable audio is attached to this side" }),
  ).toBeVisible();
  await expect(reviewStatus).toBeVisible();
});

test("decodes non-silent managed MP3 bytes and cleans up prompt and reveal URLs", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-autoplay-prompt-audio", "false");
  });
  await openStudy(page, "/?media=real-mp3");
  const audio = page.locator("audio");
  await expect(audio).toHaveAttribute("src", /^blob:/);
  expect((await lastRequest(page, "read_managed_audio"))?.args).toEqual({
    contentHash:
      "sha256:4732a7cfa0f5dc2a3c8ded1378d2fa4cef6b315dfd0e29ab5479b90a6db13157",
  });
  const decoded = await audio.evaluate(async (element) => {
    const bytes = await (await fetch(element.src)).arrayBuffer();
    const digest = Array.from(
      new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    )
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join("");
    const context = new AudioContext();
    const buffer = await context.decodeAudioData(bytes.slice(0));
    let peak = 0;
    let squaredSamples = 0;
    let sampleCount = 0;
    for (let channel = 0; channel < buffer.numberOfChannels; channel += 1) {
      for (const sample of buffer.getChannelData(channel)) {
        peak = Math.max(peak, Math.abs(sample));
        squaredSamples += sample * sample;
        sampleCount += 1;
      }
    }
    await context.close();
    return {
      byteLength: bytes.byteLength,
      digest,
      duration: buffer.duration,
      peak,
      rms: Math.sqrt(squaredSamples / sampleCount),
    };
  });
  expect(decoded.byteLength).toBe(2_445);
  expect(decoded.digest).toBe(
    "4732a7cfa0f5dc2a3c8ded1378d2fa4cef6b315dfd0e29ab5479b90a6db13157",
  );
  expect(decoded.duration).toBeGreaterThan(0);
  expect(decoded.peak).toBeGreaterThan(0.05);
  expect(decoded.rms).toBeGreaterThan(0.01);

  await audio.evaluate((element) => {
    element.dataset.playEvents = "0";
    element.dataset.seekEvents = "0";
    element.addEventListener("play", () => {
      element.dataset.playEvents = String(
        Number(element.dataset.playEvents ?? "0") + 1,
      );
    });
    element.addEventListener("seeking", () => {
      element.dataset.seekEvents = String(
        Number(element.dataset.seekEvents ?? "0") + 1,
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
  await expect(audio).toHaveAttribute("data-seek-events", "0");
  await page.getByRole("button", { name: "Play audio", exact: true }).click();
  await expect.poll(() => audio.getAttribute("data-play-events")).toBe("2");
  await expect
    .poll(() => audio.evaluate((element) => element.ended))
    .toBe(true);
  await page.getByRole("button", { name: "Replay audio" }).click();
  await expect.poll(() => audio.getAttribute("data-play-events")).toBe("3");
  expect(await mediaPlayCount(page)).toBe(0);

  const promptUrl = await audio.getAttribute("src");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  const revealAudio = page.locator("audio");
  await expect(revealAudio).toHaveAttribute("src", /^blob:/);
  await expect
    .poll(() =>
      page.evaluate(
        (url) => window.__MEIKI_TEST_REVOKED_OBJECT_URLS__?.includes(url ?? ""),
        promptUrl,
      ),
    )
    .toBe(true);
  await revealAudio.evaluate((element) => {
    element.dataset.playEvents = "0";
    element.addEventListener("play", () => {
      element.dataset.playEvents = String(
        Number(element.dataset.playEvents ?? "0") + 1,
      );
    });
  });
  await expect(revealAudio).toHaveJSProperty("paused", true);
  await expect(revealAudio).toHaveJSProperty("currentTime", 0);
  await expect(revealAudio).toHaveAttribute("data-play-events", "0");
  await page.getByRole("button", { name: "Play audio", exact: true }).click();
  await expect
    .poll(() => revealAudio.evaluate((element) => element.currentTime))
    .toBeGreaterThan(0);
  await expect(revealAudio).toHaveAttribute("data-play-events", "1");
  await expect
    .poll(() => revealAudio.evaluate((element) => element.ended))
    .toBe(true);
  const revealUrl = await revealAudio.getAttribute("src");
  await page.keyboard.press("3");
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        (url) => window.__MEIKI_TEST_REVOKED_OBJECT_URLS__?.includes(url ?? ""),
        revealUrl,
      ),
    )
    .toBe(true);
  const managedReads = await page.evaluate(() =>
    (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
      (request) => request.command === "read_managed_audio",
    ),
  );
  expect(managedReads).toHaveLength(3);
  for (const managedRead of managedReads.slice(1)) {
    expect(managedRead.args).toEqual(managedReads[0]?.args);
  }
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

  await audio.evaluate((element) => {
    Object.defineProperty(element, "error", {
      configurable: true,
      get: () => ({ code: MediaError.MEDIA_ERR_ABORTED }),
    });
    element.dispatchEvent(new Event("error"));
  });
  await expect(page.getByRole("alert")).toHaveText("Audio could not load.");
  await audio.evaluate((element) => {
    Object.defineProperty(element, "error", {
      configurable: true,
      get: () => ({ code: MediaError.MEDIA_ERR_DECODE }),
    });
    element.dispatchEvent(new Event("error"));
  });
  await expect(page.getByRole("alert")).toHaveText(
    "Audio could not be decoded.",
  );

  await openStudy(page, "/?media=playback-error");
  await page.getByRole("button", { name: "Play audio", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveText(
    "Audio could not play. Try again.",
  );
  await expect(page.getByLabel("Your answer")).toBeEnabled();
});

test("reports native managed-audio transport failures without blocking study", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-autoplay-prompt-audio", "false");
  });
  await openStudy(page, "/?media=transport-error");
  await expect(page.getByRole("alert")).toHaveText("Audio transport failed.");
  await expect(
    page.getByRole("button", { name: "Play audio", exact: true }),
  ).toBeDisabled();
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
    await expect(
      page
        .getByTestId("answer-difference")
        .locator('[data-feedback="correct"]'),
    ).toBeVisible();
    await expect(page.getByText("exact", { exact: true })).toHaveCount(0);
    expect((await lastRequest(page, "check_answer"))?.args).toMatchObject({
      request: { raw_response: response },
    });
  }
});

for (const theme of ["light", "dark"] as const) {
  test(`revealed cloze uses the primary pair with 4.5:1 contrast in ${theme} mode`, async ({
    page,
  }) => {
    await openStudy(page, "/?fixture=cjk");
    await chooseTheme(page, theme);
    await page.getByLabel("Your answer").fill("行きます");
    await page.getByLabel("Your answer").press("Enter");

    const appearance = await page
      .locator("#study-prompt mark")
      .evaluate((mark) => {
        const sample = (color: string): number[] => {
          const canvas = document.createElement("canvas");
          canvas.width = 1;
          canvas.height = 1;
          const context = canvas.getContext("2d");
          if (!context) throw new Error("Canvas color sampling is unavailable");
          context.fillStyle = color;
          context.fillRect(0, 0, 1, 1);
          return [...context.getImageData(0, 0, 1, 1).data.slice(0, 3)];
        };
        const luminance = (color: number[]): number => {
          const [red, green, blue] = color.map((component) => {
            const channel = component / 255;
            return channel <= 0.04045
              ? channel / 12.92
              : ((channel + 0.055) / 1.055) ** 2.4;
          });
          return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
        };
        const style = getComputedStyle(mark);
        const rootStyle = getComputedStyle(document.documentElement);
        const foreground = sample(style.color);
        const background = sample(style.backgroundColor);
        const foregroundLuminance = luminance(foreground);
        const backgroundLuminance = luminance(background);
        return {
          background,
          borderRadius: style.borderRadius,
          contrast:
            (Math.max(foregroundLuminance, backgroundLuminance) + 0.05) /
            (Math.min(foregroundLuminance, backgroundLuminance) + 0.05),
          fontWeight: Number(style.fontWeight),
          foreground,
          paddingInlineEnd: Number.parseFloat(style.paddingInlineEnd),
          paddingInlineStart: Number.parseFloat(style.paddingInlineStart),
          primary: sample(rootStyle.getPropertyValue("--primary")),
          primaryForeground: sample(
            rootStyle.getPropertyValue("--primary-foreground"),
          ),
        };
      });

    expect(appearance.background).toEqual(appearance.primary);
    expect(appearance.foreground).toEqual(appearance.primaryForeground);
    expect(appearance.contrast).toBeGreaterThanOrEqual(4.5);
    expect(appearance.fontWeight).toBeGreaterThanOrEqual(700);
    expect(appearance.borderRadius).toBe("0px");
    expect(appearance.paddingInlineStart).toBeGreaterThan(0);
    expect(appearance.paddingInlineStart).toBeLessThanOrEqual(8);
    expect(appearance.paddingInlineEnd).toBeGreaterThan(0);
    expect(appearance.paddingInlineEnd).toBeLessThanOrEqual(8);
  });
}

for (const [language, fixture, answer] of [
  ["Latin", "ltr", "la bibliothèque"],
  ["Korean", "korean", "읽어요"],
  ["Japanese", "cjk", "行きます"],
] as const) {
  test(`revealed cloze remains semantic for ${language} text`, async ({
    page,
  }) => {
    await revealAnswer(page, fixture, answer);
    const highlight = page.locator("#study-prompt mark");
    await expect(highlight).toHaveText(answer);
    await expect(highlight).toHaveJSProperty("tagName", "MARK");
  });
}

test("mixed-direction revealed content remains isolated in source order", async ({
  page,
}) => {
  await revealAnswer(page, "mixed", "三時");
  const prompt = page.locator("#study-prompt");
  await expect(prompt).toHaveAttribute("dir", "auto");
  await expect(prompt).toHaveText("Meetingは الساعة 三時 に始まる");
  await expect(prompt.locator("mark")).toHaveText("三時");
  expect(
    await prompt.evaluate((element) => getComputedStyle(element).unicodeBidi),
  ).toBe("isolate");
});

test("long revealed answer wraps without clipping or horizontal overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 720 });
  const answer =
    "this intentionally long highlighted answer includes 한국어, 日本語, and العربية while wrapping naturally across several lines";
  await revealAnswer(page, "longanswer", answer);

  const layout = await page.locator("#study-prompt mark").evaluate((mark) => {
    const prompt = mark.parentElement;
    if (!prompt) throw new Error("Revealed cloze has no prompt container");
    const promptBounds = prompt.getBoundingClientRect();
    const lines = [...mark.getClientRects()];
    return {
      lineCount: lines.length,
      linesStayInsidePrompt: lines.every(
        (line) =>
          line.left >= promptBounds.left - 1 &&
          line.right <= promptBounds.right + 1,
      ),
      promptFits: prompt.scrollWidth <= prompt.clientWidth,
      viewportFits: document.documentElement.scrollWidth <= window.innerWidth,
    };
  });
  expect(layout.lineCount).toBeGreaterThan(1);
  expect(layout.linesStayInsidePrompt).toBe(true);
  expect(layout.promptFits).toBe(true);
  expect(layout.viewportFits).toBe(true);
});

test("renders difference semantics without altering raw input", async ({
  page,
}) => {
  await openStudy(page, "/?answer=wrong");
  await page.getByLabel("Your answer").fill(" 図書館 ");
  await page.getByLabel("Your answer").press("Enter");

  const difference = page.getByTestId("answer-difference");
  await expect(difference.locator("ins .feedback-symbol")).toHaveText("+");
  await expect(difference.locator("ins bdi")).toHaveText("行きます");
  await expect(difference.locator("del .feedback-symbol")).toHaveText("−");
  await expect(difference.locator("del bdi")).toHaveText("図書館");
  const differenceStyles = await difference.evaluate((element) => {
    const extra = element.querySelector("del");
    const missing = element.querySelector("ins");
    if (!extra || !missing) {
      throw new Error("Answer difference semantics are missing");
    }
    const extraStyle = getComputedStyle(extra);
    const missingStyle = getComputedStyle(missing);
    return {
      extraBackground: extraStyle.backgroundColor,
      extraDecoration: extraStyle.textDecorationLine,
      missingBackground: missingStyle.backgroundColor,
      missingDecoration: missingStyle.textDecorationLine,
    };
  });
  expect(differenceStyles.extraBackground).not.toBe("rgba(0, 0, 0, 0)");
  expect(differenceStyles.extraDecoration).toContain("line-through");
  expect(differenceStyles.missingBackground).not.toBe("rgba(0, 0, 0, 0)");
  expect(differenceStyles.missingDecoration).toContain("underline");
  await expect(
    page.locator(".answer-comparison strong").nth(1),
  ).toHaveJSProperty("textContent", " 図書館 ");
});

test("tries the same answer again without reviewing and restarts local response state", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-study-visual-keyboard", "true");
    localStorage.setItem("meiki-typing-platform", "windows");
    localStorage.setItem("meiki-e2e-study-time", "1000");
    Object.defineProperty(performance, "now", {
      configurable: true,
      value: () => Number(localStorage.getItem("meiki-e2e-study-time") ?? "0"),
    });
  });
  await openStudy(page, "/");
  const input = page.getByLabel("Your answer");
  const prompt = await page.locator("#study-prompt").textContent();
  await input.fill("first response");
  await input.dispatchEvent("keydown", { code: "ShiftLeft", key: "Shift" });
  await page.evaluate(() =>
    localStorage.setItem("meiki-e2e-study-time", "1800"),
  );
  await input.press("Enter");
  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();

  await page.evaluate(() =>
    localStorage.setItem("meiki-e2e-study-time", "5000"),
  );
  await page.getByRole("button", { name: "Try answer again" }).click();
  await expect(page.locator("#study-prompt")).toHaveText(prompt ?? "");
  await expect(input).toHaveValue("");
  await expect(input).toBeFocused();
  await expect(page.getByText("Expected answer", { exact: true })).toHaveCount(
    0,
  );
  await expect(page.getByTestId("typing-key-ShiftLeft")).toHaveAttribute(
    "data-held",
    "false",
  );
  expect(await requestCount(page, "grade_review")).toBe(0);
  expect(await requestCount(page, "undo_review")).toBe(0);

  await input.fill("行きます");
  await page.evaluate(() =>
    localStorage.setItem("meiki-e2e-study-time", "5250"),
  );
  await input.press("Enter");
  await page.getByRole("button", { name: /^Good/ }).click();
  expect((await lastRequest(page, "grade_review"))?.args).toMatchObject({
    request: { raw_response: "行きます", response_duration_ms: 250 },
  });
});

for (const [answerMode, response, renderedAnswer] of [
  ["exact", "行きます", "行きます"],
  ["accepted", "いきます", "いきます"],
] as const) {
  test(`${answerMode} answers use complete semantic success feedback without raw comparison copy`, async ({
    page,
  }) => {
    await openStudy(
      page,
      answerMode === "accepted" ? "/?answer=accepted" : "/",
    );
    await page.getByLabel("Your answer").fill(response);
    await page.getByLabel("Your answer").press("Enter");

    const feedback = page
      .getByTestId("answer-difference")
      .locator('[data-feedback="correct"]');
    await expect(feedback.locator(".feedback-symbol")).toHaveText("✓");
    await expect(feedback.locator("bdi")).toHaveText(renderedAnswer);
    await expect(feedback).toContainText(`Correct: ${renderedAnswer}`);
    for (const rawLabel of [
      "exact",
      "accepted variant",
      "near match",
      "incorrect",
      "empty",
    ]) {
      await expect(page.getByText(rawLabel, { exact: true })).toHaveCount(0);
    }
  });
}

test("empty and mixed answer differences expose missing, extra, and correct semantics", async ({
  page,
}) => {
  await openStudy(page, "/?answer=empty");
  await page.getByLabel("Your answer").press("Enter");
  const emptyDifference = page.getByTestId("answer-difference");
  await expect(emptyDifference.locator("ins .feedback-symbol")).toHaveText("+");
  await expect(emptyDifference.locator("ins bdi")).toHaveText("行きます");
  await expect(emptyDifference.locator("ins")).toContainText(
    "Missing: 行きます",
  );
  await expect(page.getByText("empty", { exact: true })).toHaveCount(0);

  await openStudy(page, "/?answer=extra-prefix");
  await page.getByLabel("Your answer").fill("大学生");
  await page.getByLabel("Your answer").press("Enter");
  const mixedDifference = page.getByTestId("answer-difference");
  await expect(mixedDifference.locator("del .feedback-symbol")).toHaveText("−");
  await expect(mixedDifference.locator("del bdi")).toHaveText("大");
  await expect(mixedDifference.locator("del")).toContainText("Extra: 大");
  await expect(
    mixedDifference.locator('[data-feedback="correct"] .feedback-symbol'),
  ).toHaveText("✓");
  await expect(
    mixedDifference.locator('[data-feedback="correct"] bdi'),
  ).toHaveText("学生");
  await expect(
    mixedDifference.locator('[data-feedback="correct"]'),
  ).toContainText("Correct: 学生");

  await openStudy(page, "/?answer=grapheme");
  await page.getByLabel("Your answer").fill("e\u{301}👨‍👩‍👧‍👦!");
  await page.getByLabel("Your answer").press("Enter");
  await expect(
    page
      .getByTestId("answer-difference")
      .locator('[data-feedback="correct"] bdi'),
  ).toHaveText("e\u{301}👨‍👩‍👧‍👦");
  await expect(
    page.getByTestId("answer-difference").locator("del bdi"),
  ).toHaveText("!");
});

for (const theme of ["light", "dark"] as const) {
  test(`correct and incorrect answer feedback reaches 4.5:1 contrast in ${theme} mode`, async ({
    page,
  }) => {
    await openStudy(page, "/?answer=extra-prefix");
    await chooseTheme(page, theme);
    await page.getByLabel("Your answer").fill("大学生");
    await page.getByLabel("Your answer").press("Enter");

    const contrasts = await page
      .getByTestId("answer-difference")
      .evaluate((element) => {
        const sample = (color: string): number[] => {
          const canvas = document.createElement("canvas");
          canvas.width = 1;
          canvas.height = 1;
          const context = canvas.getContext("2d");
          if (!context) throw new Error("Canvas color sampling is unavailable");
          context.fillStyle = color;
          context.fillRect(0, 0, 1, 1);
          return [...context.getImageData(0, 0, 1, 1).data.slice(0, 3)];
        };
        const luminance = (color: number[]): number => {
          const [red, green, blue] = color.map((component) => {
            const channel = component / 255;
            return channel <= 0.04045
              ? channel / 12.92
              : ((channel + 0.055) / 1.055) ** 2.4;
          });
          return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
        };
        const contrast = (node: Element): number => {
          const style = getComputedStyle(node);
          const foreground = luminance(sample(style.color));
          const background = luminance(sample(style.backgroundColor));
          return (
            (Math.max(foreground, background) + 0.05) /
            (Math.min(foreground, background) + 0.05)
          );
        };
        const correct = element.querySelector('[data-feedback="correct"]');
        const extra = element.querySelector('[data-feedback="extra"]');
        if (!correct || !extra) throw new Error("Answer feedback is missing");
        return { correct: contrast(correct), extra: contrast(extra) };
      });
    expect(contrasts.correct).toBeGreaterThanOrEqual(4.5);
    expect(contrasts.extra).toBeGreaterThanOrEqual(4.5);
  });
}

test("grade buttons disclose keyboard shortcuts on hover and focus", async ({
  page,
}) => {
  await revealAnswer(page, "cjk", "行きます");
  const buttons = ["Again", "Hard", "Good", "Easy"].map((name) =>
    page.getByRole("button", { name: new RegExp(`^${name}`) }),
  );
  for (const [index, button] of buttons.entries()) {
    await expect(button).toHaveAttribute(
      "aria-keyshortcuts",
      String(index + 1),
    );
  }
  await buttons[0].hover();
  await expect(
    page.getByRole("tooltip", { name: "Shortcut: 1" }),
  ).toBeVisible();
  await page.mouse.move(0, 0);
  await buttons[2].focus();
  await expect(
    page.getByRole("tooltip", { name: "Shortcut: 3" }),
  ).toBeVisible();
});

test("rapid repeated grading submits once and advances directly", async ({
  page,
}) => {
  await revealAnswer(page, "cjk", "行きます");
  const good = page.getByRole("button", { name: /^Good/ });
  await good.evaluate((button: HTMLButtonElement) => {
    button.click();
    button.click();
  });
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  expect(await requestCount(page, "grade_review")).toBe(1);
});

test("blocks conflicting revealed actions while a grade is committing", async ({
  page,
}) => {
  await openStudy(page, "/?grade=controlled");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");

  const tryAgain = page.getByRole("button", { name: "Try answer again" });
  const edit = page.getByRole("button", { name: "Edit note" });
  const suspend = page.getByRole("button", { name: "Suspend" });
  await page.getByRole("button", { name: /^Good/ }).click();
  await expect(page.getByText("Saving review…")).toBeVisible();
  await expect(tryAgain).toBeDisabled();
  await expect(edit).toBeDisabled();
  await expect(suspend).toBeDisabled();

  await tryAgain.dispatchEvent("click");
  await edit.dispatchEvent("click");
  await suspend.dispatchEvent("click");
  await expect(page.getByText("Saving review…")).toBeVisible();
  await expect(
    page.getByText("Expected answer", { exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel("Your answer")).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Add / Edit card" }),
  ).toHaveCount(0);
  expect(await requestCount(page, "grade_review")).toBe(1);
  expect(await requestCount(page, "suspend_card")).toBe(0);

  await page.evaluate(() =>
    window.dispatchEvent(new Event("meiki-e2e-release-grade")),
  );
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByTestId("review-saved-status")).toBeVisible();
});

test("maps keyboard grading and undo without browser persistence logic", async ({
  page,
}) => {
  await openStudy(page, "/");
  await page.keyboard.type("行きます");
  await page.keyboard.press("Enter");
  await page.keyboard.press("4");
  await expect(page.getByText(/Second card ·/)).toBeVisible();

  await page.locator("#main-content").focus();
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

test("keeps one undo on session completion and restores the reviewed queue position", async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 720 });
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
        ],
        position: 0,
        startedAtMs: 1_700_000_000_000,
        pendingReview: null,
      }),
    );
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Resume study" }).click();
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.getByRole("button", { name: /^Good/ }).click();

  await expect(
    page.getByRole("heading", { name: "Session complete" }),
  ).toBeVisible();
  await expect(page.getByTestId("review-saved-status")).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    ),
  ).toBe(false);
  expect(
    await page.evaluate(() => localStorage.getItem("meiki-active-study-queue")),
  ).toBeNull();

  await page.getByRole("button", { name: "Undo review" }).click();
  await expect(page.getByText("日曜日は図書館に[…]")).toBeVisible();
  await expect(page.getByLabel("Your answer")).toBeFocused();
  await expect(page.getByLabel("Your answer")).toHaveValue("");
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({
    position: 0,
    entries: [
      {
        card_id: "due-card",
        card_content_version: 0,
        schedule_version: 2,
      },
    ],
  });
});

test("discards the previous-review undo before it could replace a new response", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("meiki-vim-keybindings", "true");
  });
  await openStudy(page, "/");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.getByRole("button", { name: /^Good/ }).click();
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByTestId("review-saved-status")).toBeVisible();

  const nextAnswer = page.getByLabel("Your answer");
  await nextAnswer.fill("unfinished next answer");
  await expect(page.getByTestId("review-saved-status")).toHaveCount(0);
  await page.locator("#main-content").focus();
  await page.keyboard.press("u");
  await expect(nextAnswer).toHaveValue("unfinished next answer");
  expect(await requestCount(page, "undo_review")).toBe(0);
});

test("editing or suspending the next card retires the previous-review undo", async ({
  page,
}) => {
  await openStudy(page, "/");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.getByRole("button", { name: /^Good/ }).click();
  await expect(page.getByTestId("review-saved-status")).toBeVisible();

  await page.getByRole("button", { name: "Edit note" }).click();
  await page.getByRole("button", { name: "Return to study" }).click();
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  await expect(page.getByTestId("review-saved-status")).toHaveCount(0);

  await page.getByRole("button", { name: "Suspend" }).click();
  await expect(
    page.getByRole("heading", { name: "Card suspended" }),
  ).toBeVisible();
  expect(await requestCount(page, "undo_review")).toBe(0);
});

test("a failed undo preserves the committed review and retries the same undo request", async ({
  page,
}) => {
  await openStudy(page, "/?failure=undo");
  await page.getByLabel("Your answer").fill("行きます");
  await page.getByLabel("Your answer").press("Enter");
  await page.getByRole("button", { name: /^Good/ }).click();
  await expect(page.getByText(/Second card ·/)).toBeVisible();

  await page.locator("#main-content").focus();
  await page.keyboard.press("ControlOrMeta+z");
  await expect(page.getByRole("alert")).toContainText(
    "The review undo was interrupted.",
  );
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("meiki-active-study-queue") ?? "null"),
    ),
  ).toMatchObject({ position: 0, entries: [{ card_id: "new-card" }] });

  await page.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByText("Last review undone.")).toBeVisible();
  await expect(page.getByLabel("Your answer")).toBeFocused();
  const undoRequests = await page.evaluate(() =>
    (window.__MEIKI_TEST_REQUESTS__ ?? []).filter(
      (request) => request.command === "undo_review",
    ),
  );
  expect(undoRequests).toHaveLength(2);
  expect(undoRequests[0]?.args).toEqual(undoRequests[1]?.args);
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
  await expect(page.getByText(/Second card ·/)).toBeVisible();
  expect(await requestCount(page, "grade_review")).toBe(2);
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
