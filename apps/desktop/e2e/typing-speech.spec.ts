import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { typingLessons, typingTracks } from "../src/lib/typing-lessons";
import { installMockApi } from "./support/mock-api";

type Voice = { name: string; lang: string; localService: boolean };
type SpeechRequest = {
  text: string;
  lang: string;
  voiceName: string;
  voiceLang: string;
};
type SpeechState = {
  requests: SpeechRequest[];
  cancelCount: number;
  activeCount: number;
  maximumActiveCount: number;
};

test.beforeEach(async ({ page }) => {
  await installMockApi(page);
});

async function installSpeechSynthesis(
  page: Page,
  voices: Voice[],
): Promise<void> {
  await page.addInitScript((initialVoices) => {
    type TestUtterance = {
      text: string;
      lang: string;
      voice: Voice | null;
      onend: (() => void) | null;
      onerror: (() => void) | null;
    };
    type SpeechTestWindow = Window & {
      __MEIKI_SPEECH_STATE__: SpeechState;
      __MEIKI_FINISH_SPEECH__: (failed?: boolean) => void;
      __MEIKI_SET_SPEECH_VOICES__: (nextVoices: Voice[]) => void;
    };

    const testWindow = window as SpeechTestWindow;
    let availableVoices = initialVoices;
    let activeUtterance: TestUtterance | null = null;
    const voiceListeners = new Set<EventListener>();
    const state: SpeechState = {
      requests: [],
      cancelCount: 0,
      activeCount: 0,
      maximumActiveCount: 0,
    };

    class TestSpeechSynthesisUtterance implements TestUtterance {
      text: string;
      lang = "";
      voice: Voice | null = null;
      onend: (() => void) | null = null;
      onerror: (() => void) | null = null;

      constructor(text: string) {
        this.text = text;
      }
    }

    const synthesis = {
      getVoices: () => availableVoices,
      speak: (utterance: TestUtterance) => {
        activeUtterance = utterance;
        state.activeCount += 1;
        state.maximumActiveCount = Math.max(
          state.maximumActiveCount,
          state.activeCount,
        );
        state.requests.push({
          text: utterance.text,
          lang: utterance.lang,
          voiceName: utterance.voice?.name ?? "",
          voiceLang: utterance.voice?.lang ?? "",
        });
      },
      cancel: () => {
        state.cancelCount += 1;
        activeUtterance = null;
        state.activeCount = 0;
      },
      addEventListener: (type: string, listener: EventListener) => {
        if (type === "voiceschanged") voiceListeners.add(listener);
      },
      removeEventListener: (type: string, listener: EventListener) => {
        if (type === "voiceschanged") voiceListeners.delete(listener);
      },
    };

    Object.defineProperty(window, "SpeechSynthesisUtterance", {
      configurable: true,
      value: TestSpeechSynthesisUtterance,
    });
    Object.defineProperty(window, "speechSynthesis", {
      configurable: true,
      value: synthesis,
    });
    testWindow.__MEIKI_SPEECH_STATE__ = state;
    testWindow.__MEIKI_FINISH_SPEECH__ = (failed = false) => {
      const utterance = activeUtterance;
      if (!utterance) return;
      activeUtterance = null;
      state.activeCount = 0;
      if (failed) utterance.onerror?.();
      else utterance.onend?.();
    };
    testWindow.__MEIKI_SET_SPEECH_VOICES__ = (nextVoices) => {
      availableVoices = nextVoices;
      for (const listener of voiceListeners) {
        listener(new Event("voiceschanged"));
      }
    };
  }, voices);
}

async function installUnavailableSpeechSynthesis(page: Page): Promise<void> {
  await page.addInitScript(() => {
    Object.defineProperty(window, "SpeechSynthesisUtterance", {
      configurable: true,
      value: undefined,
    });
    Object.defineProperty(window, "speechSynthesis", {
      configurable: true,
      value: undefined,
    });
  });
}

async function openTyping(page: Page): Promise<void> {
  await page.goto("/");
  const openNavigation = page.getByRole("button", { name: "Open navigation" });
  if (await openNavigation.isVisible()) await openNavigation.click();
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: "Typing", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Typing", level: 1 }),
  ).toBeVisible();
}

