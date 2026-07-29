/// <reference types="vite/client" />

import type { invoke } from "@tauri-apps/api/core";

declare global {
  interface Window {
    __MEIKI_TEST_INVOKE__?: typeof invoke;
  }
}

export {};
