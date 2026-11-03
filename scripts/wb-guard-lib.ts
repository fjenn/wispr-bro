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
  "src/assets/fonts/fork/**",
  "tests/fork/**",
  "playwright.fork.config.ts",
  ".github/workflows/wb-*",
  ".github/ISSUE_TEMPLATE/wb-*",
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

// ---------------------------------------------------------------------------
// Rust lexing (just enough to find comments and `#[cfg(test)]` modules)

/** Half-open source range `[start, end)` (UTF-16 offsets). */
export type Span = [number, number];

export interface RustToken {
  kind: "ident" | "punct" | "literal" | "lifetime";
  text: string;
  start: number;
  end: number;
}

export interface RustLex {
  tokens: RustToken[];
  /** Line, block (nested) and doc comments. */
  comments: Span[];
  /** False when a string, char or block comment never closes. */
  ok: boolean;
}

const IDENT_START = /[A-Za-z_\u0080-￿]/;
const IDENT_CHAR = /[A-Za-z0-9_\u0080-￿]/;

/**
 * A small Rust lexer: strings (with escapes), raw strings (`r#"…"#`, `br`,
 * `cr`), byte/C strings, char literals vs lifetimes, nested block comments.
 * Braces inside literals and comments are therefore never counted. On
 * malformed input it returns `ok: false` and callers fail closed.
 */
export function lexRust(src: string): RustLex {
  const tokens: RustToken[] = [];
  const comments: Span[] = [];
  const n = src.length;
  let i = 0;
  const quoted = (from: number): number => {
    // `from` is just past the opening `"`; returns the index past the close.
    let j = from;
    while (j < n) {
      if (src[j] === "\\") j += 2;
      else if (src[j] === '"') return j + 1;
      else j++;
    }
    return -1;
  };
  while (i < n) {
    const c = src[i];
    if (/\s/.test(c)) {
      i++;
      continue;
    }
    if (c === "/" && src[i + 1] === "/") {
      const e = src.indexOf("\n", i);
      const end = e < 0 ? n : e;
      comments.push([i, end]);
      i = end;
      continue;
    }
    if (c === "/" && src[i + 1] === "*") {
      let depth = 1;
      let j = i + 2;
      while (j < n && depth > 0) {
        if (src[j] === "/" && src[j + 1] === "*") {
          depth++;
          j += 2;
        } else if (src[j] === "*" && src[j + 1] === "/") {
          depth--;
          j += 2;
        } else j++;
      }
      if (depth > 0) return { tokens, comments, ok: false };
      comments.push([i, j]);
      i = j;
      continue;
    }
    if (c === '"') {
      const e = quoted(i + 1);
      if (e < 0) return { tokens, comments, ok: false };
      tokens.push({ kind: "literal", text: src.slice(i, e), start: i, end: e });
      i = e;
      continue;
    }
    if (c === "'") {
      let e = -1;
      if (src[i + 1] === "\\") {
        e = src.indexOf("'", i + 3);
        if (e >= 0) e++;
      } else {
        const cp = src.codePointAt(i + 1);
        const w = cp !== undefined && cp > 0xffff ? 2 : 1;
        if (src[i + 1] !== undefined && src[i + 1 + w] === "'") e = i + 2 + w;
      }
      if (e > 0) {
        tokens.push({
          kind: "literal",
          text: src.slice(i, e),
          start: i,
          end: e,
        });
        i = e;
        continue;
      }
      // Lifetime or label: `'a`, `'static`.
      let j = i + 1;
      while (j < n && IDENT_CHAR.test(src[j])) j++;
      tokens.push({
        kind: "lifetime",
        text: src.slice(i, j),
        start: i,
        end: j,
      });
      i = j;
      continue;
    }
    if (IDENT_START.test(c) || /[0-9]/.test(c)) {
      let j = i + 1;
      while (j < n && IDENT_CHAR.test(src[j])) j++;
      const word = src.slice(i, j);
      if (/^(r|br|cr)$/.test(word) && (src[j] === "#" || src[j] === '"')) {
        let k = j;
        while (src[k] === "#") k++;
        if (src[k] === '"') {
          const close = '"' + "#".repeat(k - j);
          const e = src.indexOf(close, k + 1);
          if (e < 0) return { tokens, comments, ok: false };
          const end = e + close.length;
          tokens.push({
            kind: "literal",
            text: src.slice(i, end),
            start: i,
            end,
          });
          i = end;
          continue;
        }
      }
      if (/^(b|c)$/.test(word) && src[j] === '"') {
        const e = quoted(j + 1);
        if (e < 0) return { tokens, comments, ok: false };
        tokens.push({
          kind: "literal",
          text: src.slice(i, e),
          start: i,
          end: e,
        });
        i = e;
        continue;
      }
      if (word === "b" && src[j] === "'") {
        const e = src[j + 1] === "\\" ? src.indexOf("'", j + 3) : j + 2;
        if (e < 0 || src[e] !== "'") return { tokens, comments, ok: false };
        tokens.push({
          kind: "literal",
          text: src.slice(i, e + 1),
          start: i,
          end: e + 1,
        });
        i = e + 1;
        continue;
      }
      tokens.push({
        kind: /[0-9]/.test(c) ? "literal" : "ident",
        text: word,
        start: i,
        end: j,
      });
      i = j;
      continue;
    }
    tokens.push({ kind: "punct", text: c, start: i, end: i + 1 });
    i++;
  }
  return { tokens, comments, ok: true };
}