async function startPractice(page: Page): Promise<Locator> {
  await page.getByRole("button", { name: "Start practice" }).click();
  const input = page.getByLabel("Practice input");
  await expect(input).toBeVisible();
  return input;
}

async function openLesson(page: Page, lessonId: string): Promise<Locator> {
  const lesson = typingLessons.find((candidate) => candidate.id === lessonId);
  if (!lesson) throw new Error(`Missing Typing lesson ${lessonId}`);
  const languageLessons = typingLessons.filter(
    (candidate) => candidate.language === lesson.language,
  );
  const lessonIndex = languageLessons.findIndex(
    (candidate) => candidate.id === lessonId,
  );
  const precedingIds = languageLessons
    .slice(0, lessonIndex)
    .map((candidate) => candidate.id);
  await page.addInitScript(
    ({ language, completedIds }) => {
      localStorage.setItem("meiki-typing-language", language);
      localStorage.setItem(
        "meiki-typing-completed",
        JSON.stringify(completedIds),
      );
    },
    { language: lesson.language, completedIds: precedingIds },
  );
  await openTyping(page);
  let input = await startPractice(page);
  for (const completedId of precedingIds) {
    const completedLesson = typingLessons.find(
      (candidate) => candidate.id === completedId,
    );
    if (!completedLesson) continue;
    await expect(
      page.getByRole("heading", { name: completedLesson.title, level: 2 }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Next" }).click();
    input = page.getByLabel("Practice input");
  }
  await expect(
    page.getByRole("heading", { name: lesson.title, level: 2 }),
  ).toBeVisible();
  return input;
}

async function dispatchKey(
  input: Locator,
  type: "keydown" | "keyup",
  init: {
    code: string;
    key: string;
    repeat?: boolean;
    shiftKey?: boolean;
    isComposing?: boolean;
  },
): Promise<void> {
  await input.evaluate(
    (element, event) =>
      element.dispatchEvent(
        new KeyboardEvent(event.type, {
          bubbles: true,
          code: event.code,
          key: event.key,
          repeat: event.repeat,
          shiftKey: event.shiftKey,
          isComposing: event.isComposing,
        }),
      ),
    { type, ...init },
  );
}

async function speechState(page: Page): Promise<SpeechState> {
  return page.evaluate(
    () =>
      (
        window as Window & {
          __MEIKI_SPEECH_STATE__: SpeechState;
        }
      ).__MEIKI_SPEECH_STATE__,
  );
}

async function finishSpeech(page: Page, failed = false): Promise<void> {
  await page.evaluate(
    (shouldFail) =>
      (
        window as Window & {
          __MEIKI_FINISH_SPEECH__: (failed?: boolean) => void;
        }
      ).__MEIKI_FINISH_SPEECH__(shouldFail),
    failed,
  );
}

test("physical character sound uses active legends without changing correctness feedback", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Korean local", lang: "ko", localService: true },
  ]);
  await openTyping(page);
  const input = await startPractice(page);

  await dispatchKey(input, "keydown", { code: "KeyF", key: "f" });
  await expect(page.locator("#typing-live-status")).toContainText(
    "Pressed F. Expected Q. Try again.",
  );
  expect((await speechState(page)).requests).toEqual([
    {
      text: "ㄹ",
      lang: "ko",
      voiceName: "Korean local",
      voiceLang: "ko",
    },
  ]);
  await finishSpeech(page);

  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  await finishSpeech(page);
  await dispatchKey(input, "keydown", { code: "ShiftLeft", key: "Shift" });
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "Q" });
  expect((await speechState(page)).requests.map(({ text }) => text)).toEqual([
    "ㄹ",
    "ㅂ",
    "ㅃ",
  ]);

  await dispatchKey(input, "keydown", {
    code: "KeyQ",
    key: "Q",
    repeat: true,
  });
  await dispatchKey(input, "keyup", { code: "KeyQ", key: "Q" });
  await dispatchKey(input, "keyup", { code: "ShiftLeft", key: "Shift" });
  expect((await speechState(page)).requests).toHaveLength(3);
});

