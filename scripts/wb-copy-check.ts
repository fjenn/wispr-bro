// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * wb-copy-check (S-37, R5 §1 own-words rule, DEC-45, RV-24): fails when an
 * English fork string reuses Wispr Flow's UI copy.
 *
 * Wispr Flow's wording is what R1 (`docs/research/R1-wispr-flow-teardown.md`)
 * quotes in double quotes, plus the strings RV-24 (`docs/research/
 * r5-review.md`) listed as reused. Our copy is every string in
 * `src/i18n/locales/en/fork.json` (Hub, settings, onboarding, lessons, the
 * bar, tray, menus and VoiceOver announcements under `rust.*`) and the
 * English sample texts (`src/fork/onboarding/samples/en.json`).
 *
 * The rule: a fork string matches a quote when both share a run of five or
 * more words ("over four words long", R5 §1), after normalising case,
 * punctuation, quotes, ⌥/Opt key names and the product name ("Flow",
 * "Wispr Flow", "Wispr", "Wispr Bro" and `{{appName}}` all become one
 * token). Generic functional labels are short and never reach five words.
 *
 * A reviewed exception goes in `scripts/wb-copy-check.allow.json` as
 * `{ "<fork key>": "<why it is our own wording>" }`; an entry whose key no
 * longer matches anything is reported as stale.
 *
 * Usage:
 *   bun scripts/wb-copy-check.ts          # report; exit 1 on a match
 *   bun scripts/wb-copy-check.ts --json   # machine-readable report
 */
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, "..");

/** Words a shared run needs before it counts as reused copy. */
export const MIN_WORDS = 5;

export interface Quote {
  text: string;
  source: string;
}
export interface Finding {
  key: string;
  text: string;
  quote: string;
  source: string;
  shared: string;
}

const BRAND = "appname";

/** Lower-case words, punctuation and markup removed, the brand as one token. */
export function words(text: string): string[] {
  const s = text
    .toLowerCase()
    .replace(/\{\{\s*appname\s*\}\}/g, ` ${BRAND} `)
    .replace(/\{\{[^}]*\}\}/g, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/[‘’ʼ]/g, "'")
    .replace(/\bwispr\s+(flow|bro)\b/g, ` ${BRAND} `)
    .replace(/\b(wispr|flow)\b/g, ` ${BRAND} `)
    .replace(/⌥\s*(opt(ion)?)?/g, " option ")
    .replace(/\bopt\b/g, "option")
    .replace(/[^\p{L}\p{N}' ]+/gu, " ")
    .replace(/(^| )'+|'+(?= |$)/g, " ");
  return s.split(/\s+/).filter(Boolean);
}

function runs(ws: string[], n: number): Set<string> {
  const out = new Set<string>();
  for (let i = 0; i + n <= ws.length; i++)
    out.add(ws.slice(i, i + n).join(" "));
  return out;
}

/** The longest run of words `a` and `b` share, if it reaches `MIN_WORDS`. */
export function sharedRun(a: string, b: string): string | null {
  const wa = words(a);
  const wb = words(b);
  if (wa.length < MIN_WORDS || wb.length < MIN_WORDS) return null;
  const rb = runs(wb, MIN_WORDS);
  let best: string | null = null;
  for (let i = 0; i + MIN_WORDS <= wa.length; i++) {
    if (!rb.has(wa.slice(i, i + MIN_WORDS).join(" "))) continue;
    // Grow the match as far as it goes.
    let n = MIN_WORDS;
    while (
      i + n < wa.length &&
      wb.join(" ").includes(wa.slice(i, i + n + 1).join(" "))
    ) {
      n++;
    }
    const run = wa.slice(i, i + n).join(" ");
    if (!best || run.length > best.length) best = run;
  }
  return best;
}

/** Double-quoted strings of five or more words in a Markdown file. */
export function quotesIn(markdown: string, source: string): Quote[] {
  const out: Quote[] = [];
  for (const m of markdown.matchAll(/"([^"\n]{8,400})"/g)) {
    const text = m[1].replace(/\*\*/g, "").trim();
    if (words(text).length >= MIN_WORDS) out.push({ text, source });
  }
  for (const m of markdown.matchAll(/“([^”\n]{8,400})”/g)) {
    const text = m[1].replace(/\*\*/g, "").trim();
    if (words(text).length >= MIN_WORDS) out.push({ text, source });
  }
  return out;
}

