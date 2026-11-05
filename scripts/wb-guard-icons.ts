// SPDX-License-Identifier: GPL-3.0-or-later
//
// Icon guard (Design Phase 0, docs/fork/design/SYSTEM-PLAN.md §3).
//   - Fork frontend code (src/fork/) imports Lucide glyphs only through the
//     wrapper `src/fork/ui/icons.ts` (1.5 stroke, fixed sizes), never from
//     `lucide-react` directly, so the icon style never mixes again. The
//     wrapper's two files are the only exception.
//
// Run alone: `bun scripts/wb-guard-icons.ts`; `bun run wb:guards` runs it
// with the rest.

import {
  cliMain,
  listFiles,
  newResult,
  readIfExists,
  REPO_ROOT,
  type GuardResult,
} from "./wb-guard-lib";

/** The wrapper: the only fork files allowed to import `lucide-react`. */
export const ICON_WRAPPER = ["src/fork/ui/Icon.tsx", "src/fork/ui/icons.ts"];

const LUCIDE_IMPORT_RE =
  /(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)["']lucide-react(?:\/[^"']*)?["']/;

export function runIcons(opts: { root?: string } = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const r = newResult("icons");
  let checked = 0;
  for (const f of listFiles(root)) {
    if (!f.startsWith("src/fork/") || !/\.(ts|tsx|js|jsx)$/.test(f)) continue;
    if (ICON_WRAPPER.includes(f)) continue;
    checked++;
    const lines = (readIfExists(root, f) ?? "").split("\n");
    lines.forEach((line, i) => {
      if (LUCIDE_IMPORT_RE.test(line)) {
        r.violations.push({
          file: f,
          line: i + 1,
          rule: "icons/direct-lucide",
          message:
            "import glyphs from src/fork/ui/icons (the 1.5-stroke wrapper), not from lucide-react",
        });
      }
    });
  }
  r.notes.push(`checked ${checked} fork frontend file(s)`);
  return r;
}

if (import.meta.main) cliMain(() => runIcons());
