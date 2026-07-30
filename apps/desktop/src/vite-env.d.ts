/// <reference types="vite/client" />

import type { invoke } from "@tauri-apps/api/core";
import type { MediaRoleDto } from "./lib/generated/MediaRoleDto";

declare global {
  interface Window {
    __MEIKI_TEST_INVOKE__?: typeof invoke;
    __MEIKI_TEST_PICK_FILE__?: (role: MediaRoleDto) => Promise<string | null>;
    __MEIKI_TEST_PICK_ARCHIVE__?: () => Promise<string | null>;
    __MEIKI_TEST_PICK_SCHEDULER_PARAMETERS__?: () => Promise<string | null>;
  }
}

export {};
