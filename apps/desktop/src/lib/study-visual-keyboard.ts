const preferenceKey = "meiki-study-visual-keyboard";

export function readStudyVisualKeyboardPreference(): boolean {
  return localStorage.getItem(preferenceKey) === "true";
}

export function writeStudyVisualKeyboardPreference(enabled: boolean): void {
  localStorage.setItem(preferenceKey, String(enabled));
}
