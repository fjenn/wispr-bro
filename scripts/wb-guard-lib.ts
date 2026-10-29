// SPDX-License-Identifier: GPL-3.0-or-later
//
// Shared helpers for the Wispr Bro CI guards (F-03, R5 §4.12).
//
// Every guard exports a `run…(opts)` function that returns a `GuardResult`
// and never throws for a rule violation, so the self-tests
// (`scripts/wb-guards.test.ts`) can run it on a fixture tree. The CLI wrapper
// (`cliMain`) prints the result and sets the exit code.

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";

export const REPO_ROOT = resolve(import.meta.dir, "..");

export interface Violation {
  /** Repo-relative path (or a pseudo path such as `Cargo.lock`). */
  file: string;
  /** 1-based line, when known. */
  line?: number;
  /** Short rule id, e.g. `hosts/fork`. */
  rule: string;
  message: string;
}

export interface GuardResult {
  guard: string;
  violations: Violation[];
  /** Set when the guard could not run (e.g. no build). Not a failure. */
  skipped?: string;
  /** Informational lines printed after the verdict. */
  notes: string[];
}

export function newResult(guard: string): GuardResult {
  return { guard, violations: [], notes: [] };
}

export function formatViolation(v: Violation): string {
  const loc = v.line ? `${v.file}:${v.line}` : v.file;
  return `  ✗ ${loc} [${v.rule}] ${v.message}`;
}

export function printResult(r: GuardResult): void {
  if (r.skipped) {
    console.log(`- ${r.guard}: SKIPPED (${r.skipped})`);
  } else if (r.violations.length === 0) {
    console.log(`✓ ${r.guard}: pass`);
  } else {
    console.log(`✗ ${r.guard}: ${r.violations.length} violation(s)`);
    for (const v of r.violations) console.log(formatViolation(v));
  }
  for (const n of r.notes) console.log(`    ${n}`);
}

export function failed(r: GuardResult): boolean {
  return !r.skipped && r.violations.length > 0;
}

/** Run a guard from the command line; exit 1 on any violation. */
export async function cliMain(
  run: () => GuardResult | Promise<GuardResult>,
): Promise<void> {
  try {
    const r = await run();
    printResult(r);
    process.exit(failed(r) ? 1 : 0);
  } catch (e) {
    console.error(`guard crashed: ${(e as Error).stack ?? e}`);
    process.exit(2);
  }
}

export function argValue(argv: string[], name: string): string | undefined {
  const i = argv.indexOf(name);
  if (i >= 0 && i + 1 < argv.length) return argv[i + 1];
  const eq = argv.find((a) => a.startsWith(`${name}=`));
  return eq?.slice(name.length + 1);
}

// ---------------------------------------------------------------------------
// Files

const WALK_SKIP = new Set([
  "node_modules",
  "target",
  "dist",
  ".git",
  "gen",
  "test-results",
  "playwright-report",
]);

/**
 * Repo-relative paths (forward slashes) of every tracked or untracked,
 * non-ignored file. Uses git when `root/.git` exists, otherwise walks the
 * tree (fixtures), skipping build and dependency directories.
 */
export function listFiles(root: string): string[] {
  if (existsSync(join(root, ".git"))) {
    const out = git(root, [
      "ls-files",
      "-z",
      "--cached",
      "--others",
      "--exclude-standard",
    ]);
    return [
      ...new Set(
        out
          .split("\0")
          .filter(Boolean)
          .filter((f) => existsSync(join(root, f))),
      ),
    ].sort();
  }
  const files: string[] = [];
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      if (WALK_SKIP.has(name)) continue;
      const full = join(dir, name);
      const st = statSync(full);
      if (st.isDirectory()) walk(full);
      else if (st.isFile()) files.push(toPosix(relative(root, full)));
    }
  };
  walk(root);
  return files.sort();
}

export function toPosix(p: string): string {
  return sep === "/" ? p : p.split(sep).join("/");
}

export function readText(root: string, rel: string): string {
  return readFileSync(join(root, rel), "utf8");
}

