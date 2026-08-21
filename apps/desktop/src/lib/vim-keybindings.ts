const preferenceKey = "meiki-vim-keybindings";

const editableControlSelector = [
  "input",
  "textarea",
  "select",
  '[contenteditable]:not([contenteditable="false"])',
  '[role="textbox"]',
  '[role="searchbox"]',
  '[role="combobox"]',
  '[role="spinbutton"]',
].join(",");

const overlaySelector = [
  '[data-slot="dialog-content"][data-state="open"]',
  '[data-slot="alert-dialog-content"][data-state="open"]',
  '[data-slot="sheet-content"][data-state="open"]',
  '[data-slot="select-content"][data-state="open"]',
  '[role="menu"][data-state="open"]',
  '[role="listbox"][data-state="open"]',
].join(",");

export type VimMode = "normal" | "insert";

export function readVimKeybindings(): boolean {
  return localStorage.getItem(preferenceKey) === "true";
}

export function writeVimKeybindings(enabled: boolean): void {
  localStorage.setItem(preferenceKey, String(enabled));
}

export function vimCommandAllowed(
  event: KeyboardEvent,
  enabled: boolean,
  compositionActive = false,
  primaryModifierAllowed = false,
  editableControlAllowed = false,
): boolean {
  return (
    enabled &&
    !compositionActive &&
    !event.isComposing &&
    (primaryModifierAllowed || (!event.ctrlKey && !event.metaKey)) &&
    !event.altKey &&
    !event.getModifierState("AltGraph") &&
    (editableControlAllowed || !pathMatches(event, editableControlSelector)) &&
    !pathMatches(event, overlaySelector) &&
    !document.querySelector(overlaySelector)
  );
}

export function eventPathContainsActionControl(event: KeyboardEvent): boolean {
  return pathMatches(event, 'button, a[href], [role="button"]');
}

function pathMatches(event: KeyboardEvent, selector: string): boolean {
  return event
    .composedPath()
    .some((entry) => entry instanceof Element && entry.matches(selector));
}
