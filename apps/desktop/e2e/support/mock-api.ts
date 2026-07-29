import type { Page } from "@playwright/test";

export async function installMockApi(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const initialState = {
      scheduleVersion: 0,
      completedReviews: 0,
      dueAt: "2026-07-29T09:00:00+00:00",
    };
    const fixtures = {
      cjk: {
        prompt: "日曜日は図書館に[…]",
        fullSource: "日曜日は図書館に行きます",
        answer: "行きます",
        languageTag: "ja",
        direction: "auto",
      },
      devanagari: {
        prompt: "मैं […] पढ़ता हूँ",
        fullSource: "मैं पुस्तक पढ़ता हूँ",
        answer: "पुस्तक",
        languageTag: "hi",
        direction: "ltr",
      },
      emoji: {
        prompt: "Family: […]",
        fullSource: "Family: 👨‍👩‍👧‍👦",
        answer: "👨‍👩‍👧‍👦",
        languageTag: null,
        direction: "ltr",
      },
      ltr: {
        prompt: "Le dimanche, je vais à […]",
        fullSource: "Le dimanche, je vais à la bibliothèque",
        answer: "la bibliothèque",
        languageTag: "fr",
        direction: "ltr",
      },
      rtl: {
        prompt: "من هر روز […] می‌خوانم",
        fullSource: "من هر روز کتاب می‌خوانم",
        answer: "کتاب",
        languageTag: "fa",
        direction: "rtl",
      },
      mixed: {
        prompt: "Meetingは الساعة […] に始まる",
        fullSource: "Meetingは الساعة 三時 に始まる",
        answer: "三時",
        languageTag: null,
        direction: "auto",
      },
    } as const;

    const readState = () => {
      const value = localStorage.getItem("meiki-e2e-state");
      return value ? JSON.parse(value) : initialState;
    };
    const selectedFixture = () => {
      const name = new URLSearchParams(location.search).get("fixture") ?? "cjk";
      return fixtures[name as keyof typeof fixtures] ?? fixtures.cjk;
    };
    let nextAuthoringId = 0;
    const authoringId = (kind: string) => `${kind}-e2e-${++nextAuthoringId}`;
    const newDraft = () => ({
      source_id: authoringId("source"),
      deck_id: "default-deck",
      persisted: false,
      created_at_ms: Date.now(),
      deck_language_tag: null,
      deck_direction: "auto",
      deck_matching_policy: "strict",
      language_tag: null,
      direction: "auto",
      segments: [
        {
          id: authoringId("segment"),
          ordinal: 0,
          kind: "text",
          text: "",
          cloze_id: null,
        },
      ],
      clozes: [],
      active_cloze_id: null,
    });
    const renumber = <T extends { ordinal: number }>(segments: T[]) =>
      segments.map((segment, ordinal) => ({ ...segment, ordinal }));
    const graphemeBoundaries = (text: string) => {
      const segmenter = new Intl.Segmenter(undefined, {
        granularity: "grapheme",
      });
      return new Set([
        ...Array.from(segmenter.segment(text), (part) => part.index),
        text.length,
      ]);
    };
    const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;

    window.__MEIKI_TEST_INVOKE__ = async (command, args) => {
      const state = readState();
      const fixtureName = new URLSearchParams(location.search).get("fixture");
      const fixture = selectedFixture();
      if (command === "initialize_collection" || command === "get_study_card") {
        if (fixtureName === "error") {
          throw new Error("The local collection is temporarily unavailable.");
        }
        if (fixtureName === "loading") {
          await new Promise((resolve) => setTimeout(resolve, 350));
        }
        return {
          card_id: "sample-card",
          card_content_version: 0,
          schedule_version: state.scheduleVersion,
          prompt: fixture.prompt,
          language_tag: fixture.languageTag,
          direction: fixture.direction,
          due_at: state.dueAt,
          completed_reviews: state.completedReviews,
        };
      }
      if (command === "check_answer") {
        const request = (args as { request: { raw_response: string } }).request;
        const normalizedResponse = request.raw_response.trim().normalize("NFC");
        const exact = normalizedResponse === fixture.answer;
        return {
          card_id: "sample-card",
          card_content_version: 0,
          schedule_version: state.scheduleVersion,
          full_source: fixture.fullSource,
          expected_answer: fixture.answer,
          raw_response: request.raw_response,
          normalized_response: normalizedResponse,
          comparison: exact ? "exact" : "incorrect",
          difference: exact
            ? [{ kind: "equal", text: fixture.answer }]
            : [
                { kind: "delete", text: fixture.answer },
                { kind: "insert", text: normalizedResponse },
              ],
          suggested_grade: exact ? "good" : "again",
        };
      }
      if (command === "grade_review") {
        const nextState = {
          scheduleVersion: state.scheduleVersion + 1,
          completedReviews: state.completedReviews + 1,
          dueAt: "2026-08-01T09:00:00+00:00",
        };
        localStorage.setItem("meiki-e2e-state", JSON.stringify(nextState));
        return {
          review_event_id: "review-e2e",
          schedule_version: nextState.scheduleVersion,
          due_at: nextState.dueAt,
          interval_seconds: 259200,
        };
      }
      if (command === "new_authoring_draft") {
        return newDraft();
      }
      if (command === "make_cloze") {
        const request = (
          args as {
            request: {
              draft: ReturnType<typeof newDraft>;
              segment_id: string;
              selection_start_utf16: number;
              selection_end_utf16: number;
            };
          }
        ).request;
        const draft = copy(request.draft);
        const index = draft.segments.findIndex(
          (segment) => segment.id === request.segment_id,
        );
        if (index < 0 || draft.segments[index].kind !== "text")
          throw new Error("The selected segment no longer exists.");
        const segment = draft.segments[index];
        const boundaries = graphemeBoundaries(segment.text);
        if (
          !boundaries.has(request.selection_start_utf16) ||
          !boundaries.has(request.selection_end_utf16)
        ) {
          throw new Error(
            "The text selection splits an extended grapheme cluster.",
          );
        }
        if (request.selection_start_utf16 === request.selection_end_utf16)
          throw new Error("Select at least one complete grapheme.");
        const before = segment.text.slice(0, request.selection_start_utf16);
        const answer = segment.text.slice(
          request.selection_start_utf16,
          request.selection_end_utf16,
        );
        const after = segment.text.slice(request.selection_end_utf16);
        const clozeId = authoringId("cloze");
        const replacements = [];
        if (before)
          replacements.push({
            ...segment,
            text: before,
          });
        replacements.push({
          id: authoringId("segment"),
          ordinal: 0,
          kind: "cloze",
          text: answer,
          cloze_id: clozeId,
        });
        if (after)
          replacements.push({
            id: before ? authoringId("segment") : segment.id,
            ordinal: 0,
            kind: "text",
            text: after,
            cloze_id: null,
          });
        draft.segments.splice(index, 1, ...replacements);
        draft.segments = renumber(draft.segments);
        draft.clozes.push({
          id: clozeId,
          card_id: authoringId("card"),
          answer,
          accepted_answers: [],
          hint: "",
          language_tag: draft.language_tag,
          direction: draft.direction,
          matching_policy: null,
          annotations: [],
          explanation_markdown: "",
        });
        draft.active_cloze_id = clozeId;
        return draft;
      }
      if (command === "remove_cloze") {
        const request = (
          args as {
            request: {
              draft: ReturnType<typeof newDraft>;
              cloze_id: string;
            };
          }
        ).request;
        const draft = copy(request.draft);
        const segment = draft.segments.find(
          (item) => item.cloze_id === request.cloze_id,
        );
        if (!segment) throw new Error("The cloze no longer exists.");
        segment.kind = "text";
        segment.cloze_id = null;
        draft.clozes = draft.clozes.filter(
          (cloze) => cloze.id !== request.cloze_id,
        );
        for (let index = draft.segments.length - 1; index > 0; index -= 1) {
          const previous = draft.segments[index - 1];
          const current = draft.segments[index];
          if (previous.kind === "text" && current.kind === "text") {
            previous.text += current.text;
            draft.segments.splice(index, 1);
          }
        }
        draft.segments = renumber(draft.segments);
        draft.active_cloze_id = draft.clozes[0]?.id ?? null;
        return draft;
      }
      if (command === "reorder_segments") {
        const request = (
          args as {
            request: {
              draft: ReturnType<typeof newDraft>;
              segment_ids: string[];
            };
          }
        ).request;
        const draft = copy(request.draft);
        const byId = new Map(
          draft.segments.map((segment) => [segment.id, segment]),
        );
        draft.segments = renumber(
          request.segment_ids.map((id) => {
            const segment = byId.get(id);
            if (!segment) throw new Error("Unknown segment identity.");
            return segment;
          }),
        );
        return draft;
      }
      if (command === "preview_authoring_draft") {
        const draft = (args as { draft: ReturnType<typeof newDraft> }).draft;
        return draft.clozes.map((cloze) => ({
          cloze_id: cloze.id,
          prompt: draft.segments
            .map((segment) =>
              segment.cloze_id === cloze.id ? "[…]" : segment.text,
            )
            .join(""),
          answer: cloze.answer,
          language_tag: cloze.language_tag ?? draft.language_tag,
          direction:
            cloze.direction === "auto" ? draft.direction : cloze.direction,
          hint: cloze.hint,
          annotations: cloze.annotations,
          explanation_markdown: cloze.explanation_markdown,
        }));
      }
      if (command === "save_authoring_draft") {
        const draft = copy(
          (args as { draft: ReturnType<typeof newDraft> }).draft,
        );
        if (!draft.clozes.length)
          throw new Error("Create at least one cloze before saving.");
        if (
          draft.clozes.some(
            (cloze) =>
              cloze.explanation_markdown.includes("<") ||
              cloze.explanation_markdown.toLowerCase().includes("javascript:"),
          )
        ) {
          throw new Error(
            "Explanations support limited Markdown but not raw HTML or executable links.",
          );
        }
        draft.persisted = true;
        localStorage.setItem("meiki-e2e-authoring", JSON.stringify(draft));
        return draft;
      }
      throw new Error(`Unexpected command: ${command}`);
    };
  });
}
