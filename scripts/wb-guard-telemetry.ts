// SPDX-License-Identifier: GPL-3.0-or-later
//
// Telemetry guard (R5 §1, §4.12; GPL addendum §4 analytics SDK list).
//
// `ci/telemetry-packages.txt` lists exact package names or globs (`sentry-*`,
// `@sentry/*`, `mixpanel*`). A name matches only when the WHOLE package name
// matches, never a substring, so `unicode-segmentation` passes while
// `@segment/analytics-next` fails. Names are collected from:
//   - every `Cargo.lock` (`name = "…"`) and every `Cargo.toml` dependency key;
//   - `bun.lock` (workspace dependencies and package entries) and every
//     `package.json` dependency key;
//   - import specifiers in TS/JS (`from "x"`, `import("x")`, `require("x")`)
//     and crate roots in Rust (`use x::`, `extern crate x`).
// Rust crate names compare with `_` and `-` treated as equal.

import {
  cliMain,
  globToRegExp,
  listFiles,
  newResult,
  readIfExists,
  readList,
  REPO_ROOT,
  stripRustComments,
  type GuardResult,
} from "./wb-guard-lib";

export interface PackageRef {
  name: string;
  file: string;
  line?: number;
  via: string;
}

export function compilePatterns(patterns: string[]): RegExp[] {
  // Package names contain `/` (scopes), so `*` must not stop at `/` for
  // `@sentry/*`; globToRegExp's `*` is segment-local, which is what we want
  // for `@scope/*` (one level) and for `sentry-*` (no slash in crate names).
  return patterns.map((p) => globToRegExp(p.toLowerCase()));
}

export function isTelemetryPackage(name: string, res: RegExp[]): boolean {
  const n = name.toLowerCase();
  const alt = n.replace(/_/g, "-");
  return res.some((re) => re.test(n) || re.test(alt));
}

/** `@scope/name/sub` → `@scope/name`; `name/sub` → `name`; relative → null. */
export function packageOfSpecifier(spec: string): string | null {
  if (/^(\.|\/|@\/|~\/|node:|bun:|https?:|data:|virtual:|\$)/.test(spec))
    return null;
  const parts = spec.split("/");
  if (spec.startsWith("@"))
    return parts.length >= 2 ? `${parts[0]}/${parts[1]}` : null;
  return parts[0] || null;
}

function lineAt(text: string, idx: number): number {
  return text.slice(0, idx).split("\n").length;
}

export function collectPackages(root: string): PackageRef[] {
  const refs: PackageRef[] = [];
  const add = (name: string, file: string, via: string, line?: number) =>
    refs.push({ name, file, via, line });

  for (const f of listFiles(root)) {
    const base = f.split("/").pop() ?? f;
    const ext = base.split(".").pop() ?? "";
    if (base === "Cargo.lock") {
      const t = readIfExists(root, f) ?? "";
      for (const m of t.matchAll(/^name\s*=\s*"([^"]+)"/gm))
        add(m[1], f, "Cargo.lock", lineAt(t, m.index ?? 0));
    } else if (base === "Cargo.toml") {
      const t = readIfExists(root, f) ?? "";
      let inDeps = false;
      t.split("\n").forEach((l, i) => {
        const sec = l.match(/^\s*\[([^\]]+)\]/);
        if (sec) {
          const s = sec[1].trim();
          inDeps = /(^|\.)(dev-|build-)?dependencies$/.test(s);
          const table = s.match(
            /(?:^|\.)(?:dev-|build-)?dependencies\.([A-Za-z0-9_-]+)$/,
          );
          if (table) add(table[1], f, "Cargo.toml", i + 1);
          return;
        }
        const dep = inDeps && l.match(/^\s*([A-Za-z0-9_-]+)\s*=/);
        if (dep) add(dep[1], f, "Cargo.toml", i + 1);
        const pkgRename = inDeps && l.match(/\bpackage\s*=\s*"([^"]+)"/);
        if (pkgRename) add(pkgRename[1], f, "Cargo.toml", i + 1);
      });
    } else if (base === "bun.lock") {
      const t = readIfExists(root, f) ?? "";
      // Package entries: `"key": ["name@version", …`.
      for (const m of t.matchAll(/^\s*"([^"]+)":\s*\["(@?[^@"]+)@/gm)) {
        add(m[2], f, "bun.lock", lineAt(t, m.index ?? 0));
      }
      // Workspace dependency maps: `"name": "^1.2.3",`.
      for (const m of t.matchAll(
        /^\s*"(@?[a-z0-9][\w.-]*(?:\/[\w.-]+)?)":\s*"[^"]*",?$/gim,
      ))
        add(m[1], f, "bun.lock", lineAt(t, m.index ?? 0));
    } else if (base === "package.json") {
      const t = readIfExists(root, f) ?? "";
      try {
        const pkg = JSON.parse(t) as Record<string, unknown>;
        for (const k of [
          "dependencies",
          "devDependencies",
          "optionalDependencies",
          "peerDependencies",
        ]) {
          const deps = pkg[k];
          if (deps && typeof deps === "object")
            for (const name of Object.keys(deps))
              add(name, f, `package.json ${k}`);
        }
      } catch {
        // Fixture or broken JSON; other checks report it.
      }
    } else if (/^(ts|tsx|js|jsx|mjs|cjs|mts|cts)$/.test(ext)) {
      const t = readIfExists(root, f) ?? "";
      const re =
        /(?:\bfrom\s*|\bimport\s*\(\s*|\brequire\s*\(\s*|^\s*import\s+)["']([^"'\n]+)["']/gm;
      for (const m of t.matchAll(re)) {
        const p = packageOfSpecifier(m[1]);
        if (p) add(p, f, "import", lineAt(t, m.index ?? 0));
      }
    } else if (ext === "rs") {
      const t = stripRustComments(readIfExists(root, f) ?? "");
      for (const m of t.matchAll(
        /\b(?:use\s+(?:::)?|extern\s+crate\s+)([A-Za-z_][A-Za-z0-9_]*)/g,
      ))
        add(m[1], f, "rust use", lineAt(t, m.index ?? 0));
    }
  }
  return refs;
}

export function runTelemetry(opts: { root?: string } = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const r = newResult("telemetry");
  let patterns: string[];
  try {
    patterns = readList(root, "ci/telemetry-packages.txt").map((e) => e.value);
  } catch (e) {
    r.violations.push({
      file: "ci/telemetry-packages.txt",
      rule: "telemetry/list",
      message: (e as Error).message,
    });
    return r;
  }
  const res = compilePatterns(patterns);
  const refs = collectPackages(root);
  const seen = new Set<string>();
  for (const ref of refs) {
    if (!isTelemetryPackage(ref.name, res)) continue;
    const key = `${ref.file}:${ref.line}:${ref.name}`;
    if (seen.has(key)) continue;
    seen.add(key);
    r.violations.push({
      file: ref.file,
      line: ref.line,
      rule: "telemetry/package",
      message: `telemetry/analytics package \`${ref.name}\` (${ref.via}) is banned (ci/telemetry-packages.txt, R5 §1)`,
    });
  }
  r.notes.push(
    `checked ${new Set(refs.map((x) => x.name)).size} distinct package name(s) against ${patterns.length} pattern(s)`,
  );
  return r;
}

if (import.meta.main) cliMain(() => runTelemetry());
