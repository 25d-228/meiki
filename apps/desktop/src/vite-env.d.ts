/// <reference types="vite/client" />

import type { invoke } from "@tauri-apps/api/core";
import type { MediaRoleDto } from "./lib/generated/MediaRoleDto";
import type { BundleImportProgressDto } from "./lib/generated/BundleImportProgressDto";
import type { BundleRemovalProgressDto } from "./lib/generated/BundleRemovalProgressDto";

declare global {
  interface Window {
    __MEIKI_TEST_INVOKE__?: typeof invoke;
    __MEIKI_TEST_PICK_FILE__?: (role: MediaRoleDto) => Promise<string | null>;
    __MEIKI_TEST_PICK_ARCHIVE__?: () => Promise<string | null>;
    __MEIKI_TEST_BUNDLE_PROGRESS__?: (
      progress: BundleImportProgressDto,
    ) => void;
    __MEIKI_TEST_BUNDLE_REMOVAL_PROGRESS__?: (
      progress: BundleRemovalProgressDto,
    ) => void;
    __MEIKI_TEST_PICK_SCHEDULER_PARAMETERS__?: () => Promise<string | null>;
    __MEIKI_TEST_REQUESTS__?: Array<{
      command: string;
      args: Record<string, unknown>;
    }>;
  }
}

export {};