export interface CfgTestMod {
  name: string;
  /** `decl`: `#[cfg(test)] mod name;` (body in another file). */
  kind: "decl" | "inline";
  /** From the `#` of `#[cfg(test)]` to just past `;` or the closing `}`. */
  span: Span;
}

/**
 * Modules gated by exactly `#[cfg(test)]` (further attributes and a `pub` /
 * `pub(…)` visibility may follow): `#[cfg(test)] mod x;` declarations and
 * `#[cfg(test)] mod x { … }` blocks, with the block's extent found by brace
 * matching on the token stream. `cfg(any(test, …))` and other forms are not
 * recognized (callers fail closed). Returns [] if the file does not lex.
 */
export function rustCfgTestMods(src: string): CfgTestMod[] {
  const lex = lexRust(src);
  if (!lex.ok) return [];
  const t = lex.tokens;
  const is = (k: number, text: string) =>
    t[k]?.text === text && t[k].kind !== "literal";
  /** Index past a `#[ … ]` attribute starting at k, or -1. */
  const skipAttr = (k: number): number => {
    if (!is(k, "#") || !is(k + 1, "[")) return -1;
    let depth = 0;
    for (let j = k + 1; j < t.length; j++) {
      if (t[j].kind !== "punct") continue;
      if (t[j].text === "[") depth++;
      else if (t[j].text === "]" && --depth === 0) return j + 1;
    }
    return -1;
  };
  const out: CfgTestMod[] = [];
  for (let k = 0; k < t.length; k++) {
    if (
      !(
        is(k, "#") &&
        is(k + 1, "[") &&
        is(k + 2, "cfg") &&
        is(k + 3, "(") &&
        is(k + 4, "test") &&
        is(k + 5, ")") &&
        is(k + 6, "]")
      )
    )
      continue;
    let j = k + 7;
    for (let next = skipAttr(j); next > 0; next = skipAttr(j)) j = next;
    if (is(j, "pub")) {
      j++;
      if (is(j, "(")) {
        while (j < t.length && !is(j, ")")) j++;
        j++;
      }
    }
    if (!is(j, "mod") || t[j + 1]?.kind !== "ident") continue;
    const name = t[j + 1].text;
    if (is(j + 2, ";")) {
      out.push({ name, kind: "decl", span: [t[k].start, t[j + 2].end] });
      continue;
    }
    if (!is(j + 2, "{")) continue;
    let depth = 0;
    for (let m = j + 2; m < t.length; m++) {
      if (t[m].kind !== "punct") continue;
      if (t[m].text === "{") depth++;
      else if (t[m].text === "}" && --depth === 0) {
        out.push({ name, kind: "inline", span: [t[k].start, t[m].end] });
        k = m;
        break;
      }
    }
  }
  return out;
}

export function inSpans(index: number, spans: Span[]): boolean {
  return spans.some(([s, e]) => index >= s && index < e);
}