export function readIfExists(root: string, rel: string): string | undefined {
  const p = join(root, rel);
  return existsSync(p) ? readFileSync(p, "utf8") : undefined;
}

const TEXT_EXT = new Set([
  "ts",
  "tsx",
  "js",
  "jsx",
  "mjs",
  "cjs",
  "rs",
  "json",
  "md",
  "css",
  "html",
  "swift",
  "toml",
  "sql",
  "txt",
  "yml",
  "yaml",
  "sh",
  "m",
  "mm",
  "h",
  "c",
  "cc",
  "cpp",
  "plist",
  "conf",
  "py",
]);

export function isTextFile(rel: string): boolean {
  const ext = rel.split(".").pop()?.toLowerCase() ?? "";
  return TEXT_EXT.has(ext);
}

// ---------------------------------------------------------------------------
// Globs

/**
 * Minimal glob → RegExp: `**` matches across `/`, `*` within a segment,
 * `?` one char. A pattern without `/` matches the basename too.
 */
export function globToRegExp(glob: string): RegExp {
  let re = "";
  for (let i = 0; i < glob.length; i++) {
    const c = glob[i];
    if (c === "*") {
      if (glob[i + 1] === "*") {
        i++;
        if (glob[i + 1] === "/") {
          i++;
          re += "(?:.*/)?";
        } else {
          re += ".*";
        }
      } else {
        re += "[^/]*";
      }
    } else if (c === "?") {
      re += "[^/]";
    } else {
      re += c.replace(/[.+^${}()|[\]\\]/g, "\\$&");
    }
  }
  return new RegExp(`^${re}$`);
}

export function matchesAny(path: string, globs: string[]): boolean {
  const base = path.split("/").pop() ?? path;
  return globs.some((g) => {
    const re = globToRegExp(g);
    return re.test(path) || (!g.includes("/") && re.test(base));
  });
}

// ---------------------------------------------------------------------------
// Fork paths (R5 §4.1). One list, shared by the hook, host and SPDX guards.

/** Files and directories that belong to the fork (not upstream Handy). */
export const FORK_PATHS: string[] = [
  "src-tauri/src/fork/**",
  "src-tauri/crates/wb-*/**",
  "src/fork/**",
  "scripts/wb-*",
  "scripts/wb-*/**",
  "src/i18n/locales/*/fork.json",
  "src/i18n/locales/*/fork.flags.json",
  "src-tauri/tauri.fork.conf.json",
  "src-tauri/capabilities/fork.json",
  "src-tauri/icons-fork/**",
  "src-tauri/resources/fork/**",
  "tests/fork/**",
  "playwright.fork.config.ts",
  ".github/workflows/wb-*",
  "ci/**",
  "docs/fork/**",
  "docs/research/**",
  "eval/**",
  "CONTEXT.md",
  "CONTRIBUTING-WB.md",
  "NOTICE",
  "THIRD-PARTY-NOTICES.md",
  "LICENSES/**",
  "about.toml",
  "about.hbs",
  ".wb-prettierignore",
];

export function isForkPath(rel: string): boolean {
  return FORK_PATHS.some((g) => globToRegExp(g).test(rel));
}

/** Fork *code* paths scanned by the host and SPDX guards (R5 §4.12). */
export const FORK_CODE_PATHS: string[] = [
  "src-tauri/src/fork/**",
  "src-tauri/crates/wb-*/**",
  "src/fork/**",
  "scripts/wb-*",
  "scripts/wb-*/**",
];

export function isForkCodePath(rel: string): boolean {
  return FORK_CODE_PATHS.some((g) => globToRegExp(g).test(rel));
}

// ---------------------------------------------------------------------------
// ci/*.txt list files: one entry per line, `#` starts a comment,
// `[section]` lines start a section.

export interface ListEntry {
  value: string;
  section: string;
  line: number;
  /** Text after `#` on the same line, trimmed. */
  comment: string;
}

