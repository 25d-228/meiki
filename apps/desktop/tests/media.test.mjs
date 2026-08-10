import assert from "node:assert/strict";
import { afterEach, test } from "node:test";

import { mediaAssetSource } from "../src/lib/media.ts";

const contentHash = `sha256:${"d".repeat(64)}`;

function media(assetPath, mediaType = "audio/mpeg") {
  return {
    asset_path: assetPath,
    content_hash: contentHash,
    media_type: mediaType,
  };
}

afterEach(() => {
  Reflect.deleteProperty(globalThis, "window");
});

function useBrowserRuntime() {
  globalThis.window = {};
}

function useTauriRuntime() {
  globalThis.window = {
    __TAURI_INTERNALS__: {
      convertFileSrc(path, protocol) {
        return `${protocol}://localhost/${encodeURIComponent(path)}`;
      },
    },
  };
}

test("returns no source for missing, remote, and unsupported values", () => {
  useBrowserRuntime();

  for (const source of [
    null,
    "",
    "http://example.invalid/prompt.wav",
    "HTTPS://example.invalid/reveal.png",
    "//example.invalid/prompt.wav",
    "\\\\example.invalid\\prompt.wav",
    "blob:fixture",
    "file:///tmp/prompt.wav",
    "javascript:alert(1)",
  ]) {
    assert.equal(mediaAssetSource(media(source)), undefined);
  }
});

test("accepts the asset protocol and bounded browser data fixtures", () => {
  useBrowserRuntime();

  assert.equal(
    mediaAssetSource(media("asset://localhost/prompt.wav")),
    "asset://localhost/prompt.wav",
  );
  assert.equal(
    mediaAssetSource(media("data:audio/wav;base64,AAAA", "audio/wav")),
    "data:audio/wav;base64,AAAA",
  );
});

test("uses the managed protocol for MP3 objects only in Tauri", () => {
  useTauriRuntime();

  assert.equal(
    mediaAssetSource(
      media(
        "/app-data/collection.media/objects/sha256/3d/65d920040aab9d14d2da0b132b8f03c96a35f0a0946cb4464e0178dda12793",
      ),
    ),
    `meiki-media://localhost/${"d".repeat(64)}`,
  );
  assert.equal(
    mediaAssetSource(
      media("C:\\AppData\\Meiki\\media\\prompt.wav", "audio/wav"),
    ),
    "asset://localhost/C%3A%5CAppData%5CMeiki%5Cmedia%5Cprompt.wav",
  );
  assert.equal(
    mediaAssetSource(media("data:audio/wav;base64,AAAA", "audio/wav")),
    undefined,
  );
});

test("keeps relative fixture paths usable outside Tauri", () => {
  useBrowserRuntime();
  assert.equal(
    mediaAssetSource(media("/fixtures/prompt.wav", "audio/wav")),
    "/fixtures/prompt.wav",
  );
  assert.equal(
    mediaAssetSource(media("media/prompt.wav", "audio/wav")),
    "media/prompt.wav",
  );
});
