// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Text helpers for the iPhone note editor (`./IosNote`): a dictated take
 * placed at the caret, and the word count.
 */

export interface Caret {
  start: number;
  end: number;
}

const OPENERS = /[\s([{“‘"'«¿¡/-]$/u;
const CLOSERS = /^[\s.,;:!?)\]}”’»]/u;

/**
 * Puts `text` where the caret is (replacing a selection; at the end of the
 * note without a caret), with one space towards a word on either side:
 * what the keyboard does for a take dictated into a field. Returns the new
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
  const lead = before && !OPENERS.test(before) ? " " : "";
  const trail = after && !CLOSERS.test(after) ? " " : "";
  const inserted = `${lead}${text}${trail}`;
  return {
    body: `${before}${inserted}${after}`,
    caret: before.length + lead.length + text.length,
  };
}

const WORD = /[\p{L}\p{N}]+(?:['’\-][\p{L}\p{N}]+)*/gu;

/** Words in a note ("don't" and "well-known" are one each). */
export function countWords(body: string): number {
  return body.match(WORD)?.length ?? 0;
}
