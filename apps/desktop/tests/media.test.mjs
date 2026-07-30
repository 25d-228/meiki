import assert from "node:assert/strict";
import { afterEach, test } from "node:test";

import { mediaAssetSource } from "../src/lib/media.ts";

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
    assert.equal(mediaAssetSource(source), undefined);
  }
});

test("accepts the asset protocol and bounded browser data fixtures", () => {
  useBrowserRuntime();

  assert.equal(
    mediaAssetSource("asset://localhost/prompt.wav"),
    "asset://localhost/prompt.wav",
  );
  assert.equal(
    mediaAssetSource("data:audio/wav;base64,AAAA"),
    "data:audio/wav;base64,AAAA",
  );
});

test("converts managed POSIX and Windows paths only in Tauri", () => {
  useTauriRuntime();

  assert.equal(
    mediaAssetSource("/app-data/media/prompt.wav"),
    "asset://localhost/%2Fapp-data%2Fmedia%2Fprompt.wav",
  );
  assert.equal(
    mediaAssetSource("C:\\AppData\\Meiki\\media\\prompt.wav"),
    "asset://localhost/C%3A%5CAppData%5CMeiki%5Cmedia%5Cprompt.wav",
  );
  assert.equal(mediaAssetSource("data:audio/wav;base64,AAAA"), undefined);
});

test("keeps relative fixture paths usable outside Tauri", () => {
  useBrowserRuntime();
  assert.equal(
    mediaAssetSource("/fixtures/prompt.wav"),
    "/fixtures/prompt.wav",
  );
  assert.equal(mediaAssetSource("media/prompt.wav"), "media/prompt.wav");
});