test("Russian physical keys use Cyrillic legends and local primary-language fallback", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Russian remote", lang: "ru", localService: false },
    { name: "Russian local", lang: "ru-RU", localService: true },
  ]);
  const input = await openLesson(page, "typing-russian-top-row");
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  expect((await speechState(page)).requests).toEqual([
    {
      text: "й",
      lang: "ru",
      voiceName: "Russian local",
      voiceLang: "ru-RU",
    },
  ]);
  await finishSpeech(page);
  await dispatchKey(input, "keyup", { code: "KeyQ", key: "q" });
  await dispatchKey(input, "keydown", { code: "ShiftLeft", key: "Shift" });
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "Q" });
  expect((await speechState(page)).requests.at(-1)?.text).toBe("Й");
});

for (const example of [
  { lessonId: "typing-korean-syllable-blocks", text: "한", tag: "ko" },
  { lessonId: "typing-japanese-basic-hiragana", text: "あ", tag: "ja" },
  { lessonId: "typing-french-acute-e", text: "é", tag: "fr" },
  { lessonId: "typing-spanish-tilde-n", text: "ñ", tag: "es" },
  { lessonId: "typing-german-diaeresis-a", text: "ä", tag: "de" },
  { lessonId: "typing-portuguese-acute-a", text: "á", tag: "pt" },
  { lessonId: "typing-chinese-nihao", text: "你", tag: "zh-Hans" },
] as const) {
  test(`${example.lessonId} pronounces newly committed graphemes`, async ({
    page,
  }) => {
    await installSpeechSynthesis(page, [
      { name: "Matching local", lang: example.tag, localService: true },
    ]);
    const input = await openLesson(page, example.lessonId);
    await input.fill(example.text);
    expect((await speechState(page)).requests).toEqual([
      {
        text: example.text,
        lang: example.tag,
        voiceName: "Matching local",
        voiceLang: example.tag,
      },
    ]);
  });
}

test("composition speaks only committed graphemes once and deletion stays silent", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Japanese local", lang: "ja", localService: true },
  ]);
  const input = await openLesson(page, "typing-japanese-basic-hiragana");

  await input.dispatchEvent("compositionstart", { data: "" });
  await input.dispatchEvent("compositionupdate", { data: "nihon" });
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "日本";
  });
  expect((await speechState(page)).requests).toEqual([]);
  await input.dispatchEvent("compositionend", { data: "日本" });
  expect((await speechState(page)).requests.map(({ text }) => text)).toEqual([
    "日",
  ]);
  await finishSpeech(page);
  expect((await speechState(page)).requests.map(({ text }) => text)).toEqual([
    "日",
    "本",
  ]);
  await finishSpeech(page);

  await input.fill("日");
  expect((await speechState(page)).requests).toHaveLength(2);
  await input.fill("日本語");
  expect((await speechState(page)).requests.at(-1)?.text).toBe("本");
  await finishSpeech(page);
  expect((await speechState(page)).requests.at(-1)?.text).toBe("語");
});

test("dead-key and IME intermediate input stay silent until the grapheme commits", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "French local", lang: "fr", localService: true },
  ]);
  const input = await openLesson(page, "typing-french-acute-e");
  await input.dispatchEvent("compositionstart", { data: "" });
  await input.dispatchEvent("compositionupdate", { data: "´" });
  await dispatchKey(input, "keydown", {
    code: "Quote",
    key: "Dead",
    isComposing: true,
  });
  expect((await speechState(page)).requests).toEqual([]);

  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "e\u0301";
  });
  await input.dispatchEvent("compositionend", { data: "é" });
  expect((await speechState(page)).requests.map(({ text }) => text)).toEqual([
    "é",
  ]);
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
});

test("Pinyin composition stays silent until Chinese graphemes commit", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Chinese local", lang: "zh-CN", localService: true },
  ]);
  const input = await openLesson(page, "typing-chinese-nihao");
  await input.dispatchEvent("compositionstart", { data: "" });
  await input.dispatchEvent("compositionupdate", { data: "nihao" });
  for (const [code, key] of [
    ["KeyN", "n"],
    ["KeyI", "i"],
    ["KeyH", "h"],
  ] as const) {
    await dispatchKey(input, "keydown", {
      code,
      key,
      isComposing: true,
    });
  }
  expect((await speechState(page)).requests).toEqual([]);
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "你好";
  });
  await input.dispatchEvent("compositionend", { data: "你好" });
  expect((await speechState(page)).requests[0]).toMatchObject({
    text: "你",
    lang: "zh-Hans",
    voiceLang: "zh-CN",
  });
  await finishSpeech(page);
  expect((await speechState(page)).requests[1]?.text).toBe("好");
});

