// SPDX-License-Identifier: GPL-3.0-or-later
//
// Network host guard for source code (R5 §1 privacy promise, §4.12).
//
// Fork code (src/fork, src-tauri/src/fork, src-tauri/crates/wb-*, scripts/wb-*,
// src-tauri/tauri.fork.conf.json and the fork locale files): every URL host
// must be in `ci/allowed-hosts.txt`:
//   [network]  hosts the app may contact (§1), allowed anywhere;
//   [link]     hosts allowed only as an `openUrl(…)` / `open_url(…)` argument
//              (a link the user clicks opens in the browser);
//   [schema]   hosts allowed only as a JSON `"$schema"` value (editor hint);
//   [loopback] local hosts, never network egress;
//   [test]     hosts allowed only in fork *test code* (see isForkTestFile and
//              forkTestSpans): example names, private/loopback addresses and
//              real hosts a test names to prove the app refuses them. Each
//              entry's comment starts with its category (`example:`,
//              `private:`, `loopback:` or `refusal:`); `example:` and
//              `private:`/`loopback:` entries are checked against RFC 2606 /
//              6761 names and the private/loopback ranges. A [test] host
//              anywhere else in fork code fails (`hosts/test-only`); it is
//              never accepted by the upstream or built-bundle scans.
//
// Fork test code is:
//   - whole files: `scripts/wb-*.test.ts`, `tests/fork/**`, and fork Rust
//     files named `tests.rs` / `testutil.rs` whose parent module declares
//     them with exactly `#[cfg(test)] mod <name>;`;
//   - in other fork Rust files: the body of a `#[cfg(test)] mod <name> { … }`
//     block (extent by brace matching on a real token stream, so braces in
//     strings and comments do not count). If a file does not lex, none of it
//     counts as test code.
//   Comments of production fork code are NOT test code: describe a refused
//   host in words there (F-04 tightening), so a URL pasted into a doc
//   comment cannot hide a [test] host next to production code.
//
// Upstream code (the rest of src/, src-tauri/src/, src-tauri/*.json): every
// host must be in `ci/upstream-hosts-baseline.txt` (reviewed), in
// `ci/inert-hosts.txt` (proven unreachable) or in the [network]/[loopback]
// sections of `ci/allowed-hosts.txt`. A new upstream host fails until it is
// reviewed (R5 §4.12).

import {
  cliMain,
  findUrls,
  inSpans,
  lexRust,
  rustCfgTestMods,
  type Span,
  hostInList,
  isForkCodePath,
  isTextFile,
  listFiles,
  newResult,
  readIfExists,
  readList,
  REPO_ROOT,
  type GuardResult,
  type ListEntry,
} from "./wb-guard-lib";

export interface HostLists {
  network: string[];
  link: string[];
  schema: string[];
  loopback: string[];
  test: string[];
  baseline: string[];
  inert: string[];
}

const ALLOWED_SECTIONS = new Set([
  "network",
  "link",
  "schema",
  "loopback",
  "test",
]);
const INERT_SECTIONS: Record<string, string> = {
  identifier: "reason:",
  proven: "proof:",
  link: "source:",
};

const TEST_CATEGORIES = ["example", "private", "loopback", "refusal"] as const;

/** RFC 2606 / RFC 6761 example and special-use names (and subdomains). */
export function isExampleName(host: string): boolean {
  const h = host.replace(/^\*\./, "x.");
  return (
    /(^|\.)example\.(com|net|org)$/.test(h) ||
    /(^|\.)(example|test|invalid|localhost)$/.test(h)
  );
}

/** IPv4 as a 32-bit number: dotted quad, or one hex / decimal integer. */
function ipv4Value(host: string): number | undefined {
  if (/^0x[0-9a-f]{1,8}$/i.test(host)) return parseInt(host, 16);
  if (/^\d{1,10}$/.test(host)) {
    const v = Number(host);
    return v <= 0xffffffff ? v : undefined;
  }
  const m = host.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (!m) return undefined;
  const o = m.slice(1).map(Number);
  if (o.some((x) => x > 255)) return undefined;
  return ((o[0] << 24) >>> 0) + (o[1] << 16) + (o[2] << 8) + o[3];
}

/** 127.0.0.0/8 in any IPv4 spelling, or [::1]. */
export function isLoopbackAddress(host: string): boolean {
  if (host === "[::1]") return true;
  const v = ipv4Value(host);
  return v !== undefined && v >>> 24 === 127;
}

/** RFC 1918 and link-local (169.254/16) IPv4. */
export function isPrivateAddress(host: string): boolean {
  const v = ipv4Value(host);
  if (v === undefined) return false;
  const a = v >>> 24;
  const b = (v >>> 16) & 0xff;
  return (
    a === 10 ||
    (a === 172 && b >= 16 && b < 32) ||
    (a === 192 && b === 168) ||
    (a === 169 && b === 254)
  );
}

