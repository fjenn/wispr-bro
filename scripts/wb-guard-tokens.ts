// SPDX-License-Identifier: GPL-3.0-or-later
//
// Token guard (Design Phase 0b, docs/fork/design/SYSTEM-PLAN.md §1, §7).
//   - The v1 design-token names were aliases of the v2 roles until the
//     Phase 0b sweep migrated every use and deleted them. A v1 name in fork
//     code, styles or tests (`--wb-canvas`, `--wb-card`, `--wb-banner`,
//     `--wb-radius-card`…, with or without the `light-`/`dark-` infix) now
//     resolves to nothing, so it fails here instead of rendering unstyled.
//     The message names the v2 role to use.
//
// Run alone: `bun scripts/wb-guard-tokens.ts`; `bun run wb:guards` runs it
// with the rest.

import {
  cliMain,
  listFiles,
  newResult,
  readIfExists,
  REPO_ROOT,
  type GuardResult,
} from "./wb-guard-lib";

/** v1 name → the v2 role that replaced it. */
export const V1_TOKENS: Record<string, string> = {
  canvas: "panel",
  sidebar: "chrome",
  card: "surface",
  banner: "accent-soft",
  hover: "fill-hover",
  selected: "fill-active",
  "accent-fill": "ink",
  "on-accent": "on-ink",
  shadow: "elev-1",
  "radius-card": "radius-md",
  "radius-control": "radius-sm",
};

// `--wb-card` but not `--wb-card-title` or `--wb-canvas-warm`.
const V1_RE = new RegExp(
  `--wb-(?:light-|dark-)?(${Object.keys(V1_TOKENS).join("|")})(?![-\\w])`,
  "g",
);

const SCANNED = /^(src\/fork\/|src\/styles\/|tests\/fork\/)/;
const EXT = /\.(ts|tsx|js|jsx|css)$/;

export function runTokens(opts: { root?: string } = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const r = newResult("tokens");
  let checked = 0;
  for (const f of listFiles(root)) {
    if (!SCANNED.test(f) || !EXT.test(f)) continue;
    checked++;
    const lines = (readIfExists(root, f) ?? "").split("\n");
    lines.forEach((line, i) => {
      for (const m of line.matchAll(V1_RE)) {
        r.violations.push({
          file: f,
          line: i + 1,
          rule: "tokens/v1-name",
          message: `${m[0]} is a removed v1 token; use --wb-${V1_TOKENS[m[1]]}`,
        });
      }
    });
  }
  r.notes.push(`checked ${checked} fork frontend file(s)`);
  return r;
}

if (import.meta.main) cliMain(() => runTokens());
