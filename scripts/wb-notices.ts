// SPDX-License-Identifier: GPL-3.0-or-later
//
// Third-party notices generator (F-04, W-26; R5 §4.12 licence register).
//
//   bun scripts/wb-notices.ts            write THIRD-PARTY-NOTICES.md
//   bun scripts/wb-notices.ts --check    fail if the committed file is stale
//
// Options: --about-json <path> (use a cargo-about JSON report instead of
// running cargo-about), --out <path>, --skip-rust (test the npm and manual
// parts without cargo-about; never writes the real THIRD-PARTY-NOTICES.md).
//
// Inputs, all re-read on every run:
//   1. Rust crates: `cargo-about` (config `about.toml`, F-03) with a JSON
//      template, i.e. the crates that ship for the macOS targets (build and
//      dev dependencies excluded) and their licence texts. cargo-about is a
//      dev tool installed outside the project (see CARGO_ABOUT_INSTALL).
//   2. npm packages: the production closure of package.json `dependencies`
//      (walking node_modules/*/package.json `dependencies`, installed
//      optional and peer dependencies), minus BUILD_ONLY_NPM, with their
//      LICENSE/COPYING files.
//   3. C/C++ compiled into crates: the manual NATIVE list below. Every
//      shipped crate that builds native code (a `cc`/`cmake` build
//      dependency) must be in it, and every entry must still ship.
//   4. NOTICE: §6 model attributions and §7 fonts are copied verbatim; every
//      CC-BY catalog model must be named there; every NOTICE file found in a
//      shipped package must be named in §5 (Apache-2.0 §4(d)).
//
// Fails (exit 1) on a licence outside the `about.toml` accepted list
// (GPL-2.0-only, UNLICENSED, missing licence, non-free or unknown), on any
// of the list checks above, and in --check mode on a stale file. Exit 2 when
// cargo-about is missing.
//
// THIRD-PARTY-NOTICES.md is committed (reviewable diff on every dependency
// change) and CI's `licences` job runs `--check` on the same macOS runner
// type that builds releases; the release workflow attaches it
// (docs/fork/RELEASING.md).

import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { argValue, REPO_ROOT } from "./wb-guard-lib";

export const OUT_FILE = "THIRD-PARTY-NOTICES.md";

export const CARGO_ABOUT_INSTALL =
  "cargo install cargo-about --locked --version 0.9.3 --features cli";

/**
 * Packages in package.json `dependencies` that only run at build time and
 * never reach dist/ (their subtree is skipped). Reviewed list.
 */
export const BUILD_ONLY_NPM: Record<string, string> = {
  "@tailwindcss/vite":
    "Vite plugin: runs in the build (pulls in vite, esbuild, rollup, lightningcss); only the CSS it generates ships, covered by tailwindcss",
};

/**
 * npm packages whose `license` field is not a plain SPDX expression, with
 * the licence read by hand from the package. Reviewed list.
 */
export const NPM_CLARIFY: Record<string, string> = {};

export interface NativeComponent {
  name: string;
  licence: string;
  copyright: string;
  /** Licence file inside the crate, copied into the output. */
  file?: string;
  note?: string;
}

export interface NativeEntry {
  crate: string;
  how: string;
  components: NativeComponent[];
}

/**
 * C/C++ (and assembly) compiled and statically linked by shipped crates.
 * Checked against the cargo-about crate list on every run.
 */