export function parseList(text: string): ListEntry[] {
  const out: ListEntry[] = [];
  let section = "";
  text.split("\n").forEach((raw, i) => {
    const hash = raw.indexOf("#");
    const body = (hash >= 0 ? raw.slice(0, hash) : raw).trim();
    const comment = hash >= 0 ? raw.slice(hash + 1).trim() : "";
    if (!body) return;
    const sec = body.match(/^\[([a-z0-9-]+)\]$/i);
    if (sec) {
      section = sec[1].toLowerCase();
      return;
    }
    out.push({ value: body, section, line: i + 1, comment });
  });
  return out;
}

export function readList(root: string, rel: string): ListEntry[] {
  const text = readIfExists(root, rel);
  if (text === undefined) throw new Error(`missing list file ${rel}`);
  return parseList(text);
}

// ---------------------------------------------------------------------------
// Hosts

export interface UrlHit {
  host: string;
  url: string;
  line: number;
  /** The full source line (for context rules such as `link:` hosts). */
  lineText: string;
  index: number;
}

// A URL ends at whitespace, a quote or bracket, or where the next URL starts
// (binary strings glue literals together: `…/v4OpenRouterhttps://…`).
const URL_RE =
  /(?:https?|wss?|ftp):\/\/(?:(?!(?:https?|wss?|ftp):\/\/)[^\s"'`<>()[\]{}\\,;])+/gi;

/** Extract the host part of every absolute http(s)/ws(s)/ftp URL literal. */
export function findUrls(text: string): UrlHit[] {
  const hits: UrlHit[] = [];
  const lineStarts = [0];
  for (let i = 0; i < text.length; i++)
    if (text[i] === "\n") lineStarts.push(i + 1);
  const lineOf = (idx: number) => {
    let lo = 0;
    let hi = lineStarts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (lineStarts[mid] <= idx) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  };
  for (const m of text.matchAll(URL_RE)) {
    const host = hostOf(m[0]);
    if (!host) continue;
    const ln = lineOf(m.index ?? 0);
    const start = lineStarts[ln];
    const end = text.indexOf("\n", start);
    hits.push({
      host,
      url: m[0],
      line: ln + 1,
      lineText: text.slice(start, end < 0 ? undefined : end),
      index: m.index ?? 0,
    });
  }
  return hits;
}

export function hostOf(url: string): string | undefined {
  const m = url.match(/^[a-z]+:\/\/(?:[^@/?#]*@)?(\[[^\]]+\]|[^:/?#]+)/i);
  if (!m) return undefined;
  const host = m[1].toLowerCase().replace(/\.$/, "");
  // Skip template placeholders such as `https://${host}` or `http://{}`.
  if (!/^[a-z0-9.[\]:-]+$/.test(host) || !/[a-z0-9]/.test(host))
    return undefined;
  return host;
}

/** `*.hf.co` matches `cas-bridge.xethub.hf.co` but not `hf.co`. */
export function hostMatches(host: string, pattern: string): boolean {
  const p = pattern.toLowerCase();
  if (p.startsWith("*.")) return host.endsWith(p.slice(1));
  return host === p;
}

export function hostInList(host: string, patterns: string[]): boolean {
  return patterns.some((p) => hostMatches(host, p));
}

export function isLoopbackHost(host: string): boolean {
  return (
    host === "localhost" ||
    host.endsWith(".localhost") ||
    host === "127.0.0.1" ||
    host === "[::1]" ||
    host === "0.0.0.0"
  );
}

// ---------------------------------------------------------------------------
// git

export function git(cwd: string, args: string[]): string {
  const r = spawnSync("git", args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  });
  if (r.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${r.stderr.trim()}`);
  }
  return r.stdout;
}

export function tryGit(cwd: string, args: string[]): string | undefined {
  try {
    return git(cwd, args);
  } catch {
    return undefined;
  }
}

// ---------------------------------------------------------------------------
// Comments

/** Strip `//` and `/* *\/` comments (string-unaware; fine for build.rs). */
export function stripRustComments(src: string): string {
  return src.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
}
