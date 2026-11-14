// SPDX-License-Identifier: GPL-3.0-or-later
//
// Edition guard (docs/fork/EDITIONS.md): the normal Wispr Bro build contains
// nothing from the publik edition.
//
// Source mode (always, also inside `wb:guards`):
//   - publik markers (its domain, its API paths, its feed and asset names,
//     its self-test switch) appear in app code (src/, src-tauri/) only inside
//     the publik folders and src-tauri/tauri.publik.conf.json;
//   - Rust reaches `fork::publik` only behind `#[cfg(feature = "publik")]`;
//   - the frontend imports `…/publik/…` only from src/fork/edition.ts;
//   - nothing a normal user could see says "publik": no frontend file (UI,
//     settings, about, translations) outside the publik folder and the
//     gateway mentions it, and no Rust string literal outside the publik
//     folder does (`feature = "publik"` cfgs aside);
//   - the `publik` Cargo feature is never on by default, and the normal
//     Tauri configs never turn it on.
//
// Artifact mode (release scripts, after a build):
//   bun scripts/wb-guard-edition.ts --edition standard --dist dist --binary X.app
//     the built frontend and app contain no publik marker, and not the word
//     "publik" in any spelling;
//   bun scripts/wb-guard-edition.ts --edition publik --binary X.app
//     the app reads the publik update feed (so the feature really was on).

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import {
  PUBLIK_PATHS,
  REPO_ROOT,
  argValue,
  isPublikPath,
  cliMain,
  listFiles,
  newResult,
  readIfExists,
  type GuardResult,
} from "./wb-guard-lib";
import { printableStrings } from "./wb-guard-bundle-hosts";

/** Strings that only the publik edition may contain. */
export const PUBLIK_MARKERS: string[] = [
  "publik" + "hq",
  "/api/v1/" + "installs",
  "/api/v1/" + "telemetry",
  "latest-" + "publik.json",
  "SHA256SUMS-" + "publik",
  "Wispr-Bro-" + "publik",
  "PUBLIK_" + "SELF_TEST",
];

export { PUBLIK_PATHS, isPublikPath };

/** The string a publik build must contain: its own update feed. */
export const PUBLIK_REQUIRED = "latest-" + "publik.json";

/** The one frontend file that may import the publik folder. */
export const FRONTEND_GATEWAY = "src/fork/edition.ts";

const APP_CODE = /^(src|src-tauri)\//;
const SKIP = /^src-tauri\/(target|gen)\//;
const BINARY_EXT =
  /\.(png|jpe?g|gif|webp|icns|ico|ttf|otf|woff2?|wav|mp3|bin|gguf|onnx|dylib|so|a|zip|gz)$/i;

