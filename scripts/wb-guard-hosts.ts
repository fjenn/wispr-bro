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
//   [loopback] local hosts, never network egress.
//
// Upstream code (the rest of src/, src-tauri/src/, src-tauri/*.json): every
// host must be in `ci/upstream-hosts-baseline.txt` (reviewed), in
// `ci/inert-hosts.txt` (proven unreachable) or in the [network]/[loopback]
// sections of `ci/allowed-hosts.txt`. A new upstream host fails until it is
// reviewed (R5 §4.12).

import {
  cliMain,
  findUrls,
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
  baseline: string[];
  inert: string[];
}

const ALLOWED_SECTIONS = new Set(["network", "link", "schema", "loopback"]);

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
        message: `${e.value} is outside a [network]/[link]/[schema]/[loopback] section`,
      });
    }
  }
  const inert = readList(root, "ci/inert-hosts.txt");
  for (const e of inert) {
    if (!e.comment) {
      problems.push({
        file: "ci/inert-hosts.txt",
        line: e.line,
        message: `${e.value} needs a comment naming the test (or reason) that proves it unreachable`,
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
    /^src\/i18n\/locales\/[^/]+\/fork(\.flags)?\.json$/.test(rel)
  );
}

export function isUpstreamHostScope(rel: string): boolean {
  if (isForkHostScope(rel)) return false;
  if (rel.startsWith("src/") || rel.startsWith("src-tauri/src/")) return true;
  return /^src-tauri\/[^/]+\.json$/.test(rel);
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
