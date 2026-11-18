// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * An open note that another device changed (Mac ↔ iPhone sync): the editor
 * takes the new text in place once the user is not typing, and the caret
 * stays where it was relative to the words around it.
 */

/** How long after the last keystroke the user counts as paused. */
export const TYPING_PAUSE_MS = 1500;

/**
 * Where a caret at `pos` in `before` lands in `after`: unchanged before
 * the changed part, moved with the text after it, at the end of the
 * changed part when it was inside it.
 */
export function mapCaret(before: string, after: string, pos: number): number {
  const max = Math.min(before.length, after.length);
  let start = 0;
  while (start < max && before[start] === after[start]) start++;
  let endBefore = before.length;
  let endAfter = after.length;
  while (
    endBefore > start &&
    endAfter > start &&
    before[endBefore - 1] === after[endAfter - 1]
  ) {
    endBefore--;
    endAfter--;
  }
  if (pos <= start) return Math.min(pos, after.length);
  if (pos >= endBefore) return pos + (endAfter - endBefore);
  return endAfter;
}