export const NATIVE: NativeEntry[] = [
  {
    crate: "transcribe-cpp-sys",
    how: "CMake build, statically linked (speech recognition engine)",
    components: [
      {
        name: "transcribe.cpp",
        licence: "MIT",
        copyright: "Copyright (c) 2026 The transcribe.cpp authors",
        file: "LICENSE",
      },
      {
        name: "ggml (vendored under ggml/)",
        licence: "MIT",
        copyright: "Copyright (c) 2023-2026 The ggml authors",
        file: "ggml/LICENSE",
      },
      {
        name: "miniz (vendored under src/third_party/miniz/)",
        licence: "MIT",
        copyright:
          "Copyright 2013-2014 RAD Game Tools and Valve Software; Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC",
        file: "src/third_party/miniz/LICENSE",
      },
      {
        name: "Silero VAD segmentation logic (translated, src/third_party/silero_vad/)",
        licence: "MIT",
        copyright: "Copyright (c) 2020-present Silero Team",
        file: "src/third_party/silero_vad/LICENSE",
      },
    ],
  },
  {
    crate: "libsqlite3-sys",
    how: "`bundled` feature (rusqlite): the SQLite amalgamation is compiled in",
    components: [
      {
        name: "SQLite (sqlite3/sqlite3.c)",
        licence: "public domain",
        copyright:
          "The SQLite authors disclaim copyright; the source carries the SQLite blessing instead of a licence",
      },
    ],
  },
  {
    crate: "ring",
    how: "cc build, statically linked (crypto for rustls TLS)",
    components: [
      {
        name: "ring (C and assembly, incl. code from BoringSSL)",
        licence: "ISC AND Apache-2.0",
        copyright: "Copyright notices are in the licence files below",
        file: "LICENSE",
      },
      {
        name: "BoringSSL-derived code",
        licence: "Apache-2.0 / ISC (per file)",
        copyright: "Copyright notices are in the licence file below",
        file: "LICENSE-BoringSSL",
      },
      {
        name: "ring's own code",
        licence: "ISC",
        copyright: "Copyright notices are in the licence file below",
        file: "LICENSE-other-bits",
      },
    ],
  },
  {
    crate: "objc2-exception-helper",
    how: "cc build: a small Objective-C exception shim",
    components: [
      {
        name: "objc2-exception-helper C shim",
        licence: "Zlib OR Apache-2.0 OR MIT",
        copyright: "Copyright the crate's authors",
        note: "Licence texts are in the Rust section.",
      },
    ],
  },
  {
    crate: "signal-hook",
    how: "cc build: a small C helper for signal handling",
    components: [
      {
        name: "signal-hook C helper",
        licence: "Apache-2.0 OR MIT",
        copyright: "Copyright the crate's authors",
        note: "Licence texts are in the Rust section.",
      },
    ],
  },
];

/** Native-code notes that are not crates (shown in the output). */
export const NATIVE_NOTES = [
  "whisper.cpp is not linked: Handy's speech engine is transcribe.cpp (above), which vendors ggml.",
  "Handy's Apple Intelligence Swift bridge (built by src-tauri/build.rs) is Handy code under MIT (LICENSES/MIT-Handy.txt).",
];

// ---------------------------------------------------------------------------
// Licence expressions

/** `accepted = [ … ]` from about.toml. */
export function parseAccepted(aboutToml: string): string[] {
  const m = aboutToml.match(/^accepted\s*=\s*\[([\s\S]*?)\]/m);
  if (!m) throw new Error("about.toml: no `accepted = [ … ]` list");
  return [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
}

type Expr =
  | { id: string }
  | { op: "AND" | "OR"; left: Expr; right: Expr }
  | { with: string; id: string };

/** Parse an SPDX licence expression (AND binds tighter than OR). */
export function parseExpression(src: string): Expr {
  const tokens = src.match(/\(|\)|[^\s()]+/g) ?? [];
  let i = 0;
  const peek = () => tokens[i];
  const next = () => tokens[i++];
  function atom(): Expr {
    const t = next();
    if (t === undefined) throw new Error(`incomplete expression "${src}"`);
    if (t === "(") {
      const e = or();
      if (next() !== ")") throw new Error(`unbalanced "(" in "${src}"`);
      return e;
    }
    if (/^(AND|OR|WITH|\))$/i.test(t))
      throw new Error(`unexpected "${t}" in "${src}"`);
    if (peek()?.toUpperCase() === "WITH") {
      next();
      const ex = next();
      if (!ex) throw new Error(`WITH without an exception in "${src}"`);
      return { id: t, with: ex };
    }
    return { id: t };
  }
  function and(): Expr {
    let left = atom();
    while (peek()?.toUpperCase() === "AND") {
      next();
      left = { op: "AND", left, right: atom() };
    }
    return left;
  }
  function or(): Expr {
    let left = and();
    while (peek()?.toUpperCase() === "OR") {
      next();
      left = { op: "OR", left, right: and() };
    }
    return left;
  }
  const e = or();
  if (i !== tokens.length) throw new Error(`trailing tokens in "${src}"`);
  return e;
}

