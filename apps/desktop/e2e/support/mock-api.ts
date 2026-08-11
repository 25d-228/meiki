import type { Page } from "@playwright/test";

import { scenarioDtos } from "./scenario-dtos";

export async function installMockApi(page: Page): Promise<void> {
  await page.addInitScript((dtos) => {
    const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
    const realMp3Source =
      "data:audio/mpeg;base64,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";
    const realMp3Hash =
      "sha256:4732a7cfa0f5dc2a3c8ded1378d2fa4cef6b315dfd0e29ab5479b90a6db13157";
    const realMp3Path =
      "/existing-collection/objects/sha256/47/32a7cfa0f5dc2a3c8ded1378d2fa4cef6b315dfd0e29ab5479b90a6db13157";
    const longBundleLanguage =
      "An exceptionally long language display name for wrapping";
    const calls: Record<string, number> = {};
    const committedReviewEventIds = new Set<string>();
    const deletedDeckIds = new Set<string>();
    const removedDeckCardIds = new Set<string>();
    let deckDeletedToUnsorted = false;
    let deletedDeckCardRestored = false;
    let focusedSessionDeckDeleted = false;
    let bundleImported = false;
    let bundleRemoved =
      localStorage.getItem("meiki-e2e-bundle-removed") === "true";
    let mediaPlayAttempts = 0;
    const schedulerSettingsByDeck: Record<
      string,
      typeof dtos.schedulerSettings
    > = {};

    const mediaMode = new URLSearchParams(location.search).get("media");
    if (mediaMode === "real-mp3" || mediaMode === "transport-error") {
      Reflect.set(globalThis, "isTauri", true);
      Reflect.set(window, "__TAURI_INTERNALS__", {
        convertFileSrc(path: string, protocol = "asset") {
          return `${protocol}://localhost/${encodeURIComponent(path)}`;
        },
      });
      const createObjectUrl = URL.createObjectURL.bind(URL);
      const revokeObjectUrl = URL.revokeObjectURL.bind(URL);
      window.__MEIKI_TEST_CREATED_OBJECT_URLS__ = [];
      window.__MEIKI_TEST_REVOKED_OBJECT_URLS__ = [];
      URL.createObjectURL = (object) => {
        const url = createObjectUrl(object);
        window.__MEIKI_TEST_CREATED_OBJECT_URLS__?.push(url);
        return url;
      };
      URL.revokeObjectURL = (url) => {
        window.__MEIKI_TEST_REVOKED_OBJECT_URLS__?.push(url);
        revokeObjectUrl(url);
      };
    }

    const realMp3Media = (role: "prompt_audio" | "answer_audio") => ({
      ...clone(dtos.media.prompt_audio),
      id: `real-mp3-${role}`,
      content_hash: realMp3Hash,
      role,
      media_type: "audio/mpeg",
      byte_size: 2_445,
      original_file_name: "sentence.mp3",
      duration_ms: 500,
      asset_path: realMp3Path,
    });

    window.__MEIKI_TEST_REQUESTS__ = [];
    window.__MEIKI_TEST_PICK_FILE__ = async (role) => `/fixture/${role}`;
    window.__MEIKI_TEST_PICK_ARCHIVE__ = async () =>
      "/tmp/exports/meiki-e2e.meiki";
    window.__MEIKI_TEST_PICK_SCHEDULER_PARAMETERS__ = async () =>
      "/tmp/meiki-scheduler-parameters.json";
    const nativePlay = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = async function () {
      if (new URLSearchParams(location.search).get("media") === "real-mp3") {
        return nativePlay.call(this);
      }
      mediaPlayAttempts += 1;
      localStorage.setItem(
        "meiki-e2e-media-play-attempt-count",
        String(mediaPlayAttempts),
      );
      if (
        new URLSearchParams(location.search).get("media") === "blocked" &&
        mediaPlayAttempts === 1
      ) {
        throw new DOMException("Autoplay is blocked", "NotAllowedError");
      }
      if (
        new URLSearchParams(location.search).get("media") === "playback-error"
      ) {
        throw new DOMException("Audio decoding failed", "NotSupportedError");
      }
      localStorage.setItem(
        "meiki-e2e-media-play-count",
        String(
          Number(localStorage.getItem("meiki-e2e-media-play-count") ?? "0") + 1,
        ),
      );
      const role =
        this.closest<HTMLElement>("[data-media-role]")?.dataset.mediaRole;
      const playedRoles = JSON.parse(
        localStorage.getItem("meiki-e2e-media-played-roles") ?? "[]",
      ) as string[];
      if (role) playedRoles.push(role);
      localStorage.setItem(
        "meiki-e2e-media-played-roles",
        JSON.stringify(playedRoles),
      );
      this.dispatchEvent(new Event("play"));
    };
    HTMLMediaElement.prototype.pause = function () {
      localStorage.setItem(
        "meiki-e2e-media-pause-count",
        String(
          Number(localStorage.getItem("meiki-e2e-media-pause-count") ?? "0") +
            1,
        ),
      );
      this.dispatchEvent(new Event("pause"));
    };

    window.__MEIKI_TEST_INVOKE__ = async (command, args) => {
      window.__MEIKI_TEST_REQUESTS__?.push({
        command,
        args: clone(args ?? {}),
      });
      calls[command] = (calls[command] ?? 0) + 1;

      const params = new URLSearchParams(location.search);
      const fixtureName = params.get("fixture") ?? "cjk";
      const study =
        dtos.study[fixtureName as keyof typeof dtos.study] ?? dtos.study.cjk;
      const todayName = params.get("today") ?? "normal";
      const overview =
        dtos.today[todayName as keyof typeof dtos.today] ?? dtos.today.normal;
      const authoringName =
        params.get("authoring") ??
        (fixtureName in dtos.authoring ? fixtureName : "cjk");
      const authoring =
        dtos.authoring[authoringName as keyof typeof dtos.authoring] ??
        dtos.authoring.cjk;

      if (
        fixtureName === "error" &&
        (command === "prepare_study" || command === "get_study_card")
      ) {
        throw new Error("The local collection is temporarily unavailable.");
      }
      if (fixtureName === "loading" && command === "get_study_card") {
        await new Promise((resolve) => setTimeout(resolve, 350));
      }
      if (
        params.get("failure") === "check" &&
        command === "check_answer" &&
        calls[command] === 1
      ) {
        throw new Error("The answer check was interrupted.");
      }

      if (command === "read_managed_audio") {
        if (mediaMode === "transport-error") {
          throw new Error("Audio transport failed.");
        }
        const requestedHash = (args as { contentHash?: string })?.contentHash;
        if (requestedHash !== realMp3Hash) {
          throw new Error("Audio transport failed.");
        }
        const encoded = realMp3Source.slice(realMp3Source.indexOf(",") + 1);
        return Array.from(atob(encoded), (character) =>
          character.charCodeAt(0),
        );
      }
      if (
        params.get("failure") === "grade" &&
        command === "grade_review" &&
        calls[command] === 1
      ) {
        throw new Error("The review commit was interrupted.");
      }

      if (command === "get_today_overview") {
        if (params.get("failure") === "today") {
          throw new Error("The local collection is temporarily unavailable.");
        }
        const requestedDeckId = (args as { request?: { deck_id?: string } })
          ?.request?.deck_id;
        let availableDecks = clone(overview.decks);
        if (
          (bundleImported || params.get("bundleRemoval") === "installed") &&
          !bundleRemoved
        ) {
          availableDecks = [
            ...availableDecks,
            ...dtos.bundleDeckSummaries.map(({ id, name }) => ({ id, name })),
          ];
        }
        if (focusedSessionDeckDeleted) {
          availableDecks = availableDecks.filter(
            (deck) => deck.id !== "travel-deck",
          );
        }
        availableDecks = availableDecks.filter(
          (deck) => !deletedDeckIds.has(deck.id),
        );
        const selectedDeck = availableDecks.find(
          (deck) => deck.id === requestedDeckId,
        );
        if (requestedDeckId && requestedDeckId !== "__all_decks__") {
          if (!selectedDeck) {
            throw new Error(`deck ${requestedDeckId} does not exist`);
          }
          return {
            ...clone(overview),
            deck_id: requestedDeckId,
            deck_name: selectedDeck.name,
            decks: availableDecks,
          };
        }
        return { ...clone(overview), decks: availableDecks };
      }
      if (command === "prepare_study") {
        if (params.get("collection") === "empty")
          return clone(dtos.emptyCollectionPlan);
        if (todayName === "empty") return clone(dtos.nothingDuePlan);
        const deckId = (args as { request?: { deck_id?: string } })?.request
          ?.deck_id;
        if (deckId && deckId !== "__all_decks__") {
          return {
            ...clone(dtos.readyPlan),
            overview: {
              ...clone(dtos.readyPlan.overview),
              deck_id: deckId,
              deck_name:
                deckId === "default-deck" ? "Unsorted" : "Travel phrases",
              queue: clone(dtos.readyPlan.overview.queue).map((card) => ({
                ...card,
                deck_id: deckId,
              })),
            },
          };
        }
        return clone(dtos.readyPlan);
      }
      if (command === "reconcile_study_queue") {
        if (params.get("reconcile") === "request") {
          return clone(
            (
              args as {
                request: { entries: typeof dtos.reconciledQueue };
              }
            ).request.entries,
          );
        }
        return clone(
          params.get("reconcile") === "second"
            ? dtos.reconciledSecondCard
            : dtos.reconciledQueue,
        );
      }
      if (command === "get_study_card") {
        if (
          params.get("media") === "real-mp3" ||
          params.get("media") === "transport-error"
        ) {
          const ready = clone(dtos.readyMediaCard);
          return {
            ...ready,
            prompt_media: [realMp3Media("prompt_audio")],
          };
        }
        if (fixtureName === "stale") {
          return {
            ...clone(study.first),
            schedule_version: study.first.schedule_version + 1,
          };
        }
        const mediaSources: Record<string, string> = {
          asset: "asset://localhost/prompt.wav",
          "remote-http": "http://example.invalid/prompt.wav",
          "remote-https": "https://example.invalid/prompt.wav",
          unsupported: "javascript:alert(1)",
        };
        const mediaSource = mediaSources[params.get("media") ?? ""];
        if (mediaSource) {
          return {
            ...clone(dtos.readyMediaCard),
            prompt_media: [
              {
                ...clone(dtos.media.prompt_audio),
                asset_path: mediaSource,
              },
            ],
          };
        }
        const mediaScenario = params.get("media");
        if (
          mediaScenario === "ready" ||
          mediaScenario === "blocked" ||
          mediaScenario === "playback-error" ||
          mediaScenario === "multiple"
        ) {
          const cardId = (args as { cardId?: string })?.cardId ?? "due-card";
          const ready = clone(dtos.readyMediaCard);
          return {
            ...ready,
            card_id: cardId,
            prompt:
              cardId === "new-card"
                ? `Second card · ${ready.prompt}`
                : ready.prompt,
            prompt_media:
              mediaScenario === "multiple"
                ? [
                    ...ready.prompt_media,
                    {
                      ...ready.prompt_media[0],
                      id: "second-prompt-audio-fixture",
                    },
                  ]
                : ready.prompt_media,
          };
        }
        if (mediaScenario === "missing" || mediaScenario === "corrupt") {
          const missing = clone(dtos.missingMediaCard);
          return {
            ...missing,
            prompt_media: missing.prompt_media.map((media) => ({
              ...media,
              availability: mediaScenario,
            })),
          };
        }
        return clone(
          (args as { cardId?: string })?.cardId === "new-card"
            ? study.second
            : study.first,
        );
      }
      if (command === "check_answer") {
        if (params.get("answer") === "wrong") return clone(dtos.wrongReveal);
        if (params.get("media") === "real-mp3") {
          return {
            ...clone(dtos.readyMediaReveal),
            answer_media: [realMp3Media("answer_audio")],
          };
        }
        if (
          params.get("media") === "ready" ||
          params.get("media") === "blocked" ||
          params.get("media") === "playback-error" ||
          params.get("media") === "multiple"
        )
          return clone(dtos.readyMediaReveal);
        return clone(study.reveal);
      }
      if (command === "grade_review") {
        const request = (
          args as {
            request: {
              review_event_id: string;
              card_id: string;
              chosen_grade: string;
            };
          }
        ).request;
        if (!committedReviewEventIds.has(request.review_event_id)) {
          committedReviewEventIds.add(request.review_event_id);
          const committedReviews = JSON.parse(
            localStorage.getItem("meiki-e2e-committed-reviews") ?? "[]",
          ) as unknown[];
          committedReviews.push({
            review_event_id: request.review_event_id,
            card_id: request.card_id,
            chosen_grade: request.chosen_grade,
            schedule_version: 1,
            due_at: dtos.gradeResult.due_at,
          });
          localStorage.setItem(
            "meiki-e2e-committed-reviews",
            JSON.stringify(committedReviews),
          );
        }
        if (
          params.get("failure") === "queue-switch-mismatch" &&
          calls[command] === 1
        ) {
          return {
            ...clone(dtos.gradeResult),
            review_event_id: "unexpected-review-event",
          };
        }
        return {
          ...clone(dtos.gradeResult),
          review_event_id: request.review_event_id,
        };
      }
      if (command === "undo_review") {
        return {
          ...clone(dtos.undoResult),
          undo_event_id: (args as { request: { undo_event_id: string } })
            .request.undo_event_id,
        };
      }
      if (command === "suspend_card") return clone(dtos.suspendedCard);
      if (command === "get_scheduler_settings") {
        const deckId = (args as { deckId?: string })?.deckId ?? "default-deck";
        const fixture =
          params.get("boundary") === "midnight"
            ? dtos.midnightSchedulerSettings
            : dtos.schedulerSettings;
        return clone(
          schedulerSettingsByDeck[deckId] ?? {
            ...fixture,
            deck_id: deckId,
            ...(params.get("settings") === "legacy-default-override" &&
            deckId === "default-deck"
              ? {
                  deck_daily_time_budget_minutes: 20,
                  effective_daily_time_budget_minutes: 20,
                  budget_source: "deck_override",
                }
              : {}),
          },
        );
      }
      if (command === "preview_scheduler_policy") {
        return clone(
          (
            args as {
              request: { scheduling_mode: "automatic" | "expert" };
            }
          ).request.scheduling_mode === "expert"
            ? dtos.expertSchedulerPreview
            : dtos.schedulerPreview,
        );
      }
      if (command === "update_scheduler_settings") {
        const request = (
          args as {
            request: typeof dtos.schedulerSettings & {
              scheduling_mode: "automatic" | "expert";
            };
          }
        ).request;
        const saved = {
          ...(request.scheduling_mode === "expert"
            ? dtos.savedExpertSettings
            : dtos.savedAutomaticSettings),
          ...request,
          effective_daily_time_budget_minutes:
            request.deck_daily_time_budget_minutes ??
            request.collection_daily_time_budget_minutes,
          budget_source:
            request.deck_daily_time_budget_minutes === null
              ? "collection_budget"
              : "deck_override",
        } as typeof dtos.schedulerSettings;
        schedulerSettingsByDeck[request.deck_id] = saved;
        return clone(saved);
      }
      if (command === "import_scheduler_parameters")
        return clone(dtos.schedulerSettings);
      if (command === "export_scheduler_parameters")
        return { path: "/tmp/collection.scheduler-parameters.json" };

      if (command === "list_decks") {
        if (params.get("decks") === "lifecycle") {
          const index = Math.min(
            calls[command] - 1,
            dtos.deckLifecycle.length - 1,
          );
          return clone(dtos.deckLifecycle[index]);
        }
        return clone(dtos.decks);
      }
      if (command === "list_deck_summaries") {
        if (params.get("decks") === "loading") {
          await new Promise((resolve) => setTimeout(resolve, 350));
        }
        if (params.get("decks") === "error") {
          throw new Error("The local collection is temporarily unavailable.");
        }
        if (params.get("decks") === "empty") return [];
        if (params.get("decks") === "long-name") {
          return clone(
            dtos.deckSummaries.map((deck) =>
              deck.id === "travel-deck"
                ? {
                    ...deck,
                    name: "Travel phrases for an exceptionally long multilingual journey through 日本語 and العربية",
                  }
                : deck,
            ),
          );
        }
        if (params.get("decks") === "batch") {
          return clone(
            dtos.batchDeckSummaries.filter(
              (deck) => !deletedDeckIds.has(deck.id),
            ),
          );
        }
        if (
          (bundleImported || params.get("bundleRemoval") === "installed") &&
          !bundleRemoved
        ) {
          const summaries = [
            ...dtos.deckSummaries,
            ...dtos.bundleDeckSummaries,
          ].map((deck) =>
            params.get("emptyDeck") === deck.id
              ? { ...deck, total_cards: 0, due_cards: 0, new_cards: 0 }
              : deck,
          );
          return clone(
            summaries.filter((deck) => !deletedDeckIds.has(deck.id)),
          );
        }
        if (params.get("deckDeletion") === "focused-session") {
          return clone(
            focusedSessionDeckDeleted
              ? dtos.deckSummaries.filter((deck) => deck.id === "default-deck")
              : dtos.deckSummaries.filter(
                  (deck) => !deletedDeckIds.has(deck.id),
                ),
          );
        }
        if (params.get("deckDeletion") === "only-deck") {
          return clone(
            deckDeletedToUnsorted
              ? [
                  {
                    ...dtos.deckSummaries[0],
                    total_cards: deletedDeckCardRestored ? 1 : 0,
                    due_cards: 0,
                    new_cards: deletedDeckCardRestored ? 1 : 0,
                  },
                ]
              : dtos.deckSummaries.filter((deck) => deck.id === "travel-deck"),
          );
        }
        if (params.get("decks") === "lifecycle") {
          const index = Math.min(
            calls[command] - 1,
            dtos.deckSummaryLifecycle.length - 1,
          );
          return clone(
            dtos.deckSummaryLifecycle[index].filter(
              (deck) => !deletedDeckIds.has(deck.id),
            ),
          );
        }
        if (params.get("decks") === "empty-default") {
          return clone(
            dtos.deckSummaries.filter(
              (deck) =>
                deck.id !== "default-deck" && !deletedDeckIds.has(deck.id),
            ),
          );
        }
        return clone(
          dtos.deckSummaries.filter((deck) => !deletedDeckIds.has(deck.id)),
        );
      }
      if (command === "create_deck") return clone(dtos.createdDeck);
      if (command === "rename_deck") return clone(dtos.renamedDeck);
      if (command === "delete_decks") {
        const deckIds = (args as { request: { deck_ids: string[] } }).request
          .deck_ids;
        const affectedCards = dtos.batchDeckSummaries
          .filter((deck) => deckIds.includes(deck.id))
          .reduce((total, deck) => total + deck.total_cards, 0);
        if (params.get("batchDeletion") === "precommit-failure") {
          window.__MEIKI_TEST_DECKS_DELETION_PROGRESS__?.({
            phase: "removing_cards",
            current: 0,
            total: affectedCards,
          });
          throw new Error("storage operation failed: raw fixture id");
        }
        if (params.get("batchDeletion") === "progress") {
          const report = async (
            phase:
              "preparing" | "removing_cards" | "cleaning_audio" | "finalizing",
            current: number | null,
            total: number | null,
          ) => {
            window.__MEIKI_TEST_DECKS_DELETION_PROGRESS__?.({
              phase,
              current,
              total,
            });
            await new Promise((resolve) => setTimeout(resolve, 120));
          };
          await report("preparing", null, null);
          await report("removing_cards", 0, affectedCards);
          await report("removing_cards", affectedCards, affectedCards);
          await report("cleaning_audio", 0, 300);
          await report("cleaning_audio", 300, 300);
          await report("finalizing", null, null);
        }
        deckIds.forEach((deckId) => deletedDeckIds.add(deckId));
        if (params.get("batchDeletion") === "postcommit-failure") {
          return {
            deleted_deck_ids: clone(deckIds),
            affected_cards: affectedCards,
            media_cleanup_warning:
              "Decks deleted, but some unused audio could not be cleaned up.",
          };
        }
        return {
          deleted_deck_ids: clone(deckIds),
          affected_cards: affectedCards,
          media_cleanup_warning: null,
        };
      }
      if (command === "delete_deck") {
        const deletedDeckId = (args as { request: { deck_id: string } }).request
          .deck_id;
        if (
          params.get("deckDeletion") === "progress" ||
          params.get("deckDeletion") === "progress-visual"
        ) {
          const report = async (
            phase:
              "preparing" | "removing_cards" | "cleaning_audio" | "finalizing",
            current: number | null,
            total: number | null,
          ) => {
            window.__MEIKI_TEST_DECK_DELETION_PROGRESS__?.({
              phase,
              current,
              total,
            });
            await new Promise((resolve) => setTimeout(resolve, 120));
          };
          await report("preparing", null, null);
          await report("removing_cards", 0, 3_000);
          await report("removing_cards", 3_000, 3_000);
          await report("cleaning_audio", 0, 2_999);
          if (params.get("deckDeletion") === "progress-visual") {
            await report("cleaning_audio", 1_240, 2_999);
            while (
              localStorage.getItem("meiki-e2e-finish-deck-deletion") !== "true"
            ) {
              await new Promise((resolve) => setTimeout(resolve, 20));
            }
          }
          await report("cleaning_audio", 2_999, 2_999);
          await report("finalizing", null, null);
          deletedDeckIds.add(deletedDeckId);
          return { ...clone(dtos.movedDeck), deleted_deck_id: deletedDeckId };
        }
        if (params.get("deckDeletion") === "precommit-failure") {
          window.__MEIKI_TEST_DECK_DELETION_PROGRESS__?.({
            phase: "removing_cards",
            current: 0,
            total: 2,
          });
          throw new Error("storage operation failed: raw fixture id");
        }
        if (params.get("deckDeletion") === "postcommit-failure") {
          deletedDeckIds.add(deletedDeckId);
          return {
            ...clone(dtos.movedDeck),
            deleted_deck_id: deletedDeckId,
            media_cleanup_warning:
              "Deck deleted, but some unused audio could not be cleaned up.",
          };
        }
        if (params.get("deckDeletion") === "focused-session") {
          focusedSessionDeckDeleted = true;
          deletedDeckIds.add(deletedDeckId);
          return { ...clone(dtos.movedDeck), deleted_deck_id: deletedDeckId };
        }
        if (params.get("deckDeletion") === "only-deck") {
          deckDeletedToUnsorted = true;
          deletedDeckIds.add(deletedDeckId);
          return {
            deleted_deck_id: "travel-deck",
            affected_cards: 1,
            media_cleanup_warning: null,
          };
        }
        deletedDeckIds.add(deletedDeckId);
        return {
          ...clone(
            deletedDeckId === "travel-deck" ? dtos.movedDeck : dtos.deletedDeck,
          ),
          deleted_deck_id: deletedDeckId,
        };
      }

      if (command === "get_deck_cards") {
        const request = (
          args as {
            request: {
              deck_id: string;
              query: string;
              trash: string;
              offset: number;
              limit: number;
            };
          }
        ).request;
        let fixture =
          request.trash === "trash"
            ? dtos.deckCards.trash
            : params.get("deckCards") === "last-page"
              ? dtos.deckCards.pagination
              : request.deck_id === "default-deck"
                ? dtos.deckCards.default
                : dtos.deckCards.travel;
        if (
          params.get("deckDeletion") === "only-deck" &&
          request.deck_id === "default-deck"
        ) {
          fixture = {
            ...dtos.deckCards.default,
            cards:
              request.trash === "trash"
                ? deckDeletedToUnsorted && !deletedDeckCardRestored
                  ? dtos.deckCards.trash.cards
                  : []
                : deletedDeckCardRestored
                  ? dtos.deckCards.travel.cards.slice(0, 1)
                  : [],
            total_matches: 0,
          };
        }
        const query = request.query.trim().toLocaleLowerCase();
        const matches = fixture.cards.filter(
          (card) =>
            !removedDeckCardIds.has(card.id) &&
            (!query ||
              `${card.sentence} ${card.answer}`
                .toLocaleLowerCase()
                .includes(query)),
        );
        return {
          ...clone(fixture),
          cards: clone(
            matches.slice(request.offset, request.offset + request.limit),
          ),
          total_matches: matches.length,
          offset: request.offset,
          limit: request.limit,
        };
      }
      if (command === "apply_deck_card_action") {
        const request = (
          args as {
            request: { deck_id: string; card_ids: string[]; action: string };
          }
        ).request;
        if (
          params.get("deckDeletion") === "only-deck" &&
          request.deck_id === "default-deck" &&
          request.action === "restore"
        ) {
          deletedDeckCardRestored = true;
        }
        if (
          params.get("deckCards") === "last-page" &&
          (request.action === "move" || request.action === "trash")
        ) {
          request.card_ids.forEach((cardId) => removedDeckCardIds.add(cardId));
        }
        return { affected_cards: request.card_ids.length };
      }

      if (command === "new_authoring_draft") return clone(dtos.emptyDraft);
      if (command === "get_authoring_draft_for_card")
        return clone(dtos.persistedDraft);
      if (command === "make_cloze") {
        if (authoringName === "boundary-error") {
          throw new Error(
            "The text selection splits an extended grapheme cluster.",
          );
        }
        return clone(authoring);
      }
      if (command === "remove_cloze") return clone(dtos.authoring.removed);
      if (command === "reorder_segments") return clone(authoring);
      if (command === "preview_authoring_draft") {
        return clone(
          dtos.authoringPreviews[
            authoringName as keyof typeof dtos.authoringPreviews
          ] ?? dtos.authoringPreviews.cjk,
        );
      }
      if (command === "save_authoring_draft") {
        if (authoringName === "save-error") {
          throw new Error(
            "Explanations support limited Markdown but not raw HTML or executable links.",
          );
        }
        return { ...clone(authoring), persisted: true };
      }
      if (command === "import_media") {
        const role = (args as { request: { role: string } }).request.role;
        return clone(dtos.media[role as keyof typeof dtos.media]);
      }

      if (command === "preview_bundle") {
        const installedDecks =
          params.get("bundle") === "installed" ||
          params.get("bundle") === "unassociated"
            ? dtos.bundlePreview.decks.length
            : params.get("bundle") === "partial"
              ? 2
              : 0;
        return clone({
          ...dtos.bundlePreview,
          language_tag:
            params.get("bundleLanguage") === "long"
              ? longBundleLanguage
              : dtos.bundlePreview.language_tag,
          decks: dtos.bundlePreview.decks.map((deck, index) => ({
            ...deck,
            status: index < installedDecks ? "installed" : "will_add",
          })),
          can_import:
            params.get("bundle") === "unassociated" ||
            installedDecks < dtos.bundlePreview.decks.length,
        });
      }
      if (command === "import_bundle") {
        const bundleImportMode = params.get("bundleImport");
        const report = async (
          stage: "preparing_decks" | "adding_cards" | "adding_audio",
          current: number,
          total: number,
        ) => {
          window.__MEIKI_TEST_BUNDLE_PROGRESS__?.({ stage, current, total });
          await new Promise((resolve) => setTimeout(resolve, 100));
        };
        await report("preparing_decks", 0, 6);
        if (params.get("bundleProgress") === "preparing") {
          while (!localStorage.getItem("meiki-e2e-finish-bundle-import")) {
            await new Promise((resolve) => setTimeout(resolve, 20));
          }
        } else if (bundleImportMode === "activity") {
          await report("adding_cards", 1_240, 9_700);
          await report("adding_cards", 620, 9_700);
          localStorage.setItem("meiki-e2e-bundle-regression-sent", "true");
          let outcome = localStorage.getItem("meiki-e2e-finish-bundle-import");
          while (!outcome) {
            await new Promise((resolve) => setTimeout(resolve, 20));
            outcome = localStorage.getItem("meiki-e2e-finish-bundle-import");
          }
          if (outcome === "failure") {
            throw new Error("The bundle archive could not be verified.");
          }
          await report("adding_cards", 9_700, 9_700);
          await report("adding_audio", 9_700, 9_700);
        } else {
          await report("preparing_decks", 6, 6);
          await report("adding_cards", 0, 9_700);
          await report("adding_cards", 9_700, 9_700);
          await report("adding_audio", 0, 9_700);
          await report("adding_audio", 9_700, 9_700);
        }
        bundleImported = true;
        if (params.get("bundle") === "unassociated") {
          return {
            language_tag: "ja-JP",
            added_decks: 0,
            added_cards: 0,
            imported_media_objects: 0,
            deduplicated_media_objects: 0,
          };
        }
        return {
          language_tag: "ja-JP",
          added_decks: 6,
          added_cards: 9_700,
          imported_media_objects: 9_700,
          deduplicated_media_objects: 0,
        };
      }
      if (command === "list_installed_bundles") {
        if (params.get("decks") === "batch") {
          return [{ language_tag: "ja-JP", decks: 2, cards: 1_300 }];
        }
        if (
          (bundleImported || params.get("bundleRemoval") === "installed") &&
          !bundleRemoved
        ) {
          return [{ language_tag: "ja-JP", decks: 6, cards: 9_700 }];
        }
        return [];
      }
      if (command === "export_bundle") {
        return {
          path: "/tmp/exports/meiki-bundle-e2e.meiki",
          decks: 6,
          notes: 9_700,
          cards: 9_700,
          review_events: 0,
          media_objects: 9_700,
        };
      }
      if (command === "remove_bundle") {
        const mediaCleanupFailure =
          params.get("bundleDeletion") === "postcommit-failure";
        const cardTotals = [300, 1_100, 2_100, 3_700, 6_700, 9_700];
        for (const [index, processedCards] of cardTotals.entries()) {
          window.__MEIKI_TEST_BUNDLE_REMOVAL_PROGRESS__?.({
            removed_decks: index + 1,
            total_decks: 6,
            processed_cards: processedCards,
            total_cards: 9_700,
          });
          if (!mediaCleanupFailure) {
            await new Promise((resolve) => setTimeout(resolve, 100));
          }
        }
        bundleRemoved = true;
        localStorage.setItem("meiki-e2e-bundle-removed", "true");
        return {
          language_tag: "ja-JP",
          removed_decks: 6,
          affected_cards: 9_700,
          media_cleanup_warning: mediaCleanupFailure
            ? "Bundle removed, but some unused audio could not be cleaned up."
            : null,
        };
      }
      throw new Error(`No DTO fixture for command: ${command}`);
    };
  }, scenarioDtos);
}
