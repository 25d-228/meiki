import type { Page } from "@playwright/test";

export async function installMockApi(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const initialState = {
      scheduleVersion: 0,
      completedReviews: 0,
      dueAt: "2026-07-29T09:00:00+00:00",
      suspended: false,
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
      longmixed: {
        prompt:
          "Meetingは الساعة […] に始まる — this deliberately long multilingual prompt keeps 日本語, العربية, and English context readable without changing the stored text or forcing horizontal scrolling.",
        fullSource:
          "Meetingは الساعة 三時 に始まる — this deliberately long multilingual prompt keeps 日本語, العربية, and English context readable without changing the stored text or forcing horizontal scrolling.",
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
    const promptForCard = (
      cardId: string,
      fixture: (typeof fixtures)[keyof typeof fixtures],
    ) =>
      cardId === "new-card"
        ? `Second card · ${fixture.prompt}`
        : fixture.prompt;
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
    const mediaFixture = (
      role: "prompt_audio" | "answer_audio" | "reveal_image",
      availability: "ready" | "missing" = "ready",
    ) => {
      const image = role === "reveal_image";
      return {
        id: `${role}-e2e`,
        content_hash: `sha256:${role.padEnd(64, "0").slice(0, 64)}`,
        kind: image ? "image" : "audio",
        role,
        media_type: image ? "image/png" : "audio/wav",
        byte_size: 68,
        original_file_name: image ? "library.png" : `${role}.wav`,
        alt_text: image ? "A quiet library reading room" : null,
        width: image ? 1 : null,
        height: image ? 1 : null,
        duration_ms: image ? null : 1000,
        language_tag: null,
        direction: "auto",
        asset_path:
          availability === "ready"
            ? image
              ? "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
              : "data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQAAAAA="
            : null,
        availability,
      };
    };
    let schedulerSettings = {
      deck_id: "default-deck",
      intensity: "balanced",
      target_retention_basis_points: 9000,
      new_cards_per_day: 20,
      daily_time_budget_minutes: null as number | null,
      maximum_interval_days: 36500,
      day_boundary_minutes: 240,
      engine_version: "fsrs-7",
      active_parameter_set_id: "fsrs7-default-v1",
      previous_parameter_set_id: null as string | null,
      optimizer_status: "never_run",
      optimizer_diagnostics: null as string | null,
    };
    let failedCheck = false;
    let failedGrade = false;

    window.__MEIKI_TEST_PICK_FILE__ = async (role) => `/mock/${role}`;
    HTMLMediaElement.prototype.play = async function () {
      localStorage.setItem(
        "meiki-e2e-media-play-count",
        String(
          Number(localStorage.getItem("meiki-e2e-media-play-count") ?? "0") + 1,
        ),
      );
    };

    window.__MEIKI_TEST_INVOKE__ = async (command, args) => {
      const state = readState();
      const fixtureName = new URLSearchParams(location.search).get("fixture");
      const requestedMedia = new URLSearchParams(location.search).get("media");
      const fixture = selectedFixture();
      if (command === "get_today_overview") {
        const request = (
          args as {
            request: {
              deck_id: string;
            };
          }
        ).request;
        const scenario =
          new URLSearchParams(location.search).get("today") ?? "normal";
        const queueCard = (
          cardId: string,
          overdue: boolean,
          isNew: boolean,
        ) => ({
          card_id: cardId,
          deck_id: request.deck_id,
          due_at: overdue
            ? "2026-07-28T09:00:00+00:00"
            : "2026-07-30T09:00:00+00:00",
          ideal_due_at: overdue
            ? "2026-07-27T09:00:00+00:00"
            : "2026-07-30T09:00:00+00:00",
          overdue,
          is_new: isNew,
        });
        const decks = [
          { id: "default-deck", name: "Japanese" },
          { id: "travel-deck", name: "Travel phrases" },
        ];
        if (scenario === "empty") {
          return {
            deck_id: request.deck_id,
            deck_name:
              decks.find((deck) => deck.id === request.deck_id)?.name ??
              "Japanese",
            decks,
            due_reviews: 0,
            overdue_reviews: 0,
            new_cards: 0,
            deferred_new_cards: 0,
            estimated_seconds: 0,
            estimate_uses_history: false,
            response_time_samples: 0,
            daily_time_budget_minutes:
              schedulerSettings.daily_time_budget_minutes,
            next_due_at: "2026-08-01T09:00:00+00:00",
            queue: [],
          };
        }

        const due =
          scenario === "overdue"
            ? [
                queueCard("overdue-card", true, false),
                queueCard("due-card", false, false),
              ]
            : [queueCard("due-card", false, false)];
        const availableNew =
          scenario === "capped" || scenario === "budget" ? 3 : 1;
        const forcedBudget = scenario === "capped" ? 1 : null;
        const budget =
          forcedBudget ?? schedulerSettings.daily_time_budget_minutes;
        const newByDailyLimit = Math.min(
          availableNew,
          schedulerSettings.new_cards_per_day,
        );
        const budgetRemaining =
          budget === null
            ? Number.POSITIVE_INFINITY
            : Math.max(0, budget * 60 - due.length * 20);
        const selectedNew = Math.min(
          newByDailyLimit,
          Math.floor(budgetRemaining / 30),
        );
        const newCards = Array.from({ length: selectedNew }, (_, index) =>
          queueCard(
            index === 0 ? "new-card" : `new-card-${index + 1}`,
            false,
            true,
          ),
        );
        return {
          deck_id: request.deck_id,
          deck_name:
            decks.find((deck) => deck.id === request.deck_id)?.name ??
            "Japanese",
          decks,
          due_reviews: due.length,
          overdue_reviews: due.filter((card) => card.overdue).length,
          new_cards: newCards.length,
          deferred_new_cards: availableNew - newCards.length,
          estimated_seconds: due.length * 20 + newCards.length * 30,
          estimate_uses_history: scenario !== "empty",
          response_time_samples: scenario === "empty" ? 0 : 8,
          daily_time_budget_minutes: budget,
          next_due_at: null,
          queue: [...due, ...newCards],
        };
      }
      if (command === "initialize_collection" || command === "get_study_card") {
        if (fixtureName === "error") {
          throw new Error("The local collection is temporarily unavailable.");
        }
        if (fixtureName === "loading") {
          await new Promise((resolve) => setTimeout(resolve, 350));
        }
        const cardId =
          command === "get_study_card"
            ? (args as { cardId: string }).cardId
            : "sample-card";
        return {
          card_id: cardId,
          card_content_version: 0,
          schedule_version: state.scheduleVersion,
          prompt: promptForCard(cardId, fixture),
          language_tag: fixture.languageTag,
          direction: fixture.direction,
          due_at: state.dueAt,
          completed_reviews: state.completedReviews,
          suspended: state.suspended ?? false,
          hint: null,
          prompt_media: requestedMedia
            ? [
                mediaFixture(
                  "prompt_audio",
                  requestedMedia === "missing" ? "missing" : "ready",
                ),
              ]
            : [],
        };
      }
      if (command === "check_answer") {
        if (
          new URLSearchParams(location.search).get("failure") === "check" &&
          !failedCheck
        ) {
          failedCheck = true;
          throw new Error("The answer check was interrupted.");
        }
        const request = (
          args as { request: { card_id: string; raw_response: string } }
        ).request;
        const normalizedResponse = request.raw_response.trim().normalize("NFC");
        const exact = normalizedResponse === fixture.answer;
        const answerStart = fixture.fullSource.indexOf(fixture.answer);
        return {
          card_id: request.card_id,
          card_content_version: 0,
          schedule_version: state.scheduleVersion,
          full_source: fixture.fullSource,
          source_segments: [
            {
              text: fixture.fullSource.slice(0, answerStart),
              highlighted: false,
            },
            { text: fixture.answer, highlighted: true },
            {
              text: fixture.fullSource.slice(
                answerStart + fixture.answer.length,
              ),
              highlighted: false,
            },
          ],
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
          grade_previews: [
            { grade: "again", due_at: state.dueAt, interval_seconds: 60 },
            { grade: "hard", due_at: state.dueAt, interval_seconds: 3600 },
            { grade: "good", due_at: state.dueAt, interval_seconds: 259200 },
            { grade: "easy", due_at: state.dueAt, interval_seconds: 604800 },
          ],
          annotations: [],
          explanation: null,
          answer_media:
            requestedMedia === "ready"
              ? [mediaFixture("answer_audio"), mediaFixture("reveal_image")]
              : requestedMedia === "missing"
                ? [mediaFixture("reveal_image", "missing")]
                : [],
        };
      }
      if (command === "grade_review") {
        if (
          new URLSearchParams(location.search).get("failure") === "grade" &&
          !failedGrade
        ) {
          failedGrade = true;
          throw new Error("The review commit was interrupted.");
        }
        localStorage.setItem(
          "meiki-e2e-last-grade-request",
          JSON.stringify((args as { request: unknown }).request),
        );
        const request = (args as { request: { review_event_id: string } })
          .request;
        const nextState = {
          scheduleVersion: state.scheduleVersion + 1,
          completedReviews: state.completedReviews + 1,
          dueAt: "2026-08-01T09:00:00+00:00",
          suspended: false,
        };
        localStorage.setItem("meiki-e2e-state", JSON.stringify(nextState));
        return {
          review_event_id: request.review_event_id,
          schedule_version: nextState.scheduleVersion,
          due_at: nextState.dueAt,
          interval_seconds: 259200,
        };
      }
      if (command === "suspend_card") {
        const request = (args as { request: { card_id: string } }).request;
        const nextState = { ...state, suspended: true };
        localStorage.setItem("meiki-e2e-state", JSON.stringify(nextState));
        return {
          card_id: request.card_id,
          card_content_version: 0,
          schedule_version: nextState.scheduleVersion,
          prompt: promptForCard(request.card_id, fixture),
          language_tag: fixture.languageTag,
          direction: fixture.direction,
          due_at: nextState.dueAt,
          completed_reviews: nextState.completedReviews,
          suspended: true,
          hint: null,
          prompt_media: [],
        };
      }
      if (command === "undo_review") {
        const request = (args as { request: { undo_event_id: string } })
          .request;
        const nextState = {
          ...state,
          scheduleVersion: state.scheduleVersion + 1,
          completedReviews: Math.max(0, state.completedReviews - 1),
          dueAt: initialState.dueAt,
        };
        localStorage.setItem("meiki-e2e-state", JSON.stringify(nextState));
        return {
          undo_event_id: request.undo_event_id,
          schedule_version: nextState.scheduleVersion,
          due_at: nextState.dueAt,
          interval_seconds: 0,
          completed_reviews: nextState.completedReviews,
        };
      }
      if (command === "get_authoring_draft_for_card") {
        return {
          source_id: "sample-source",
          deck_id: "default-deck",
          persisted: true,
          created_at_ms: Date.now(),
          deck_language_tag: fixture.languageTag,
          deck_direction: fixture.direction,
          deck_matching_policy: "strict",
          language_tag: fixture.languageTag,
          direction: fixture.direction,
          segments: [
            {
              id: "segment-context",
              ordinal: 0,
              kind: "text",
              text: fixture.fullSource.slice(
                0,
                fixture.fullSource.indexOf(fixture.answer),
              ),
              cloze_id: null,
            },
            {
              id: "segment-cloze",
              ordinal: 1,
              kind: "cloze",
              text: fixture.answer,
              cloze_id: "sample-cloze",
            },
            {
              id: "segment-after",
              ordinal: 2,
              kind: "text",
              text: fixture.fullSource.slice(
                fixture.fullSource.indexOf(fixture.answer) +
                  fixture.answer.length,
              ),
              cloze_id: null,
            },
          ],
          clozes: [
            {
              id: "sample-cloze",
              card_id: "sample-card",
              answer: fixture.answer,
              accepted_answers: [],
              hint: "",
              language_tag: fixture.languageTag,
              direction: fixture.direction,
              matching_policy: null,
              annotations: [],
              explanation_markdown: "",
              media: [],
            },
          ],
          active_cloze_id: "sample-cloze",
        };
      }
      if (command === "get_scheduler_settings") {
        return copy(schedulerSettings);
      }
      if (command === "update_scheduler_settings") {
        const request = (
          args as {
            request: {
              intensity: string;
              target_retention_basis_points: number;
              new_cards_per_day: number;
              daily_time_budget_minutes: number | null;
              maximum_interval_days: number;
              day_boundary_minutes: number;
            };
          }
        ).request;
        schedulerSettings = { ...schedulerSettings, ...request };
        return copy(schedulerSettings);
      }
      if (command === "optimize_scheduler") {
        schedulerSettings = {
          ...schedulerSettings,
          optimizer_status: "insufficient_data",
          optimizer_diagnostics:
            '{"result":"insufficient_data","reviews":0,"minimum":64}',
        };
        return copy(schedulerSettings);
      }
      if (command === "rollback_scheduler") {
        schedulerSettings = {
          ...schedulerSettings,
          optimizer_status: "rolled_back",
        };
        return copy(schedulerSettings);
      }
      if (command === "rebuild_scheduler") {
        return {
          backup_path: "/tmp/collection.scheduler-rebuild.bak",
          rebuilt_cards: 1,
        };
      }
      if (command === "export_scheduler_diagnostics") {
        return {
          path: "/tmp/collection.scheduler-diagnostics.json",
        };
      }
      if (command === "new_authoring_draft") {
        return newDraft();
      }
      if (command === "import_media") {
        const request = (
          args as {
            request: {
              role: "prompt_audio" | "answer_audio" | "reveal_image";
            };
          }
        ).request;
        return {
          ...mediaFixture(request.role),
          id: authoringId("media"),
        };
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
          media: [],
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
