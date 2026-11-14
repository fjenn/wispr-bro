// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The smart leading space in our own windows (Scratchpad note, Hub fields).
 * Accessibility never reads our own process, so before a dictation is
 * pasted here Rust asks the focused window for the text before the caret
 * (`fork::insert::own_caret`): `fork-before-caret-request` `{ id, units }`,
 * answered with `fork-before-caret-reply` `{ id, text }`. Only a window that
 * has focus and a text field focused answers; the text goes to the app's
 * own process and nowhere else.
 */
import { emit, listen } from "@tauri-apps/api/event";

type Request = { id: number; units: number };

const TEXT_INPUTS = new Set(["text", "search", "url", "email", "tel", ""]);

/** Up to `units` UTF-16 units before the caret, or null when no text field
 * has focus (or it is a password field). */
export function textBeforeCaret(units: number): string | null {
  const el = document.activeElement;
  if (el instanceof HTMLTextAreaElement || el instanceof HTMLInputElement) {
    if (el instanceof HTMLInputElement && !TEXT_INPUTS.has(el.type)) {
      return null;
    }
    const start = el.selectionStart;
    if (start == null) return null;
    return el.value.slice(Math.max(0, start - units), start);
  }
  if (el instanceof HTMLElement && el.isContentEditable) {
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0) return null;
    const caret = sel.getRangeAt(0);
    const range = document.createRange();
    range.selectNodeContents(el);
    range.setEnd(caret.startContainer, caret.startOffset);
    const text = range.toString();
    return text.slice(Math.max(0, text.length - units));
  }
  return null;
}

let installed = false;

/** Installs the answer once per window. */
export function installBeforeCaret(): void {
  if (installed) return;
  installed = true;
  // Outside the app (tests, a plain browser) there is no event bridge.
  listen<Request>("fork-before-caret-request", (event) => {
    if (!document.hasFocus()) return;
    const text = textBeforeCaret(event.payload.units);
    if (text === null) return;
    emit("fork-before-caret-reply", { id: event.payload.id, text }).catch(
      () => {},
    );
  }).catch(() => {});
}
