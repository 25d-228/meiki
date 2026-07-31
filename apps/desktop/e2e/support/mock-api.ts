import type { Page } from "@playwright/test";

import { scenarioDtos } from "./scenario-dtos";

export async function installMockApi(page: Page): Promise<void> {
  await page.addInitScript((dtos) => {
    const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
    const calls: Record<string, number> = {};

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
        return clone(
          params.get("boundary") === "midnight"
            ? dtos.midnightSchedulerSettings
            : dtos.schedulerSettings,
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
        return clone(
          (
            args as {
              request: { scheduling_mode: "automatic" | "expert" };
            }
          ).request.scheduling_mode === "expert"
            ? dtos.savedExpertSettings
            : dtos.savedAutomaticSettings,
        );
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
      if (command === "create_deck") return clone(dtos.createdDeck);
      if (command === "rename_deck") return clone(dtos.renamedDeck);
      if (command === "delete_deck") {
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
