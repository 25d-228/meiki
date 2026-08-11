const frontAnswerPreferenceKey = "meiki-study-front-answer";
const visualKeyboardPreferenceKey = "meiki-study-visual-keyboard";

export function readStudyFrontAnswerPreference(): boolean {
  return localStorage.getItem(frontAnswerPreferenceKey) === "true";
}

export function writeStudyFrontAnswerPreference(enabled: boolean): void {
  localStorage.setItem(frontAnswerPreferenceKey, String(enabled));
}

export function readStudyVisualKeyboardPreference(): boolean {
  return localStorage.getItem(visualKeyboardPreferenceKey) === "true";
}

export function writeStudyVisualKeyboardPreference(enabled: boolean): void {
  localStorage.setItem(visualKeyboardPreferenceKey, String(enabled));
}
