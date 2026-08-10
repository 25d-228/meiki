import {
  convertFileSrc,
  invoke as tauriInvoke,
  isTauri,
} from "@tauri-apps/api/core";
import type { StudyMediaDto } from "$lib/generated/StudyMediaDto";

const SCHEME = /^([a-z][a-z\d+.-]*):/i;
const WINDOWS_ABSOLUTE_PATH = /^[a-z]:[\\/]/i;
const SHA256_CONTENT_HASH = /^sha256:([a-f\d]{64})$/;
const promptAudioAutoplayKey = "meiki-autoplay-prompt-audio";
const audioSeekTimeoutMs = 250;

type MediaSource = Pick<
  StudyMediaDto,
  "asset_path" | "content_hash" | "media_type"
>;

function sourceScheme(path: string): string | undefined {
  if (WINDOWS_ABSOLUTE_PATH.test(path)) return undefined;
  return SCHEME.exec(path)?.[1]?.toLowerCase();
}

export function usesManagedAudioTransport(
  contentHash: string | undefined,
  mediaType: string | undefined,
): boolean {
  return (
    mediaType === "audio/mpeg" && SHA256_CONTENT_HASH.test(contentHash ?? "")
  );
}

export async function managedAudioBlobSource(
  contentHash: string,
  mediaType: string,
): Promise<string> {
  const invoke = window.__MEIKI_TEST_INVOKE__ ?? tauriInvoke;
  const response = await invoke<unknown>("read_managed_audio", {
    contentHash,
  });
  let bytes: ArrayBuffer;
  if (response instanceof ArrayBuffer) {
    bytes = response;
  } else if (
    Array.isArray(response) &&
    response.every(
      (value) => Number.isInteger(value) && value >= 0 && value <= 0xff,
    )
  ) {
    bytes = Uint8Array.from(response).buffer;
  } else {
    throw new Error("Audio transport failed.");
  }
  if (bytes.byteLength === 0) throw new Error("Audio transport failed.");
  return URL.createObjectURL(new Blob([bytes], { type: mediaType }));
}

export function mediaAssetSource(media: MediaSource): string | undefined {
  const path = media.asset_path;
  if (!path) return undefined;
  if (path.startsWith("//") || path.startsWith("\\\\")) return undefined;

  const scheme = sourceScheme(path);
  if (scheme === "asset") return path;

  // Playwright DTO fixtures use bounded inline media in the browser dev server.
  // Packaged Tauri builds accept only managed paths and the asset protocol.
  if (scheme === "data" && !isTauri()) return path;
  if (scheme) return undefined;

  if (usesManagedAudioTransport(media.content_hash, media.media_type)) return;
  if (!isTauri()) return path;
  return convertFileSrc(path);
}

export async function restartAudio(audio: HTMLAudioElement): Promise<void> {
  if (audio.currentTime !== 0) {
    await new Promise<void>((resolve) => {
      let finished = false;
      function finishSeek(): void {
        if (finished) return;
        finished = true;
        clearTimeout(timeout);
        audio.removeEventListener("seeked", finishSeek);
        resolve();
      }
      // Some media engines omit seeked at a boundary; Replay must not hang.
      const timeout = setTimeout(finishSeek, audioSeekTimeoutMs);
      audio.addEventListener("seeked", finishSeek, { once: true });
      audio.currentTime = 0;
      if (
        !audio.seeking ||
        audio.readyState === HTMLMediaElement.HAVE_NOTHING
      ) {
        finishSeek();
      }
    });
  }
  await audio.play();
}

export function readPromptAudioAutoplay(): boolean {
  return localStorage.getItem(promptAudioAutoplayKey) !== "false";
}

export function writePromptAudioAutoplay(enabled: boolean): void {
  localStorage.setItem(promptAudioAutoplayKey, String(enabled));
}