function testEntryProblem(e: ListEntry): string | undefined {
  const cat = TEST_CATEGORIES.find((c) => e.comment.startsWith(`${c}:`));
  if (!cat || e.comment.length <= cat.length + 2) {
    return `${e.value}: a [test] entry needs a comment "<${TEST_CATEGORIES.join("|")}>: <why>"`;
  }
  const h = e.value.toLowerCase();
  if (cat === "example" && !isExampleName(h))
    return `${e.value} is not an RFC 2606/6761 example name`;
  if (cat === "private" && !isPrivateAddress(h))
    return `${e.value} is not a private (RFC 1918 / link-local) IPv4 address`;
  if (cat === "loopback" && !isLoopbackAddress(h))
    return `${e.value} is not a loopback address`;
  if (cat === "refusal" && h.startsWith("*."))
    return `${e.value}: [test] refusal entries name one host, no wildcard`;
  return undefined;
}

export function readHostLists(root: string): {
  lists: HostLists;
  problems: { file: string; line: number; message: string }[];
} {
  const problems: { file: string; line: number; message: string }[] = [];
  const allowed = readList(root, "ci/allowed-hosts.txt");
  const pick = (s: string) =>
    allowed.filter((e) => e.section === s).map((e) => e.value.toLowerCase());
  for (const e of allowed) {
    if (!ALLOWED_SECTIONS.has(e.section)) {
      problems.push({
        file: "ci/allowed-hosts.txt",
        line: e.line,
        message: `${e.value} is outside a [network]/[link]/[schema]/[loopback]/[test] section`,
      });
    } else if (e.section === "test") {
      const p = testEntryProblem(e);
      if (p)
        problems.push({
          file: "ci/allowed-hosts.txt",
          line: e.line,
          message: p,
        });
    }
  }
  const inert = readList(root, "ci/inert-hosts.txt");
  for (const e of inert) {
    const prefix = INERT_SECTIONS[e.section];
    if (!prefix) {
      problems.push({
        file: "ci/inert-hosts.txt",
        line: e.line,
        message: `${e.value} is outside an [identifier]/[proven]/[link] section`,
      });
    } else if (
      !e.comment.startsWith(prefix) ||
      e.comment.length <= prefix.length + 1
    ) {
      problems.push({
        file: "ci/inert-hosts.txt",
        line: e.line,
        message: `${e.value} needs a comment "${prefix} …" naming the test (or reason, or link source) that proves it unreachable`,
      });
    }
  }
  const baseline = readList(root, "ci/upstream-hosts-baseline.txt");
  for (const e of baseline) {
    if (!e.comment) {
      problems.push({
        file: "ci/upstream-hosts-baseline.txt",
        line: e.line,
        message: `${e.value} needs a review comment (what it is, where it is used)`,
      });
    }
  }
  const values = (l: ListEntry[]) => l.map((e) => e.value.toLowerCase());
  return {
    lists: {
      network: pick("network"),
      link: pick("link"),
      schema: pick("schema"),
      loopback: pick("loopback"),
      test: pick("test"),
      baseline: values(baseline),
      inert: values(inert),
    },
    problems,
  };
}

export function isForkHostScope(rel: string): boolean {
  return (
    isForkCodePath(rel) ||
    rel === "src-tauri/tauri.fork.conf.json" ||
    rel === "src-tauri/tauri.publik.conf.json" ||
    /^src\/i18n\/locales\/[^/]+\/fork(\.flags)?\.json$/.test(rel)
  );
}

export function isUpstreamHostScope(rel: string): boolean {
  if (isForkHostScope(rel)) return false;
  if (rel.startsWith("src/") || rel.startsWith("src-tauri/src/")) return true;
  return /^src-tauri\/[^/]+\.json$/.test(rel);
}

/** Rust test files: `tests.rs` / `testutil.rs` declared `#[cfg(test)]`. */
const RUST_TEST_FILE_RE =
  /^src-tauri\/(?:src\/fork|crates\/wb-[^/]+)\/(?:.*\/)?(tests|testutil)\.rs$/;

/** Parent-module candidates of `dir/name.rs` (Rust 2018 module layout). */
function parentModules(rel: string): string[] {
  const dir = rel.slice(0, rel.lastIndexOf("/"));
  return [`${dir}/mod.rs`, `${dir}.rs`, `${dir}/lib.rs`, `${dir}/main.rs`];
}

/** Whole-file fork test code (see the header). */
export function isForkTestFile(root: string, rel: string): boolean {
  if (/^scripts\/wb-[^/]+\.test\.ts$/.test(rel)) return true;
  if (rel.startsWith("tests/fork/")) return true;
  const m = rel.match(RUST_TEST_FILE_RE);
  if (!m) return false;
  return parentModules(rel).some((p) => {
    const src = readIfExists(root, p);
    return (
      src !== undefined &&
      rustCfgTestMods(src).some((x) => x.kind === "decl" && x.name === m[1])
    );
  });
}

/** Test-only spans of a fork Rust file: `#[cfg(test)] mod … { }` bodies. */
export function forkTestSpans(rel: string, text: string): Span[] {
  if (!rel.endsWith(".rs")) return [];
  if (!lexRust(text).ok) return [];
  return rustCfgTestMods(text)
    .filter((x) => x.kind === "inline")
    .map((x) => x.span);
}