test("exact local voices win while remote and unrelated voices are never used", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Remote exact", lang: "ko", localService: false },
    { name: "Local primary", lang: "ko-KR", localService: true },
    { name: "Local exact", lang: "ko", localService: true },
    { name: "Unrelated", lang: "ja", localService: true },
  ]);
  await openTyping(page);
  const input = await startPractice(page);
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  expect((await speechState(page)).requests[0]?.voiceName).toBe("Local exact");
});

test("delayed local voices enable sound and missing voices leave practice usable", async ({
  page,
}) => {
  await installSpeechSynthesis(page, []);
  await openTyping(page);
  const input = await startPractice(page);
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  await expect(page.locator("#typing-speech-status")).toHaveText(
    "Character sound is unavailable for this language.",
  );
  await expect(page.locator("#typing-live-status")).toContainText(
    "Correct position.",
  );

  await page.evaluate(() =>
    (
      window as Window & {
        __MEIKI_SET_SPEECH_VOICES__: (voices: Voice[]) => void;
      }
    ).__MEIKI_SET_SPEECH_VOICES__([
      { name: "Korean local", lang: "ko", localService: true },
    ]),
  );
  expect((await speechState(page)).requests[0]?.text).toBe("ㅂ");
  await expect(page.locator("#typing-speech-status")).toHaveCount(0);
});

test("unavailable synthesis keeps the exercise accessible and usable", async ({
  page,
}) => {
  await installUnavailableSpeechSynthesis(page);
  await openTyping(page);
  const input = await startPractice(page);
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  await expect(page.locator("#typing-speech-status")).toHaveText(
    "Character sound is unavailable for this language.",
  );
  await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
  expect(
    (await new AxeBuilder({ page }).include("main").analyze()).violations,
  ).toEqual([]);
});

test("speech errors leave correctness and physical state unchanged", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Korean local", lang: "ko", localService: true },
  ]);
  await openTyping(page);
  const input = await startPractice(page);
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  await finishSpeech(page, true);
  await expect(page.locator("#typing-speech-status")).toHaveText(
    "Character sound could not be played.",
  );
  await expect(page.locator("#typing-live-status")).toContainText(
    "Correct position.",
  );
});

test("remote and unrelated voices are rejected without blocking practice", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Korean remote", lang: "ko", localService: false },
    { name: "Japanese local", lang: "ja", localService: true },
  ]);
  await openTyping(page);
  const input = await startPractice(page);
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  expect((await speechState(page)).requests).toEqual([]);
  await expect(page.locator("#typing-speech-status")).toHaveText(
    "Character sound is unavailable for this language.",
  );
  await expect(page.locator("#typing-live-status")).toContainText(
    "Correct position.",
  );
});

test("rapid input remains ordered, non-overlapping, and bounded", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Korean local", lang: "ko", localService: true },
  ]);
  await openTyping(page);
  const input = await startPractice(page);
  for (let index = 0; index < 30; index += 1) {
    const q = index % 2 === 0;
    await dispatchKey(input, "keydown", {
      code: q ? "KeyQ" : "KeyW",
      key: q ? "q" : "w",
    });
  }
  expect((await speechState(page)).requests.map(({ text }) => text)).toEqual([
    "ㅂ",
  ]);
  for (let index = 0; index < 20; index += 1) await finishSpeech(page);
  const state = await speechState(page);
  expect(state.maximumActiveCount).toBe(1);
  expect(state.requests).toHaveLength(17);
  expect(state.requests.slice(1).map(({ text }) => text)).toEqual(
    Array.from({ length: 16 }, (_, index) => (index % 2 === 0 ? "ㅂ" : "ㅈ")),
  );
});

test("Retry, lesson and language changes, and navigation cancel Typing speech without speaking", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Korean local", lang: "ko", localService: true },
  ]);
  await openTyping(page);
  let input = await startPractice(page);
  expect((await speechState(page)).requests).toEqual([]);
  await page.getByRole("button", { name: "Retry" }).click();
  expect((await speechState(page)).requests).toEqual([]);

  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  const cancelBeforeRetry = (await speechState(page)).cancelCount;
  await page.getByRole("button", { name: "Retry" }).click();
  expect((await speechState(page)).cancelCount).toBe(cancelBeforeRetry + 1);

  input = page.getByLabel("Practice input");
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  const afterLanguageChange = await speechState(page);
  expect(afterLanguageChange.cancelCount).toBe(cancelBeforeRetry + 2);
  expect(afterLanguageChange.requests).toHaveLength(2);

  await page.getByRole("button", { name: "Start practice" }).click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  expect((await speechState(page)).cancelCount).toBe(cancelBeforeRetry + 3);
});

