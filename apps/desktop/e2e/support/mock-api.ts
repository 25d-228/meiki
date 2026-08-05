import type { Page } from "@playwright/test";

import { scenarioDtos } from "./scenario-dtos";

export async function installMockApi(page: Page): Promise<void> {
  await page.addInitScript((dtos) => {
    const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
    const calls: Record<string, number> = {};
    const removedDeckCardIds = new Set<string>();
    let deckDeletedToUnsorted = false;
    let deletedDeckCardRestored = false;
    let focusedSessionDeckDeleted = false;
    let bundleImported = false;
    let bundleRemoved = false;
    const schedulerSettingsByDeck: Record<
      string,
      typeof dtos.schedulerSettings
    > = {};

    window.__MEIKI_TEST_REQUESTS__ = [];
    window.__MEIKI_TEST_PICK_FILE__ = async (role) => `/fixture/${role}`;
    window.__MEIKI_TEST_PICK_ARCHIVE__ = async () =>
      "/tmp/exports/meiki-e2e.meiki";
    window.__MEIKI_TEST_PICK_SCHEDULER_PARAMETERS__ = async () =>
      "/tmp/meiki-scheduler-parameters.json";
    HTMLMediaElement.prototype.play = async function () {
      localStorage.setItem(
        "meiki-e2e-media-play-count",
        String(
          Number(localStorage.getItem("meiki-e2e-media-play-count") ?? "0") + 1,
        ),
      );
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
      if (
        params.get("failure") === "grade" &&
        command === "grade_review" &&
        calls[command] === 1
      ) {
        throw new Error("The review commit was interrupted.");
      }

      if (command === "get_today_overview") return clone(overview);
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
        return clone(
          params.get("reconcile") === "second"
            ? dtos.reconciledSecondCard
            : dtos.reconciledQueue,
        );
      }
      if (command === "get_study_card") {
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
        if (params.get("media") === "ready") return clone(dtos.readyMediaCard);
        if (params.get("media") === "missing")
          return clone(dtos.missingMediaCard);
        return clone(
          (args as { cardId?: string })?.cardId === "new-card"
            ? study.second
            : study.first,
        );
      }
      if (command === "check_answer") {
        if (params.get("answer") === "wrong") return clone(dtos.wrongReveal);
        if (params.get("media") === "ready")
          return clone(dtos.readyMediaReveal);
        return clone(study.reveal);
      }
      if (command === "grade_review") {
        return {
          ...clone(dtos.gradeResult),
          review_event_id: (args as { request: { review_event_id: string } })
            .request.review_event_id,
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
        if (
          (bundleImported || params.get("bundleRemoval") === "installed") &&
          !bundleRemoved
        ) {
          return clone([...dtos.deckSummaries, ...dtos.bundleDeckSummaries]);
        }
        if (params.get("deckDeletion") === "focused-session") {
          return clone(
            focusedSessionDeckDeleted
              ? dtos.deckSummaries.filter((deck) => deck.id === "default-deck")
              : dtos.deckSummaries,
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
          return clone(dtos.deckSummaryLifecycle[index]);
        }
        if (params.get("decks") === "empty-default") {
          return clone(
            dtos.deckSummaries.filter((deck) => deck.id !== "default-deck"),
          );
        }
        return clone(dtos.deckSummaries);
      }
      if (command === "create_deck") return clone(dtos.createdDeck);
      if (command === "rename_deck") return clone(dtos.renamedDeck);
      if (command === "delete_deck") {
        if (params.get("deckDeletion") === "focused-session") {
          focusedSessionDeckDeleted = true;
          return clone(dtos.movedDeck);
        }
        if (params.get("deckDeletion") === "only-deck") {
          deckDeletedToUnsorted = true;
          return { deleted_deck_id: "travel-deck", affected_cards: 1 };
        }
        return clone(
          (args as { request: { deck_id: string } }).request.deck_id ===
            "travel-deck"
            ? dtos.movedDeck
            : dtos.deletedDeck,
        );
      }

      if (command === "get_library") {
        return clone(
          params.get("collection") === "empty"
            ? dtos.emptyLibrary
            : dtos.library,
        );
      }
      if (command === "apply_library_bulk_action") {
        const action = (args as { request: { action: string } }).request.action;
        return clone(
          dtos.bulkResults[action as keyof typeof dtos.bulkResults] ??
            dtos.bulkResults.suspend,
        );
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

      if (command === "export_archive") return clone(dtos.archiveExport);
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
          decks: dtos.bundlePreview.decks.map((deck, index) => ({
            ...deck,
            status: index < installedDecks ? "installed" : "missing",
          })),
          can_import:
            params.get("bundle") === "unassociated" ||
            installedDecks < dtos.bundlePreview.decks.length,
        });
      }
      if (command === "import_bundle") {
        const report = async (
          stage: "preparing_decks" | "adding_cards" | "adding_audio",
          current: number,
          total: number,
        ) => {
          window.__MEIKI_TEST_BUNDLE_PROGRESS__?.({ stage, current, total });
          await new Promise((resolve) => setTimeout(resolve, 100));
        };
        await report("preparing_decks", 0, 6);
        await report("preparing_decks", 6, 6);
        await report("adding_cards", 0, 9_700);
        await report("adding_cards", 9_700, 9_700);
        await report("adding_audio", 0, 9_700);
        await report("adding_audio", 9_700, 9_700);
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
        if (
          (bundleImported || params.get("bundleRemoval") === "installed") &&
          !bundleRemoved
        ) {
          return [{ language_tag: "ja-JP", decks: 6, cards: 9_700 }];
        }
        return [];
      }
      if (command === "remove_bundle") {
        const cardTotals = [300, 1_100, 2_100, 3_700, 6_700, 9_700];
        for (const [index, movedCards] of cardTotals.entries()) {
          window.__MEIKI_TEST_BUNDLE_REMOVAL_PROGRESS__?.({
            removed_decks: index + 1,
            total_decks: 6,
            moved_cards: movedCards,
            total_cards: 9_700,
          });
          await new Promise((resolve) => setTimeout(resolve, 100));
        }
        bundleRemoved = true;
        return {
          language_tag: "ja-JP",
          removed_decks: 6,
          moved_cards: 9_700,
        };
      }
      if (command === "preview_archive") return clone(dtos.archivePreview);
      if (command === "add_archive_deck") return clone(dtos.archiveAddDeck);
      if (command === "import_archive") return clone(dtos.archiveImport);
      if (command === "list_backups") return [];
      if (command === "restore_backup") {
        return {
          path: "/tmp/backups/collection.db.pre-restore.bak",
          file_name: "collection.db.pre-restore.bak",
          byte_size: 4096,
        };
      }

      throw new Error(`No DTO fixture for command: ${command}`);
    };
  }, scenarioDtos);
}
