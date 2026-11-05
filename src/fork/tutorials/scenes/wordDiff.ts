// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A small word diff for drawn scenes (S-31): the How it works scene shows
 * removed and added words for two fixed strings, without a backend call
 * (real runs use `fork_transform_diff`, S-22). Longest common subsequence
 * over words; whitespace stays attached to the word after it, as the
 * backend's ops do. Our own code.
 */
import type { DiffOp } from "../../bindings";

function tokens(text: string): string[] {
  return text.match(/\s*\S+/g) ?? [];
}

const bare = (token: string) => token.trim();

export function wordDiff(before: string, after: string): DiffOp[] {
  const a = tokens(before);
  const b = tokens(after);
  const lcs: number[][] = Array.from({ length: a.length + 1 }, () =>
    new Array<number>(b.length + 1).fill(0),
  );
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lcs[i][j] =
        bare(a[i]) === bare(b[j])
          ? lcs[i + 1][j + 1] + 1
          : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const ops: DiffOp[] = [];
  const push = (kind: DiffOp["kind"], text: string) => {
    const last = ops[ops.length - 1];
    if (last && last.kind === kind) last.text += text;
    else ops.push({ kind, text });
  };
  let i = 0;
  let j = 0;
  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && bare(a[i]) === bare(b[j])) {
      push("equal", b[j]);
      i++;
      j++;
    } else if (
      j < b.length &&
      (i === a.length || lcs[i][j + 1] >= lcs[i + 1][j])
    ) {
      push("insert", b[j]);
      j++;
    } else {
      push("delete", a[i]);
      i++;
    }
  }
  return ops;
}

/** Changed runs (a delete next to an insert counts once), as S-22 counts. */
export function changeCount(ops: readonly DiffOp[]): number {
  let count = 0;
  let inChange = false;
  for (const op of ops) {
    if (op.kind === "equal") inChange = false;
    else if (!inChange) {
      count++;
      inChange = true;
    }
  }
  return count;
}