test("changing a lesson cancels active pronunciation without speaking the new target", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Korean local", lang: "ko", localService: true },
  ]);
  await page.addInitScript(() => {
    localStorage.setItem(
      "meiki-typing-completed",
      JSON.stringify(["typing-korean-basic-consonants"]),
    );
    localStorage.setItem("meiki-vim-keybindings", "true");
  });
  await openTyping(page);
  const input = await startPractice(page);
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  const cancelBeforeChange = (await speechState(page)).cancelCount;
  await page.getByRole("button", { name: "Next" }).click();
  expect((await speechState(page)).cancelCount).toBe(cancelBeforeChange + 1);
  expect((await speechState(page)).requests.map(({ text }) => text)).toEqual([
    "ㅂ",
  ]);
  await expect(
    page.getByRole("heading", { name: "Basic vowels", level: 2 }),
  ).toBeVisible();
  const cancelBeforePrevious = (await speechState(page)).cancelCount;
  await page.locator("main").press("h");
  expect((await speechState(page)).cancelCount).toBe(cancelBeforePrevious + 1);
  expect((await speechState(page)).requests).toHaveLength(1);
  await expect(
    page.getByRole("heading", { name: "Basic consonants", level: 2 }),
  ).toBeVisible();
});

test("Vim input and Retry retain their existing behavior around character sound", async ({
  page,
}) => {
  await installSpeechSynthesis(page, [
    { name: "Korean local", lang: "ko", localService: true },
  ]);
  await page.addInitScript(() => {
    localStorage.setItem("meiki-vim-keybindings", "true");
  });
  await openTyping(page);
  await startPractice(page);
  const input = page.getByLabel("Practice input");
  await input.press("Escape");
  await expect(page.getByLabel("Vim mode NORMAL")).toBeVisible();
  await page.locator("main").press("i");
  await expect(input).toBeFocused();
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  expect((await speechState(page)).requests[0]?.text).toBe("ㅂ");
  await input.press("Escape");
  const cancelBeforeRetry = (await speechState(page)).cancelCount;
  await page.locator("main").press("r");
  expect((await speechState(page)).cancelCount).toBe(cancelBeforeRetry + 1);
  await expect(page.getByTestId("typing-physical-trail")).toHaveText(
    "None yet",
  );
});

test("automatic character sound adds no controls, backend commands, network, Arabic, or narrow overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 760 });
  await installSpeechSynthesis(page, [
    { name: "Korean local", lang: "ko", localService: true },
  ]);
  const runtimeRequests: string[] = [];
  page.on("request", (request) => {
    if (["fetch", "xhr"].includes(request.resourceType())) {
      runtimeRequests.push(request.url());
    }
  });
  await openTyping(page);
  const existingCommands = await page.evaluate(
    () => window.__MEIKI_TEST_REQUESTS__?.length ?? 0,
  );
  const input = await startPractice(page);
  await dispatchKey(input, "keydown", { code: "KeyQ", key: "q" });
  await expect(
    page.getByRole("button", { name: /hear|pronounc/i }),
  ).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Arabic/i })).toHaveCount(0);
  await expect(
    page.getByRole("group", { name: "Language" }).getByRole("button"),
  ).toHaveCount(8);
  expect(typingTracks.map(({ language }) => language)).not.toContain("arabic");
  expect(runtimeRequests).toEqual([]);
  expect(
    await page.evaluate(() => window.__MEIKI_TEST_REQUESTS__?.length ?? 0),
  ).toBe(existingCommands);
  const requestCountBeforeSandbox = (await speechState(page)).requests.length;
  await page.getByRole("button", { name: "Japanese — Romaji input" }).click();
  await page.getByLabel("Conversion sandbox input").fill("nihongo");
  expect((await speechState(page)).requests).toHaveLength(
    requestCountBeforeSandbox,
  );
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});