const OPEN_URL_RE = /\bopen_?url\s*\(|\bopenUrl\b|\bopener::open_url\b/i;

/** A `[link]` host is fine when the URL sits inside an openUrl call. */
function inOpenUrlContext(lines: string[], line: number): boolean {
  for (let l = Math.max(0, line - 3); l < line; l++) {
    if (OPEN_URL_RE.test(lines[l])) return true;
  }
  return false;
}

export function runHosts(opts: { root?: string } = {}): GuardResult {
  const root = opts.root ?? REPO_ROOT;
  const r = newResult("hosts");
  let lists: HostLists;
  try {
    const read = readHostLists(root);
    lists = read.lists;
    for (const p of read.problems)
      r.violations.push({ ...p, rule: "hosts/list" });
  } catch (e) {
    r.violations.push({
      file: "ci/",
      rule: "hosts/list",
      message: (e as Error).message,
    });
    return r;
  }

  const upstreamOk = [
    ...lists.baseline,
    ...lists.inert,
    ...lists.network,
    ...lists.loopback,
  ];
  let forkFiles = 0;
  let upstreamFiles = 0;
  const upstreamHosts = new Set<string>();
  for (const f of listFiles(root)) {
    if (!isTextFile(f)) continue;
    const fork = isForkHostScope(f);
    if (!fork && !isUpstreamHostScope(f)) continue;
    const text = readIfExists(root, f) ?? "";
    const lines = text.split("\n");
    if (fork) forkFiles++;
    else upstreamFiles++;
    let testFile: boolean | undefined;
    let testSpans: Span[] | undefined;
    const inTestCode = (index: number): boolean => {
      testFile ??= isForkTestFile(root, f);
      if (testFile) return true;
      testSpans ??= forkTestSpans(f, text);
      return inSpans(index, testSpans);
    };
    for (const hit of findUrls(text)) {
      const h = hit.host;
      if (fork) {
        if (hostInList(h, lists.network) || hostInList(h, lists.loopback))
          continue;
        if (hostInList(h, lists.link)) {
          if (inOpenUrlContext(lines, hit.line)) continue;
          r.violations.push({
            file: f,
            line: hit.line,
            rule: "hosts/link-only",
            message: `${h} is a [link] host: it may appear only as an openUrl(…) argument`,
          });
          continue;
        }
        if (hostInList(h, lists.schema)) {
          if (/"\$schema"\s*:/.test(hit.lineText)) continue;
          r.violations.push({
            file: f,
            line: hit.line,
            rule: "hosts/schema-only",
            message: `${h} is a [schema] host: it may appear only as a "$schema" value`,
          });
          continue;
        }
        if (hostInList(h, lists.test)) {
          if (inTestCode(hit.index)) continue;
          r.violations.push({
            file: f,
            line: hit.line,
            rule: "hosts/test-only",
            message: `${h} is a [test] host: it may appear only in fork test code (test files, #[cfg(test)] mod blocks), never in production code or its comments`,
          });
          continue;
        }
        r.violations.push({
          file: f,
          line: hit.line,
          rule: "hosts/fork",
          message: `host ${h} (${hit.url}) is not in ci/allowed-hosts.txt (R5 §1); new hosts need an owner-approved PR`,
        });
      } else {
        upstreamHosts.add(h);
        if (hostInList(h, upstreamOk)) continue;
        r.violations.push({
          file: f,
          line: hit.line,
          rule: "hosts/upstream",
          message: `new upstream host ${h} (${hit.url}): review it, then add it to ci/upstream-hosts-baseline.txt, prove it unreachable in ci/inert-hosts.txt, or list it in R5 §1`,
        });
      }
    }
  }
  r.notes.push(
    `scanned ${forkFiles} fork file(s), ${upstreamFiles} upstream file(s); ${upstreamHosts.size} distinct upstream host(s)`,
  );
  const stale = lists.baseline.filter((b) => !upstreamHosts.has(b));
  if (stale.length) {
    r.notes.push(
      `baseline hosts no longer found upstream (may be removed): ${stale.join(", ")}`,
    );
  }
  return r;
}

/** Every host in upstream scope with the files naming it (baseline input). */
export function listUpstreamHosts(root: string): Map<string, Set<string>> {
  const hosts = new Map<string, Set<string>>();
  for (const f of listFiles(root)) {
    if (!isTextFile(f) || !isUpstreamHostScope(f)) continue;
    for (const hit of findUrls(readIfExists(root, f) ?? "")) {
      const files = hosts.get(hit.host) ?? new Set<string>();
      files.add(f);
      hosts.set(hit.host, files);
    }
  }
  return new Map([...hosts].sort(([a], [b]) => a.localeCompare(b)));
}

if (import.meta.main) {
  if (process.argv.includes("--list-upstream")) {
    // Prints a draft for ci/upstream-hosts-baseline.txt; review every line.
    for (const [host, files] of listUpstreamHosts(REPO_ROOT)) {
      const list = [...files];
      console.log(
        `${host.padEnd(32)} # ${list.slice(0, 3).join(", ")}${list.length > 3 ? `, +${list.length - 3}` : ""}`,
      );
    }
  } else {
    cliMain(() => runHosts());
  }
}