/** R1's quotes and the RV-24 row of the R5 review. */
export function wisprQuotes(root = ROOT): Quote[] {
  const r1 = "docs/research/R1-wispr-flow-teardown.md";
  const review = "docs/research/r5-review.md";
  const quotes = quotesIn(fs.readFileSync(path.join(root, r1), "utf8"), r1);
  const rv = fs
    .readFileSync(path.join(root, review), "utf8")
    .split("\n")
    .filter((l) => l.startsWith("| RV-24 "))
    .join("\n");
  return [...quotes, ...quotesIn(rv, `${review} RV-24`)];
}

/** Every string leaf of a JSON tree as `dotted.key → text`. */
export function flatten(
  node: unknown,
  prefix = "",
  out: Map<string, string> = new Map(),
): Map<string, string> {
  if (typeof node === "string") out.set(prefix, node);
  else if (Array.isArray(node))
    node.forEach((v, i) => flatten(v, `${prefix}[${i}]`, out));
  else if (node && typeof node === "object")
    for (const [k, v] of Object.entries(node))
      flatten(v, prefix ? `${prefix}.${k}` : k, out);
  return out;
}

/** The English fork copy: fork.json (without Handy's rebranded overrides) and the sample texts. */
export function forkStrings(root = ROOT): Map<string, string> {
  const fork = JSON.parse(
    fs.readFileSync(path.join(root, "src/i18n/locales/en/fork.json"), "utf8"),
  ) as Record<string, unknown>;
  // `overrides` is Handy's own copy with the name swapped (F-01), not ours.
  const { overrides: _handy, ...ours } = fork;
  const out = flatten(ours);
  const samples = path.join(root, "src/fork/onboarding/samples/en.json");
  if (fs.existsSync(samples)) {
    flatten(JSON.parse(fs.readFileSync(samples, "utf8")), "samples.en", out);
  }
  return out;
}

export function check(
  strings: Map<string, string>,
  quotes: Quote[],
  allow: Record<string, string> = {},
): { findings: Finding[]; stale: string[] } {
  const findings: Finding[] = [];
  const allowedHit = new Set<string>();
  // Index every quote by its five-word runs, so each string is compared
  // only with the quotes it shares a run with.
  const byRun = new Map<string, Quote[]>();
  for (const q of quotes) {
    for (const run of runs(words(q.text), MIN_WORDS)) {
      const list = byRun.get(run) ?? [];
      if (!list.includes(q)) list.push(q);
      byRun.set(run, list);
    }
  }
  for (const [key, text] of strings) {
    const candidates = new Set<Quote>();
    for (const run of runs(words(text), MIN_WORDS)) {
      for (const q of byRun.get(run) ?? []) candidates.add(q);
    }
    for (const q of candidates) {
      const shared = sharedRun(text, q.text);
      if (!shared) continue;
      if (key in allow) {
        allowedHit.add(key);
        continue;
      }
      findings.push({ key, text, quote: q.text, source: q.source, shared });
    }
  }
  const stale = Object.keys(allow).filter((k) => !allowedHit.has(k));
  return { findings, stale };
}

function main(): void {
  const allowPath = path.join(__dirname, "wb-copy-check.allow.json");
  const allow: Record<string, string> = fs.existsSync(allowPath)
    ? JSON.parse(fs.readFileSync(allowPath, "utf8"))
    : {};
  const strings = forkStrings();
  const quotes = wisprQuotes();
  const { findings, stale } = check(strings, quotes, allow);
  if (process.argv.includes("--json")) {
    console.log(JSON.stringify({ findings, stale }, null, 2));
  } else {
    for (const f of findings) {
      console.error(
        `✗ ${f.key}: "${f.text}"\n    shares "${f.shared}" with "${f.quote}" (${f.source})`,
      );
    }
    for (const k of stale) {
      console.error(`✗ stale allow-list entry: ${k}`);
    }
    if (findings.length === 0 && stale.length === 0) {
      console.log(
        `✓ copy check: ${strings.size} English fork strings, ${quotes.length} Wispr quotes, no shared run of ${MIN_WORDS}+ words`,
      );
    }
  }
  if (findings.length > 0 || stale.length > 0) process.exit(1);
}

if (import.meta.main) main();