function idAccepted(id: string, accepted: Set<string>): boolean {
  if (accepted.has(id)) return true;
  // `X+` means "X or later": fine when X-or-later is accepted.
  if (id.endsWith("+")) return accepted.has(`${id.slice(0, -1)}-or-later`);
  return false;
}

export function satisfies(expr: Expr, accepted: Set<string>): boolean {
  if ("op" in expr)
    return expr.op === "OR"
      ? satisfies(expr.left, accepted) || satisfies(expr.right, accepted)
      : satisfies(expr.left, accepted) && satisfies(expr.right, accepted);
  if ("with" in expr) return accepted.has(`${expr.id} WITH ${expr.with}`);
  return idAccepted(expr.id, accepted);
}

const FORBIDDEN_RE =
  /^(GPL-2\.0(-only)?|LGPL-2\.0(-only)?|UNLICENSED|SEE LICENSE IN.*|CC-BY-NC.*|BUSL-.*|SSPL-.*|Elastic-.*|Commons-Clause|PolyForm-.*|JSON)$/i;

/** Why a licence expression is not acceptable, or undefined when it is. */
export function licenceProblem(
  license: string | undefined,
  accepted: Set<string>,
): string | undefined {
  if (!license || !license.trim()) return "no licence declared (unlicensed)";
  if (FORBIDDEN_RE.test(license.trim()))
    return `forbidden licence "${license}" (GPL-2.0-only, unlicensed or non-free)`;
  let expr: Expr;
  try {
    expr = parseExpression(license);
  } catch (e) {
    return `not an SPDX expression: ${(e as Error).message}`;
  }
  if (!satisfies(expr, accepted))
    return `licence "${license}" is not satisfiable from the about.toml accepted list (add a docs/fork/LICENSE-REGISTER.md row first)`;
  return undefined;
}

// ---------------------------------------------------------------------------
// Packages

export interface LicenceText {
  file: string;
  text: string;
}

export interface Pkg {
  ecosystem: "rust" | "npm";
  name: string;
  version: string;
  license: string;
  repository?: string;
  author?: string;
  texts: LicenceText[];
  notices: LicenceText[];
}

export interface Problem {
  where: string;
  message: string;
}

const LICENCE_FILE_RE = /^(licen[cs]e|copying|unlicense)([._-].*)?$/i;
const NOTICE_FILE_RE = /^notices?(\.[a-z]+)?$/i;

function filesIn(dir: string, re: RegExp): LicenceText[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((d) => d.isFile() && re.test(d.name))
    .map((d) => d.name)
    .sort()
    .map((f) => ({ file: f, text: readFileSync(join(dir, f), "utf8") }));
}

function npmLicence(pj: Record<string, unknown>): string | undefined {
  const l = pj.license;
  if (typeof l === "string") return l;
  if (l && typeof l === "object" && "type" in l)
    return String((l as { type: unknown }).type);
  const ls = pj.licenses;
  if (Array.isArray(ls) && ls.length)
    return ls
      .map((x) => (typeof x === "string" ? x : String(x?.type ?? "")))
      .join(" OR ");
  return undefined;
}

function npmRepo(pj: Record<string, unknown>): string | undefined {
  const r = pj.repository;
  const url =
    typeof r === "string"
      ? r
      : r && typeof r === "object" && "url" in r
        ? String((r as { url: unknown }).url)
        : undefined;
  return url?.replace(/^git\+/, "").replace(/\.git$/, "");
}

function npmAuthor(pj: Record<string, unknown>): string | undefined {
  const a = pj.author;
  if (typeof a === "string") return a;
  if (a && typeof a === "object" && "name" in a)
    return String((a as { name: unknown }).name);
  return undefined;
}

