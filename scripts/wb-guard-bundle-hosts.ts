// SPDX-License-Identifier: GPL-3.0-or-later
//
// Built-bundle host guard (R5 §4.12 "Network: built bundle").
//
// Scans the frontend build (`dist/**/*.{js,css,html}`) and the printable
// strings of each Mach-O binary passed with --binary (repeatable; an `.app`
// path scans every file in `Contents/MacOS/`). Every URL host must be in
// `ci/allowed-hosts.txt` (any section) or `ci/inert-hosts.txt`.
//
// Skips with a message when there is no `dist/` and no --binary: CI runs it
// after `wb:build` (wb-egress.yml); `bun run wb:guards` on a fresh checkout
// skips it.
//
// Usage: bun scripts/wb-guard-bundle-hosts.ts [--dist dist] [--binary <path>]…

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative, resolve } from "node:path";
import {
  argValue,
  cliMain,
  findUrls,
  hostInList,
  isLoopbackHost,
  newResult,
  REPO_ROOT,
  toPosix,
  type GuardResult,
} from "./wb-guard-lib";
import { readHostLists } from "./wb-guard-hosts";

/** Printable ASCII runs of at least `min` bytes (like `strings -a -n min`). */
export function printableStrings(buf: Uint8Array, min = 8): string[] {
  const out: string[] = [];
  let start = -1;
  for (let i = 0; i <= buf.length; i++) {
    const b = i < buf.length ? buf[i] : 0;
    const printable = b >= 0x20 && b < 0x7f;
    if (printable) {
      if (start < 0) start = i;
    } else if (start >= 0) {
      if (i - start >= min)
        out.push(Buffer.from(buf.subarray(start, i)).toString("latin1"));
      start = -1;
    }
  }
  return out;
}

function walk(dir: string, exts: RegExp, acc: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) walk(full, exts, acc);
    else if (exts.test(name)) acc.push(full);
  }
  return acc;
}

function binariesFor(path: string): string[] {
  if (path.endsWith(".app") && statSync(path).isDirectory()) {
    const macos = join(path, "Contents", "MacOS");
    return existsSync(macos)
      ? readdirSync(macos).map((n) => join(macos, n))
      : [];
  }
  return [path];
}

// Common generic TLDs; any two-letter label is accepted as a ccTLD.
const GENERIC_TLDS = new Set(
  (
    "com org net edu gov mil int io ai co dev app rs aws computer cloud info " +
    "biz xyz online site tech page link zip mov top club live news blog store " +
    "shop me tv cc ly gg sh to ws fm im is la tk"
  ).split(" "),
);

/**
 * Rust packs string literals back to back, so a binary's printable runs glue
 * a URL to the next literal (`https://blob.handy.computertranslation…`).
 * A host that starts with a known host followed by letters is trimmed back
 * to that host; then a host whose last label is not a plausible TLD
 * (compressed-asset noise such as `www.years`) is dropped (undefined).
 */
export function normalizePackedHost(
  host: string,
  known: string[],
): string | undefined {
  for (const k of known) {
    if (
      host.length > k.length &&
      host.startsWith(k) &&
      /[a-z]/.test(host[k.length])
    )
      return k;
  }
  if (host.startsWith("[") || /^[\d.]+$/.test(host)) return host;
  const tld = host.split(".").pop() ?? "";
  if (/^[a-z]{2}$/.test(tld) || GENERIC_TLDS.has(tld)) return host;
  return undefined;
}

export interface BundleOptions {
  root?: string;
  dist?: string;
  binaries?: string[];
}

export function runBundleHosts(opts: BundleOptions = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const r = newResult("bundle-hosts");
  const { lists, problems } = readHostLists(root);
  for (const p of problems) r.violations.push({ ...p, rule: "hosts/list" });
  const ok = [
    ...lists.network,
    ...lists.link,
    ...lists.schema,
    ...lists.loopback,
    ...lists.inert,
  ];

  const distDir = resolve(root, opts.dist ?? "dist");
  const binaries = (opts.binaries ?? []).flatMap((b) =>
    binariesFor(resolve(root, b)),
  );
  const haveDist = existsSync(distDir) && statSync(distDir).isDirectory();
  if (!haveDist && binaries.length === 0) {
    r.skipped =
      "no build: dist/ not found and no --binary given (run `bun run wb:build` first; wb-egress.yml runs this after the release build)";
    return r;
  }

  const known = [...ok, ...lists.baseline].filter((k) => !k.startsWith("*."));
  const report = (file: string, text: string, packed: boolean) => {
    const seen = new Set<string>();
    for (const hit of findUrls(text)) {
      const h = packed ? normalizePackedHost(hit.host, known) : hit.host;
      if (h === undefined) continue;
      if (!h.includes(".") && !isLoopbackHost(h)) continue; // single-label noise
      if (hostInList(h, ok) || isLoopbackHost(h)) continue;
      if (seen.has(h)) continue;
      seen.add(h);
      r.violations.push({
        file,
        line: hit.line,
        rule: "hosts/bundle",
        message: `host ${h} (${hit.url.slice(0, 100)}) is in the build but not in ci/allowed-hosts.txt or ci/inert-hosts.txt`,
      });
    }
  };

  if (haveDist) {
    const files = walk(distDir, /\.(m?js|css|html)$/);
    for (const f of files) {
      const rel = relative(root, f);
      const shown = rel.startsWith("..") ? f : rel;
      report(toPosix(shown), readFileSync(f, "utf8"), false);
    }
    r.notes.push(`scanned ${files.length} file(s) in ${toPosix(distDir)}`);
  } else {
    r.notes.push("dist/ not found: frontend bundle not scanned");
  }
  if (binaries.length === 0) {
    r.notes.push("no --binary given: Mach-O strings not scanned");
  }
  for (const b of binaries) {
    if (!existsSync(b)) {
      r.violations.push({
        file: b,
        rule: "hosts/bundle",
        message: "binary not found",
      });
      continue;
    }
    const strings = printableStrings(new Uint8Array(readFileSync(b)));
    // One string per line so `line` is the string index.
    report(`${basename(b)} (strings)`, strings.join("\n"), true);
    r.notes.push(`scanned ${strings.length} string(s) in ${b}`);
  }
  return r;
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const binaries: string[] = [];
  argv.forEach((a, i) => {
    if (a === "--binary" && argv[i + 1]) binaries.push(argv[i + 1]);
    else if (a.startsWith("--binary=")) binaries.push(a.slice(9));
  });
  cliMain(() => runBundleHosts({ dist: argValue(argv, "--dist"), binaries }));
}
