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
//   - Design polish pass: no raw hex colour (`#1a1a1a`, `bg-[#fff]`) in
//     fork frontend code outside `src/fork/styles/tokens.css`, so every
//     colour is a token with a light and a dark value (or a named constant
//     there: the HUD, the drawn apps' `--wb-light-*`, the wallpaper,
//     confetti). Comments, the generated `bindings.ts` and the debug-only
//     `src/fork/debug/` are not checked; a line may opt out with the
//     comment `wb-allow-colour` and a reason.
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

/** Where raw colours are checked, and the files allowed to hold them. */
const COLOUR_SCANNED = /^src\/fork\/.*\.(ts|tsx|css)$/;
const COLOUR_EXEMPT = /^src\/fork\/(styles\/tokens\.css|bindings\.ts|debug\/)/;
// `#abc`, `#abcd`, `#aabbcc`, `#aabbccdd`, not followed by more of a name
// (`#add-word` is an id, `#/fork/debug` a route).
const HEX_RE = /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?![\w-])/g;
const ALLOW = "wb-allow-colour";

/** `text` with comments blanked (line numbers kept). */
export function stripComments(text: string, css: boolean): string {
  let out = text.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "));
  if (!css) {
    // `//` to the end of the line, unless inside a string or a URL.
    out = out
      .split("\n")
      .map((line) => line.replace(/(^|[^:"'`\w])\/\/.*$/, "$1"))
      .join("\n");
  }
  return out;
}

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
  for (const f of listFiles(root)) {
    if (!COLOUR_SCANNED.test(f) || COLOUR_EXEMPT.test(f)) continue;
    const raw = readIfExists(root, f) ?? "";
    const rawLines = raw.split("\n");
    stripComments(raw, f.endsWith(".css"))
      .split("\n")
      .forEach((line, i) => {
        if (rawLines[i]?.includes(ALLOW)) return;
        for (const m of line.matchAll(HEX_RE)) {
          r.violations.push({
            file: f,
            line: i + 1,
            rule: "tokens/raw-colour",
            message: `${m[0]} is a raw colour; use a token from src/fork/styles/tokens.css (add one there if none fits)`,
          });
        }
      });
  }
  r.notes.push(`checked ${checked} fork frontend file(s)`);
  return r;
}

if (import.meta.main) cliMain(() => runTokens());