/** Resolve `name` from `fromDir` the way Node does (walk up node_modules). */
function resolvePackage(
  root: string,
  name: string,
  fromDir: string,
): string | undefined {
  for (let d = fromDir; ; d = dirname(d)) {
    const candidate = join(d, "node_modules", name);
    if (existsSync(join(candidate, "package.json"))) return candidate;
    if (d === root || d === dirname(d)) return undefined;
  }
}

/** The npm runtime closure of `root/package.json` `dependencies`. */
export function collectNpm(root: string): { pkgs: Pkg[]; problems: Problem[] } {
  const problems: Problem[] = [];
  const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const seen = new Map<string, Pkg>();
  const walk = (name: string, from: string, optional: boolean) => {
    if (BUILD_ONLY_NPM[name] !== undefined) return;
    const dir = resolvePackage(root, name, from);
    if (!dir) {
      if (!optional)
        problems.push({
          where: `npm:${name}`,
          message: "dependency is not installed (run `bun install`)",
        });
      return;
    }
    if (seen.has(dir)) return;
    const pj = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    const license = (NPM_CLARIFY[pj.name] ?? npmLicence(pj) ?? "")
      .replace(/\s+/g, " ")
      .trim();
    seen.set(dir, {
      ecosystem: "npm",
      name: pj.name ?? name,
      version: pj.version ?? "?",
      license,
      repository: npmRepo(pj),
      author: npmAuthor(pj),
      texts: filesIn(dir, LICENCE_FILE_RE),
      notices: filesIn(dir, NOTICE_FILE_RE),
    });
    for (const d of Object.keys(pj.dependencies ?? {})) walk(d, dir, false);
    for (const d of Object.keys(pj.optionalDependencies ?? {}))
      walk(d, dir, true);
    for (const d of Object.keys(pj.peerDependencies ?? {})) walk(d, dir, true);
  };
  for (const d of Object.keys(pkg.dependencies ?? {})) walk(d, root, false);
  for (const name of Object.keys(BUILD_ONLY_NPM)) {
    if (!(name in (pkg.dependencies ?? {})))
      problems.push({
        where: `npm:${name}`,
        message:
          "listed in BUILD_ONLY_NPM but not a package.json dependency; remove the stale entry",
      });
  }
  // Nested copies of the same name@version are one package.
  const unique = new Map<string, Pkg>();
  for (const p of seen.values()) unique.set(`${p.name}@${p.version}`, p);
  return { pkgs: sortPkgs([...unique.values()]), problems };
}

function sortPkgs(p: Pkg[]): Pkg[] {
  return p.sort((a, b) =>
    a.name === b.name
      ? a.version.localeCompare(b.version, "en", { numeric: true })
      : a.name.localeCompare(b.name, "en"),
  );
}

// cargo-about JSON (template `{{{json this}}}`), only the fields used here.
interface AboutCrate {
  name: string;
  version: string;
  manifest_path: string;
  repository?: string | null;
  dependencies?: { name: string; kind: string | null }[];
}
export interface AboutJson {
  crates: { license: string; package: AboutCrate }[];
  licenses: {
    id: string;
    name: string;
    text: string;
    used_by: { crate: { name: string; version: string } }[];
  }[];
}

export interface RustData {
  pkgs: Pkg[];
  /** Licence texts with the crates that use them (cargo-about's view). */
  texts: { id: string; name: string; text: string; usedBy: string[] }[];
  dirs: Map<string, string>;
  native: Set<string>;
}

