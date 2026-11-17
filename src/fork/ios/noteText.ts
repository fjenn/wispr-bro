// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Text helpers for the iPhone note editor (`./IosNote`): a dictated take
 * placed at the caret, and the word count.
 */

export interface Caret {
  start: number;
  end: number;
}

/**
 * The product's leading-space rule (`src-tauri/src/fork/insert/spacing.rs`
 * `needs_leading_space`; the keyboard's `WBInsertion.swift`): a space only
 * after a letter, a digit or a closing mark.
 */
const BEFORE_WANTS_SPACE = /[\p{L}\p{N}.,;:!?…%)\]}"'”’»]$/u;
/** A take that starts with closing punctuation attaches to what is there. */
const TAKE_ATTACHES = /^[.,;:!?…)\]}”’»]/u;
/** What follows the caret already separates itself: white space or a closing mark. */
const AFTER_SEPARATES = /^[\s.,;:!?…)\]}”’»]/u;

/**
 * Puts `text` where the caret is (replacing a selection; at the end of the
 * note without a caret). One space goes in front after a letter, a digit
 * or closing punctuation: never at the start, after white space or a new
 * line, or after an opening bracket or quote. One space follows when a
 * word comes right after the caret, never a second one. Returns the new
 * body and where the caret goes.
 */
export function insertAtCaret(
  body: string,
  text: string,
  caret: Caret | null,
): { body: string; caret: number } {
  const start = Math.min(Math.max(caret?.start ?? body.length, 0), body.length);
  const end = Math.min(Math.max(caret?.end ?? start, start), body.length);
  const before = body.slice(0, start);
  const after = body.slice(end);
  const take = text.trim();
  const lead =
    BEFORE_WANTS_SPACE.test(before) && !TAKE_ATTACHES.test(take) ? " " : "";
  const trail = after && !AFTER_SEPARATES.test(after) ? " " : "";
  return {
    body: `${before}${lead}${take}${trail}${after}`,
    caret: before.length + lead.length + take.length,
  };
}

const WORD = /[\p{L}\p{N}]+(?:['’\-][\p{L}\p{N}]+)*/gu;

/** Words in a note ("don't" and "well-known" are one each). */
export function countWords(body: string): number {
  return body.match(WORD)?.length ?? 0;
}