/** A positive `feature = "publik"` cfg (not `not(feature = "publik")`). */
function gatesOnPublik(line: string): boolean {
  if (!/feature\s*=\s*"publik"/.test(line)) return false;
  return !/not\s*\(\s*feature\s*=\s*"publik"/.test(line);
}

/** A string literal on the line mentions publik (cfg attributes aside). */
function rustStringMentionsPublik(line: string): boolean {
  const code = line.replace(/feature\s*=\s*"publik"/g, "");
  return /"[^"\n]*publik[^"\n]*"/i.test(code);
}

const RUST_USE_RE = /\bmod\s+publik\b|\bpublik::/;
const TS_IMPORT_RE =
  /(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s+|\brequire\s*\(\s*)["'`]([^"'`]*)["'`]/g;

export interface EditionOptions {
  root?: string;
  /** Artifact mode: which edition the artifacts are. */
  edition?: string;
  /** Built frontend directory to scan (artifact mode). */
  dist?: string;
  /** Built app bundles or executables to scan (artifact mode). */
  binaries?: string[];
}

export function runEdition(opts: EditionOptions = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const r = newResult("edition");
  if (opts.edition !== undefined) {
    scanArtifacts(r, opts);
    return r;
  }
  scanSources(r, root);
  return r;
}

function scanSources(r: GuardResult, root: string): void {
  for (const rel of listFiles(root)) {
    if (!APP_CODE.test(rel) || SKIP.test(rel) || BINARY_EXT.test(rel)) continue;
    if (isPublikPath(rel)) continue;
    const text = readFileSync(join(root, rel), "latin1");
    const lines = text.split("\n");
    lines.forEach((line, i) => {
      for (const m of PUBLIK_MARKERS) {
        if (line.includes(m))
          r.violations.push({
            file: rel,
            line: i + 1,
            rule: "edition/marker",
            message: `"${m}" belongs in the publik folders only`,
          });
      }
      if (
        rel.startsWith("src/") &&
        rel !== FRONTEND_GATEWAY &&
        /publik/i.test(line)
      )
        r.violations.push({
          file: rel,
          line: i + 1,
          rule: "edition/text",
          message: "normal frontend files must not mention publik",
        });
      if (rel.endsWith(".rs") && rustStringMentionsPublik(line))
        r.violations.push({
          file: rel,
          line: i + 1,
          rule: "edition/text",
          message: "a Rust string outside the publik folder mentions publik",
        });
      if (rel.endsWith(".rs") && RUST_USE_RE.test(line)) {
        const window = lines.slice(Math.max(0, i - 3), i + 1);
        if (!window.some(gatesOnPublik))
          r.violations.push({
            file: rel,
            line: i + 1,
            rule: "edition/rust-cfg",
            message:
              'reaches fork::publik without #[cfg(feature = "publik")] on it',
          });
      }
      if (/\.(ts|tsx|js|jsx|mjs)$/.test(rel) && rel !== FRONTEND_GATEWAY) {
        for (const m of line.matchAll(TS_IMPORT_RE)) {
          if (/(^|\/)publik(\/|$)/.test(m[1]))
            r.violations.push({
              file: rel,
              line: i + 1,
              rule: "edition/import",
              message: `imports "${m[1]}"; only ${FRONTEND_GATEWAY} may`,
            });
        }
      }
    });
  }

  const cargo = readIfExists(root, "src-tauri/Cargo.toml") ?? "";
  cargo.split("\n").forEach((line, i) => {
    if (/^\s*default\s*=.*"publik"/.test(line))
      r.violations.push({
        file: "src-tauri/Cargo.toml",
        line: i + 1,
        rule: "edition/default",
        message: "the publik feature must stay off by default",
      });
  });
  for (const rel of [
    "src-tauri/tauri.conf.json",
    "src-tauri/tauri.fork.conf.json",
  ]) {
    const raw = readIfExists(root, rel);
    if (!raw) continue;
    const features = (JSON.parse(raw)?.build?.features ?? []) as string[];
    if (features.includes("publik"))
      r.violations.push({
        file: rel,
        rule: "edition/config",
        message: "the normal configs must not turn the publik feature on",
      });
  }
}

function walkFiles(dir: string, acc: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) walkFiles(full, acc);
    else if (st.isFile()) acc.push(full);
  }
  return acc;
}

/** Files to scan for one `--binary`: an .app's executables, or the file. */
function binaryFiles(path: string): string[] {
  if (statSync(path).isDirectory()) {
    const macos = join(path, "Contents", "MacOS");
    return existsSync(macos) ? walkFiles(macos) : walkFiles(path);
  }
  return [path];
}

function scanArtifacts(r: GuardResult, opts: EditionOptions): void {
  const edition = opts.edition;
  if (edition !== "standard" && edition !== "publik") {
    r.violations.push({
      file: "-",
      rule: "edition/args",
      message: `--edition must be standard or publik, not "${edition}"`,
    });
    return;
  }
  const targets: string[] = [];
  if (opts.dist) {
    if (!existsSync(opts.dist))
      r.violations.push({
        file: opts.dist,
        rule: "edition/args",
        message: "not found",
      });
    else targets.push(...walkFiles(opts.dist));
  }
  for (const b of opts.binaries ?? []) {
    if (!existsSync(b))
      r.violations.push({
        file: b,
        rule: "edition/args",
        message: "not found",
      });
    else targets.push(...binaryFiles(b));
  }
  if (targets.length === 0) {
    r.violations.push({
      file: "-",
      rule: "edition/args",
      message: "artifact mode needs --dist and/or --binary",
    });
    return;
  }

  let sawRequired = false;
  for (const file of targets) {
    const strings = printableStrings(readFileSync(file), 6);
    const found = new Set<string>();
    for (const s of strings) {
      for (const m of PUBLIK_MARKERS) if (s.includes(m)) found.add(m);
      if (found.size === 0 && /publik/i.test(s)) found.add("publik");
      if (s.includes(PUBLIK_REQUIRED)) sawRequired = true;
    }
    if (edition === "standard")
      for (const m of found)
        r.violations.push({
          file,
          rule: "edition/leak",
          message: `the normal build contains publik marker "${m}"`,
        });
  }
  r.notes.push(`${edition}: scanned ${targets.length} file(s)`);
  if (edition === "publik" && (opts.binaries ?? []).length > 0 && !sawRequired)
    r.violations.push({
      file: (opts.binaries ?? []).join(", "),
      rule: "edition/missing",
      message: `a publik build must read ${PUBLIK_REQUIRED}; was the feature on?`,
    });
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const binaries: string[] = [];
  argv.forEach((a, i) => {
    if (a === "--binary" && argv[i + 1]) binaries.push(argv[i + 1]);
  });
  await cliMain(() =>
    runEdition({
      edition: argValue(argv, "--edition"),
      dist: argValue(argv, "--dist"),
      binaries,
    }),
  );
}