export function collectRust(about: AboutJson): RustData {
  const dirs = new Map<string, string>();
  const native = new Set<string>();
  const pkgs = sortPkgs(
    about.crates.map((c) => {
      const dir = dirname(c.package.manifest_path);
      dirs.set(c.package.name, dir);
      if (
        (c.package.dependencies ?? []).some(
          (d) => d.kind === "build" && (d.name === "cc" || d.name === "cmake"),
        )
      )
        native.add(c.package.name);
      return {
        ecosystem: "rust" as const,
        name: c.package.name,
        version: c.package.version,
        license: c.license.replace(/\s+/g, " ").trim(),
        repository: c.package.repository ?? undefined,
        texts: [],
        notices: filesIn(dir, NOTICE_FILE_RE),
      };
    }),
  );
  const byText = new Map<
    string,
    { id: string; name: string; text: string; usedBy: Set<string> }
  >();
  for (const l of about.licenses) {
    const key = `${l.id}\0${l.text}`;
    const e = byText.get(key) ?? {
      id: l.id,
      name: l.name,
      text: l.text,
      usedBy: new Set<string>(),
    };
    for (const u of l.used_by)
      e.usedBy.add(`${u.crate.name} ${u.crate.version}`);
    byText.set(key, e);
  }
  const texts = [...byText.values()]
    .map((e) => ({ ...e, usedBy: [...e.usedBy].sort() }))
    .sort(
      (a, b) =>
        a.id.localeCompare(b.id, "en") ||
        a.usedBy[0].localeCompare(b.usedBy[0], "en"),
    );
  return { pkgs, texts, dirs, native };
}

// ---------------------------------------------------------------------------
// NOTICE

/** Body of the NOTICE section whose heading is "<n>. <title>". */
export function noticeSection(notice: string, title: string): string {
  const lines = notice.split("\n");
  const start = lines.findIndex((l) =>
    new RegExp(`^\\d+\\. ${title}\\s*$`).test(l),
  );
  if (start < 0) throw new Error(`NOTICE has no "${title}" section`);
  let i = start + 1;
  if (/^=+$/.test(lines[i] ?? "")) i++;
  let end = i;
  while (
    end < lines.length &&
    !(/^=+$/.test(lines[end]) && /^\d+\. /.test(lines[end + 1] ?? ""))
  )
    end++;
  return lines.slice(i, end).join("\n").trim();
}

/** Package names listed in NOTICE §5 (`- <name>` lines). */
export function noticeApacheNames(notice: string): Set<string> {
  const body = noticeSection(notice, "Apache-2.0 NOTICE files");
  return new Set(
    [...body.matchAll(/^- (\S+)/gm)].map((m) => m[1].replace(/:$/, "")),
  );
}

interface CatalogModel {
  id: string;
  repository: string;
  license: string;
}

/** CC-BY catalog models (attribution required) missing from NOTICE §6. */
export function missingModelAttributions(
  notice: string,
  catalog: { models: CatalogModel[] },
): string[] {
  const body = noticeSection(notice, "Model attributions");
  return catalog.models
    .filter((m) => /^cc-by/i.test(m.license) && !/-nc/i.test(m.license))
    .filter((m) => !body.includes(m.repository))
    .map((m) => `${m.id} (${m.repository}, ${m.license})`);
}

// ---------------------------------------------------------------------------
// Rendering

