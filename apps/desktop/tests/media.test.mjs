import assert from "node:assert/strict";
import { afterEach, test } from "node:test";

import {
  managedAudioBlobSource,
  mediaAssetSource,
  usesManagedAudioTransport,
} from "../src/lib/media.ts";

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
  Reflect.deleteProperty(globalThis, "isTauri");
});

function useBrowserRuntime() {
  globalThis.window = {};
}

function useTauriRuntime() {
  globalThis.isTauri = true;
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

test("reserves extensionless managed MP3 objects for native byte transport", () => {
  useBrowserRuntime();
  assert.equal(
    mediaAssetSource(
      media(
        "/app-data/collection.media/objects/sha256/3d/65d920040aab9d14d2da0b132b8f03c96a35f0a0946cb4464e0178dda12793",
      ),
    ),
    undefined,
  );
  assert.equal(usesManagedAudioTransport(contentHash, "audio/mpeg"), true);
});

test("uses the official Tauri runtime check for other local media paths", () => {
  useTauriRuntime();
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

test("creates typed Blob URLs from native array and buffer responses", async () => {
  useTauriRuntime();
  const expected = Uint8Array.from([0x49, 0x44, 0x33, 0x04]);

  for (const nativeResponse of [Array.from(expected), expected.buffer]) {
    globalThis.window.__MEIKI_TEST_INVOKE__ = async (command, args) => {
      assert.equal(command, "read_managed_audio");
      assert.deepEqual(args, { contentHash });
      return nativeResponse;
    };

    const source = await managedAudioBlobSource(contentHash, "audio/mpeg");
    const response = await globalThis.fetch(source);
    assert.equal(response.headers.get("content-type"), "audio/mpeg");
    assert.deepEqual(new Uint8Array(await response.arrayBuffer()), expected);
    globalThis.URL.revokeObjectURL(source);
  }
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
