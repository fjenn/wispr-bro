// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Success detection for practice steps (S-31, B-40): a practice step names
 * the shortcuts it waits for (fork action ids and their chords) and hears
 * them from both places they can arrive:
 *
 * - the page's own keydown, when the feature's shortcut is not registered
 *   (Transforms off: the key reaches the web view; the typed character,
 *   e.g. ⌥1's "¡", is suppressed);
 * - `fork-lesson-shortcut`, when the fork tap caught it (Transforms on) and
 *   Wispr Bro was in front: while a practice step is mounted it keeps
 *   `fork_lesson_practice` on (renewed every 60 s, off on unmount) so the
 *   backend hands the key here instead of running it
 *   (`fork/tutorials/practice.rs`).
 *
 * What counts as success (text selected, words matched…) is the step's
 * call: the detector only reports which action fired. `matchChord` and
 * `chordForEvent` are pure and unit-tested (`SuccessDetector.test.ts`).
 */
import { useEffect, useRef } from "react";
import { commands, events } from "../bindings";

/** The parts of a KeyboardEvent the matcher reads. */
export interface KeyLike {
  key: string;
  code: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

const MODIFIERS: Record<string, "alt" | "ctrl" | "meta" | "shift"> = {
  option: "alt",
  alt: "alt",
  ctrl: "ctrl",
  control: "ctrl",
  command: "meta",
  cmd: "meta",
  shift: "shift",
};

/** The key an event names, layout-independent for letters and digits. */
export function eventKeyName(event: KeyLike): string {
  const digit = /^Digit(\d)$/.exec(event.code);
  if (digit) return digit[1];
  const letter = /^Key([A-Z])$/.exec(event.code);
  if (letter) return letter[1].toLowerCase();
  if (event.code === "Space") return "space";
  if (event.key === "Enter") return "return";
  return event.key.toLowerCase();
}

/**
 * Whether `event` is the chord (fork hotkey grammar: `option+1`,
 * `ctrl+option+o`). Modifiers must match exactly; a chord with no key
 * (modifier-only, `fn`) never matches a keydown.
 */
export function matchChord(chord: string, event: KeyLike): boolean {
  const parts = chord.toLowerCase().split("+").filter(Boolean);
  const want = { alt: false, ctrl: false, meta: false, shift: false };
  let key: string | null = null;
  for (const part of parts) {
    const modifier = MODIFIERS[part];
    if (modifier) want[modifier] = true;
    else if (key === null) key = part;
    else return false;
  }
  if (!key || key === "fn") return false;
  return (
    want.alt === event.altKey &&
    want.ctrl === event.ctrlKey &&
    want.meta === event.metaKey &&
    want.shift === event.shiftKey &&
    eventKeyName(event) === key
  );
}

/** The action whose chord the event is, if any. */
export function actionForEvent(
  chords: Readonly<Record<string, readonly string[]>>,
  event: KeyLike,
): string | null {
  for (const [action, list] of Object.entries(chords)) {
    if (list.some((chord) => matchChord(chord, event))) return action;
  }
  return null;
}

/** How often the open practice step renews the backend mode. */
export const PRACTICE_RENEW_MS = 60_000;

/**
 * Listens for the step's shortcuts while `active`: keydown on the document
 * (capture phase, so a focused textarea does not type the character) and
 * the backend's `fork-lesson-shortcut`. `onAction` gets the action id.
 */
export function useLessonShortcuts(
  chords: Readonly<Record<string, readonly string[]>>,
  onAction: (action: string) => void,
  active = true,
): void {
  const handler = useRef(onAction);
  handler.current = onAction;
  const chordsRef = useRef(chords);
  chordsRef.current = chords;

  useEffect(() => {
    if (!active) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.repeat || event.isComposing) return;
      const action = actionForEvent(chordsRef.current, event);
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      handler.current(action);
    };
    document.addEventListener("keydown", onKey, true);

    void commands.forkLessonPractice(true).catch(() => undefined);
    const renew = window.setInterval(
      () => void commands.forkLessonPractice(true).catch(() => undefined),
      PRACTICE_RENEW_MS,
    );
    let live = true;
    let off: (() => void) | undefined;
    void events.forkLessonShortcut
      .listen((event) => {
        const action = event.payload.action;
        // Only the step's own actions; anything else the backend sent while
        // the step was open is ignored.
        if (action in chordsRef.current) handler.current(action);
      })
      .then((unlisten) => {
        if (live) off = unlisten;
        else unlisten();
      })
      .catch(() => undefined);

    return () => {
      live = false;
      document.removeEventListener("keydown", onKey, true);
      window.clearInterval(renew);
      off?.();
      void commands.forkLessonPractice(false).catch(() => undefined);
    };
  }, [active]);
}
