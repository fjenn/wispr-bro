// SPDX-License-Identifier: GPL-3.0-or-later
//
// Licence header guard (R5 §1 GPL obligations, §4.12).
//   - Every source file under the fork code paths (src-tauri/src/fork/,
//     src-tauri/crates/wb-*, src/fork/, scripts/wb-*) has an
//     `SPDX-License-Identifier:` line in its first 5 lines, except files
//     matched by `ci/generated.txt` and formats without comments (JSON, images).
//   - Files marked `AGPL-3.0-only` are listed in `NOTICE` (sub-check skipped
//     until F-04 adds NOTICE).
//   - No file outside the fork paths carries a GPL/AGPL header (SPDX line or
//     the GNU licence boilerplate in its first 30 lines). LICENSE, NOTICE,
//     LICENSES/ and docs are exempt (they are the licence texts themselves).

import {
  cliMain,
  isForkCodePath,
  isForkPath,
  isTextFile,
  listFiles,
  matchesAny,
  newResult,
  readIfExists,
  readList,
  REPO_ROOT,
  type GuardResult,
} from "./wb-guard-lib";
import { GPL_HEADER_RE } from "./wb-guard-plumbing";

/** Extensions whose files can carry a comment header. */
const HEADER_EXT =
  /\.(rs|ts|tsx|js|jsx|mjs|cjs|mts|cts|css|swift|sh|sql|toml|ya?ml|py|m|mm|h|c|cc|cpp|html)$/;

const SPDX_RE = /SPDX-License-Identifier:\s*([A-Za-z0-9.+()\- ]+)/;

/** Outside-fork files that may contain the GPL text. */
const LICENCE_TEXT_FILES = [
  "LICENSE",
  "LICENSE.*",
  "COPYING",
  "NOTICE",
  "LICENSES/**",
  "THIRD-PARTY-NOTICES.md",
  "docs/**",
];

export function runSpdx(opts: { root?: string } = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const r = newResult("spdx");
  let generated: string[] = [];
  try {
    generated = readList(root, "ci/generated.txt").map((e) => e.value);
  } catch (e) {
    r.violations.push({
      file: "ci/generated.txt",
      rule: "spdx/list",
      message: (e as Error).message,
    });
  }
  const notice = readIfExists(root, "NOTICE");
  const agpl: string[] = [];
  let checked = 0;

  for (const f of listFiles(root)) {
    if (isForkCodePath(f)) {
      if (!HEADER_EXT.test(f)) continue;
      if (matchesAny(f, generated)) continue;
      checked++;
      const head = (readIfExists(root, f) ?? "")
        .split("\n")
        .slice(0, 5)
        .join("\n");
      const m = head.match(SPDX_RE);
      if (!m) {
        r.violations.push({
          file: f,
          line: 1,
          rule: "spdx/missing",
          message:
            "fork file has no SPDX header; add `SPDX-License-Identifier: GPL-3.0-or-later` (or list a generated file in ci/generated.txt)",
        });
        continue;
      }
      if (/AGPL-3\.0/.test(m[1])) agpl.push(f);
      continue;
    }
    if (isForkPath(f) || matchesAny(f, LICENCE_TEXT_FILES) || !isTextFile(f))
      continue;
    const text = readIfExists(root, f) ?? "";
    const head = text.split("\n").slice(0, 30).join("\n");
    const m = head.match(GPL_HEADER_RE);
    if (m) {
      r.violations.push({
        file: f,
        line: head.slice(0, m.index).split("\n").length,
        rule: "spdx/upstream-gpl",
        message:
          "GPL/AGPL header outside the fork paths: ported GPL code lives in fork files only (R5 §1 thin-layer rule)",
      });
    }
  }

  if (notice === undefined) {
    r.notes.push(
      `NOTICE not found: AGPL listing check skipped until F-04 (${agpl.length} AGPL-marked file(s))`,
    );
  } else {
    for (const f of agpl) {
      if (!notice.includes(f)) {
        r.violations.push({
          file: f,
          line: 1,
          rule: "spdx/agpl-notice",
          message: "file is marked AGPL-3.0-only but is not listed in NOTICE",
        });
      }
    }
  }
  r.notes.push(`checked ${checked} fork source file(s)`);
  return r;
}

if (import.meta.main) cliMain(() => runSpdx());
