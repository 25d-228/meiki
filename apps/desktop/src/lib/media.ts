import { convertFileSrc } from "@tauri-apps/api/core";

export function mediaAssetSource(path: string | null): string | undefined {
  if (!path) return undefined;
  if (
    path.startsWith("asset:") ||
    path.startsWith("http:") ||
    path.startsWith("https:") ||
    path.startsWith("blob:") ||
    path.startsWith("data:")
  ) {
    return path;
  }
  return "__TAURI_INTERNALS__" in window ? convertFileSrc(path) : path;
}