function fence(text: string): string {
  const runs = text.match(/`{3,}/g) ?? [];
  const n = Math.max(3, ...runs.map((r) => r.length + 1));
  const f = "`".repeat(n);
  return `${f}text\n${text.replace(/\s+$/, "")}\n${f}`;
}

function cell(s: string | undefined): string {
  return (s ?? "").replace(/\|/g, "\\|").replace(/\n/g, " ");
}

export interface RenderInput {
  rust?: RustData;
  npm: Pkg[];
  native: { entry: NativeEntry; texts: Map<string, string> }[];
  notice: string;
}

export function render(input: RenderInput): string {
  const out: string[] = [];
  const push = (...l: string[]) => out.push(...l);
  push(
    "# Third-party notices",
    "",
    "<!-- Generated by scripts/wb-notices.ts. Do not edit; run `bun scripts/wb-notices.ts`. -->",
    "",
    "Wispr Bro is licensed under GPL-3.0 (`LICENSE`); its own notices, ported code and",
    "AGPL-marked files are in `NOTICE`. This file lists every third-party component",
    "in the distributed app with its licence text: speech models and fonts (from",
    "`NOTICE`), C/C++ compiled into Rust crates, Rust crates (cargo-about, macOS",
    "targets, build and dev dependencies excluded) and npm runtime packages.",
    "",
    "## Summary",
    "",
    "| Licence | Rust crates | npm packages |",
    "| --- | --- | --- |",
  );
  const count = new Map<string, [number, number]>();
  for (const p of [...(input.rust?.pkgs ?? []), ...input.npm]) {
    const c = count.get(p.license) ?? [0, 0];
    c[p.ecosystem === "rust" ? 0 : 1]++;
    count.set(p.license, c);
  }
  for (const [l, [r, n]] of [...count].sort((a, b) =>
    a[0].localeCompare(b[0], "en"),
  ))
    push(`| ${cell(l)} | ${r} | ${n} |`);
  push(
    "",
    `Total: ${input.rust?.pkgs.length ?? 0} Rust crates, ${input.npm.length} npm packages.`,
    "",
  );

  push(
    "## Speech models",
    "",
    "From `NOTICE` §6. Model weights are separate works downloaded at first run.",
    "",
    fence(noticeSection(input.notice, "Model attributions")),
    "",
    "## Fonts",
    "",
    "From `NOTICE` §7.",
    "",
    fence(noticeSection(input.notice, "Fonts")),
    "",
  );

  push("## C/C++ compiled into Rust crates", "");
  for (const { entry, texts } of input.native) {
    const v = input.rust?.pkgs.find((p) => p.name === entry.crate)?.version;
    push(`### ${entry.crate}${v ? ` ${v}` : ""}`, "", entry.how, "");
    for (const c of entry.components) {
      push(`- **${c.name}**: ${c.licence}. ${c.copyright}.`);
      if (c.note) push(`  ${c.note}`);
    }
    push("");
    for (const c of entry.components) {
      if (!c.file) continue;
      push(`#### ${c.name}: \`${c.file}\``, "", fence(texts.get(c.file)!), "");
    }
  }
  for (const n of NATIVE_NOTES) push(`- ${n}`);
  push("");

  const notices = [...(input.rust?.pkgs ?? []), ...input.npm].filter(
    (p) => p.notices.length,
  );
  push("## NOTICE files", "");
  if (!notices.length)
    push("No shipped Rust crate or npm package has a NOTICE file.", "");
  for (const p of notices)
    for (const n of p.notices)
      push(`### ${p.name} ${p.version}: ${n.file}`, "", fence(n.text), "");

  push("## Rust crates", "");
  if (!input.rust) {
    push("_Not generated: cargo-about was not run (--skip-rust)._", "");
  } else {
    push(
      "| Crate | Version | Licence | Repository |",
      "| --- | --- | --- | --- |",
    );
    for (const p of input.rust.pkgs)
      push(
        `| ${cell(p.name)} | ${cell(p.version)} | ${cell(p.license)} | ${cell(p.repository)} |`,
      );
    push("", "### Rust licence texts", "");
    for (const t of input.rust.texts) {
      push(
        `#### ${t.name} (${t.id})`,
        "",
        `Used by: ${t.usedBy.join(", ")}`,
        "",
        fence(t.text),
        "",
      );
    }
  }

  push(
    "## npm packages",
    "",
    "| Package | Version | Licence | Repository |",
    "| --- | --- | --- | --- |",
  );
  for (const p of input.npm)
    push(
      `| ${cell(p.name)} | ${cell(p.version)} | ${cell(p.license)} | ${cell(p.repository)} |`,
    );
  push("", "### npm licence texts", "");
  const byText = new Map<string, { file: string; users: string[] }>();
  const noFile: Pkg[] = [];
  for (const p of input.npm) {
    if (!p.texts.length) noFile.push(p);
    for (const t of p.texts) {
      const e = byText.get(t.text) ?? { file: t.file, users: [] };
      e.users.push(`${p.name} ${p.version}`);
      byText.set(t.text, e);
    }
  }
  for (const [text, e] of [...byText].sort((a, b) =>
    a[1].users[0].localeCompare(b[1].users[0], "en"),
  ))
    push(`Used by: ${e.users.join(", ")}`, "", fence(text), "");
  if (noFile.length) {
    push(
      "#### Packages without a licence file",
      "",
      "These packages declare their licence in package.json but ship no licence",
      "file; the standard text of the declared licence applies.",
      "",
    );
    for (const p of noFile)
      push(
        `- ${p.name} ${p.version}: ${p.license}${p.author ? `, ${p.author}` : ""}`,
      );
    push("");
  }
  return out.join("\n");
}

// ---------------------------------------------------------------------------
// Running

export interface GenerateOptions {
  root?: string;
  about?: AboutJson;
  skipRust?: boolean;
}

export interface GenerateResult {
  markdown: string;
  problems: Problem[];
  warnings: string[];
  licences: Map<string, number>;
}

export function generate(opts: GenerateOptions): GenerateResult {
  const root = opts.root ?? REPO_ROOT;
  const problems: Problem[] = [];
  const warnings: string[] = [];
  const accepted = new Set(
    parseAccepted(readFileSync(join(root, "about.toml"), "utf8")),
  );
  const notice = readFileSync(join(root, "NOTICE"), "utf8");

  const npm = collectNpm(root);
  problems.push(...npm.problems);
  const rust = opts.about ? collectRust(opts.about) : undefined;
  if (!rust && !opts.skipRust)
    throw new Error("no cargo-about data (pass about or skipRust)");

  for (const p of [...(rust?.pkgs ?? []), ...npm.pkgs]) {
    const why = licenceProblem(p.license, accepted);
    if (why)
      problems.push({
        where: `${p.ecosystem}:${p.name} ${p.version}`,
        message: why,
      });
  }
  for (const p of npm.pkgs)
    if (!p.texts.length)
      warnings.push(
        `npm:${p.name} ${p.version} has no licence file (declares ${p.license})`,
      );

  // C/C++ list.
  const native: RenderInput["native"] = [];
  for (const entry of NATIVE) {
    const texts = new Map<string, string>();
    const dir = rust?.dirs.get(entry.crate);
    if (rust && !dir) {
      problems.push({
        where: `native:${entry.crate}`,
        message:
          "listed in NATIVE (scripts/wb-notices.ts) but no longer shipped; remove or update the entry",
      });
      continue;
    }
    for (const c of entry.components) {
      if (!c.file) continue;
      const path = dir ? join(dir, c.file) : undefined;
      if (path && existsSync(path))
        texts.set(c.file, readFileSync(path, "utf8"));
      else if (rust)
        problems.push({
          where: `native:${entry.crate}`,
          message: `licence file ${c.file} not found in the crate`,
        });
      else texts.set(c.file, "(crate source not read: --skip-rust)");
    }
    native.push({ entry, texts });
  }
  if (rust) {
    const listed = new Set(NATIVE.map((n) => n.crate));
    for (const name of rust.native)
      if (!listed.has(name))
        problems.push({
          where: `native:${name}`,
          message:
            "crate compiles native code (cc/cmake build dependency) but is not in NATIVE (scripts/wb-notices.ts); add its vendored C/C++ with licences",
        });
  }

  // NOTICE checks.
  try {
    const catalog = JSON.parse(
      readFileSync(join(root, "src-tauri/src/catalog/catalog.json"), "utf8"),
    );
    for (const m of missingModelAttributions(notice, catalog))
      problems.push({
        where: "NOTICE §6",
        message: `CC-BY catalog model without an attribution: ${m}`,
      });
    const apache = noticeApacheNames(notice);
    for (const p of [...(rust?.pkgs ?? []), ...npm.pkgs])
      if (p.notices.length && !apache.has(p.name))
        problems.push({
          where: `${p.ecosystem}:${p.name} ${p.version}`,
          message: `ships ${p.notices.map((n) => n.file).join(", ")}; list "- ${p.name}" in NOTICE §5 (Apache-2.0 §4(d))`,
        });
    noticeSection(notice, "Fonts");
  } catch (e) {
    problems.push({ where: "NOTICE", message: (e as Error).message });
  }

  const licences = new Map<string, number>();
  for (const p of [...(rust?.pkgs ?? []), ...npm.pkgs])
    licences.set(p.license, (licences.get(p.license) ?? 0) + 1);
  return {
    markdown: render({ rust, npm: npm.pkgs, native, notice }) + "\n",
    problems,
    warnings,
    licences,
  };
}

/** cargo-about command line, or undefined when it is not installed. */
export function findCargoAbout(): string[] | undefined {
  const env = process.env.CARGO_ABOUT;
  if (env) return [env, "about"];
  const bin = Bun.which("cargo-about");
  if (bin) return [bin, "about"];
  return undefined;
}

export function runCargoAbout(root: string, cmd: string[]): AboutJson {
  const tmp = mkdtempSync(join(tmpdir(), "wb-notices-"));
  try {
    const tmpl = join(tmp, "json.hbs");
    const out = join(tmp, "about.json");
    writeFileSync(tmpl, "{{{json this}}}\n");
    const res = spawnSync(
      cmd[0],
      [
        ...cmd.slice(1),
        "generate",
        "--fail",
        "--manifest-path",
        join(root, "src-tauri/Cargo.toml"),
        "-c",
        join(root, "about.toml"),
        tmpl,
        "-o",
        out,
      ],
      { cwd: root, encoding: "utf8", stdio: ["ignore", "inherit", "pipe"] },
    );
    if (res.status !== 0)
      throw new Error(
        `cargo-about failed (exit ${res.status}): ${(res.stderr ?? "").trim().split("\n").slice(-20).join("\n")}`,
      );
    return JSON.parse(readFileSync(out, "utf8"));
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function main(argv: string[]): number {
  const root = REPO_ROOT;
  const check = argv.includes("--check");
  const skipRust = argv.includes("--skip-rust");
  const aboutPath = argValue(argv, "--about-json");
  const outArg = argValue(argv, "--out");
  if (skipRust && !outArg && !check) {
    console.error(
      `--skip-rust output is incomplete; pass --out <path> (it never overwrites ${OUT_FILE})`,
    );
    return 1;
  }
  let about: AboutJson | undefined;
  if (aboutPath) about = JSON.parse(readFileSync(aboutPath, "utf8"));
  else if (!skipRust) {
    const cmd = findCargoAbout();
    if (!cmd) {
      console.error(
        [
          "cargo-about is not installed. It is a dev tool, not a project dependency.",
          `Install it with:\n  ${CARGO_ABOUT_INSTALL}`,
          "or point CARGO_ABOUT at a cargo-about binary, then rerun.",
          "To test only the npm and manual parts: bun scripts/wb-notices.ts --skip-rust --out <path>",
        ].join("\n"),
      );
      return 2;
    }
    about = runCargoAbout(root, cmd);
  }
  const res = generate({ root, about, skipRust });
  for (const w of res.warnings) console.log(`  note: ${w}`);
  console.log("Licences found:");
  for (const [l, n] of [...res.licences].sort((a, b) => b[1] - a[1]))
    console.log(`  ${String(n).padStart(4)}  ${l}`);
  if (res.problems.length) {
    console.error(`\n✗ ${res.problems.length} licence problem(s):`);
    for (const p of res.problems) console.error(`  ${p.where}: ${p.message}`);
    return 1;
  }
  const outPath = resolve(root, outArg ?? OUT_FILE);
  if (check) {
    if (skipRust) {
      console.log("✓ npm, manual and NOTICE checks pass (Rust skipped)");
      return 0;
    }
    const current = existsSync(outPath) ? readFileSync(outPath, "utf8") : "";
    if (current !== res.markdown) {
      console.error(
        `✗ ${outArg ?? OUT_FILE} is stale: run \`bun scripts/wb-notices.ts\` and commit the result`,
      );
      return 1;
    }
    console.log(`✓ ${outArg ?? OUT_FILE} is current`);
    return 0;
  }
  writeFileSync(outPath, res.markdown);
  console.log(`✓ wrote ${outArg ?? OUT_FILE}`);
  return 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
