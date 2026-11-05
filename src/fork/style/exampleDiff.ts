// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * What a preset's example changes against the group's Formal one (owner's
 * rc10 report: the cards looked the same). The style rules
 * (`fork/text/style_rules.rs`) only drop commas and the final period, turn
 * that period into "!" and lower-case sentence starts, so the difference is
 * a few characters; the cards mark them instead of the examples inventing
 * effects the app does not apply. Pure, so `exampleDiff.test.ts` checks it
 * without a browser.
 */

export type DiffPart =
  /** Text the preset leaves as Formal has it. */
  | { kind: "same"; text: string }
  /** Characters the preset changed (case, "." to "!"). */
  | { kind: "changed"; text: string }
  /** Characters of Formal the preset dropped (a comma, the final period). */
  | { kind: "removed"; text: string };

/** The same character, or the same one changed by a rule. */
function aligned(a: string, b: string): boolean {
  if (a === b) return true;
  if (a.toLowerCase() === b.toLowerCase()) return true;
  return (a === "." || a === "!") && (b === "." || b === "!");
}

/**
 * `text` as parts against `formal`: a longest common subsequence over
 * characters, where a case change or "."/"!" still lines up. Characters
 * only in `formal` come back as `removed`; characters only in `text` (no
 * rule adds any) as `changed`.
 */
export function diffFromFormal(formal: string, text: string): DiffPart[] {
  const a = Array.from(formal);
  const b = Array.from(text);
  const n = a.length;
  const m = b.length;
  // lcs[i][j]: the longest alignment of a[i..] and b[j..].
  const lcs = Array.from({ length: n + 1 }, () =>
    new Array<number>(m + 1).fill(0),
  );
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i][j] = aligned(a[i], b[j])
        ? lcs[i + 1][j + 1] + 1
        : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const parts: DiffPart[] = [];
  const push = (kind: DiffPart["kind"], ch: string) => {
    const last = parts[parts.length - 1];
    if (last && last.kind === kind) last.text += ch;
    else parts.push({ kind, text: ch });
  };
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && aligned(a[i], b[j])) {
      push(a[i] === b[j] ? "same" : "changed", b[j]);
      i++;
      j++;
    } else if (i < n && (j >= m || lcs[i + 1][j] >= lcs[i][j + 1])) {
      push("removed", a[i]);
      i++;
    } else {
      push("changed", b[j]);
      j++;
    }
  }
  return parts;
}

/** Characters marked (changed or removed): 0 means the cards look alike. */
export function markedCount(parts: readonly DiffPart[]): number {
  return parts
    .filter((p) => p.kind !== "same")
    .reduce((n, p) => n + Array.from(p.text).length, 0);
}
