import { convertFileSrc } from "@tauri-apps/api/core";

const SCHEME = /^([a-z][a-z\d+.-]*):/i;
const WINDOWS_ABSOLUTE_PATH = /^[a-z]:[\\/]/i;

function sourceScheme(path: string): string | undefined {
  if (WINDOWS_ABSOLUTE_PATH.test(path)) return undefined;
  return SCHEME.exec(path)?.[1]?.toLowerCase();
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function mediaAssetSource(path: string | null): string | undefined {
  if (!path) return undefined;
  if (path.startsWith("//") || path.startsWith("\\\\")) return undefined;

  const scheme = sourceScheme(path);
  if (scheme === "asset") return path;

  // Playwright DTO fixtures use bounded inline media in the browser dev server.
  // Packaged Tauri builds accept only managed paths and the asset protocol.
  if (scheme === "data" && !isTauriRuntime()) return path;
  if (scheme) return undefined;

  return isTauriRuntime() ? convertFileSrc(path) : path;
}
