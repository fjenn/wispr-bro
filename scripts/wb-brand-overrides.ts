// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * wb-brand-overrides (F-01, DEC-35): rebrand Handy's UI strings without
 * editing Handy's `translation.json` files.
 *
 * For every locale it finds each key of Handy's
 * `src/i18n/locales/<lng>/translation.json` whose text names Handy (in English
 * or in that locale) and writes a rebranded copy under the top-level
 * `overrides` object of `src/i18n/locales/<lng>/fork.json`. At load,
 * `src/fork/i18n.ts` merges `overrides` into Handy's `translation` namespace
 * with `i18n.addResourceBundle(lng, "translation", overrides, true, true)`.
 *
 * Usage:
 *   bun scripts/wb-brand-overrides.ts           # (re)write overrides
 *   bun scripts/wb-brand-overrides.ts --check   # report only, exit 1 on drift
 *
 * Rules
 *   - The brand is written literally ("Wispr Bro"), not as `{{appName}}`:
 *     Handy's call sites pass no `appName` variable, and a literal keeps the
 *     override a plain i18next string.
 *   - Grammar: Turkish case suffixes follow vowel harmony for "Bro"
 *     (Handy'nin → Wispr Bro'nun, Handy'yi → Wispr Bro'yu, Handy'ye →
 *     Wispr Bro'ya); Danish/Swedish genitive "Handys" → "Wispr Bros";
 *     compounds ("Handy-Symbol") and English/Dutch "Handy's" keep their form.
 *   - The env var name `HANDY_DISABLE_UPDATER` becomes
 *     `WISPRBRO_DISABLE_UPDATER` (both are honoured, `fork::early_init`).
 *   - Generated values are flagged in `fork.flags.json` as
 *     `"overrides.<key>": "machine"` (mechanical rewrite, awaits review) or
 *     `"untranslated"` (the locale lacks the key, so English was rebranded).
 *     An override without a flag, or flagged `"ai-reviewed"` (kept), counts
 *     as reviewed and is kept as is, unless
 *     it still names Handy or Handy's key disappeared.
 *   - `{{placeholders}}` and `<tags>` must survive unchanged (checked).
 */
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
export const LOCALES_DIR = path.join(__dirname, "..", "src", "i18n", "locales");
const SOURCE_LANG = "en";
const OVERRIDES_KEY = "overrides";
const FLAG_PREFIX = `${OVERRIDES_KEY}.`;

export const BRAND = "Wispr Bro";
/** Matches any upstream brand mention the rewrite must remove. */
export const UPSTREAM_BRAND = /Handy|HANDY_/;

type Json = { [key: string]: string | Json };
type Flag = "untranslated" | "machine" | "ai-reviewed";
type Flags = Record<string, Flag>;

/** Locale-specific rewrites, applied before the generic one. */
const LOCALE_RULES: Record<string, [RegExp, string][]> = {
  tr: [
    [/Handy(['’])nin/g, `${BRAND}$1nun`],
    [/Handy(['’])yi/g, `${BRAND}$1yu`],
    [/Handy(['’])ye/g, `${BRAND}$1ya`],
    [/Handy(['’])yle/g, `${BRAND}$1yla`],
    [/Handy(['’])de/g, `${BRAND}$1da`],
    [/Handy(['’])den/g, `${BRAND}$1dan`],
    [/Handy(['’])dir/g, `${BRAND}$1dur`],
  ],
};

const GENERIC_RULES: [RegExp, string][] = [
  [/HANDY_DISABLE_UPDATER/g, "WISPRBRO_DISABLE_UPDATER"],
  // Also covers "Handy's", "Handys" (da/sv genitive) and "Handy-…" compounds.
  [/Handy/g, BRAND],
];

/** Replaces Handy's brand in one string, keeping the locale's grammar. */
export function rebrand(text: string, lng: string): string {
  let out = text;
  for (const [re, to] of [...(LOCALE_RULES[lng] ?? []), ...GENERIC_RULES]) {
    out = out.replace(re, to);
  }
  return out;
}

export function flatten(obj: Json, prefix = ""): Map<string, string> {
  const out = new Map<string, string>();
  for (const [key, value] of Object.entries(obj)) {
    const dotted = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "string") out.set(dotted, value);
    else if (value && typeof value === "object") {
      for (const [k, v] of flatten(value, dotted)) out.set(k, v);
    }
  }
  return out;
}

function nest(entries: [string, string][]): Json {
  const root: Json = {};
  for (const [dotted, value] of entries) {
    const parts = dotted.split(".");
    let node = root;
    for (const part of parts.slice(0, -1)) {
      node[part] ??= {};
      node = node[part] as Json;
    }
    node[parts[parts.length - 1]] = value;
  }
  return root;
}

/** `{{placeholders}}` and `<tag>` names, sorted, for invariance checks. */
export function markup(text: string): string {
  const vars = [...text.matchAll(/\{\{\s*([^}\s]+)\s*\}\}/g)].map((m) => m[1]);
  const tags = [...text.matchAll(/<\/?([a-zA-Z0-9_]+)\s*\/?>/g)].map((m) =>
    m[0].replace(/\s+/g, ""),
  );
  return [...vars, ...tags].sort().join("\u0000");
}

export function readJson<T>(file: string, fallback: T): T {
  if (!fs.existsSync(file)) return fallback;
  return JSON.parse(fs.readFileSync(file, "utf8")) as T;
}

function writeJson(file: string, data: unknown): void {
  fs.writeFileSync(file, JSON.stringify(data, null, 2) + "\n");
}

export function locales(): string[] {
  return fs
    .readdirSync(LOCALES_DIR, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .sort();
}

export function handyStrings(lng: string): Map<string, string> {
  return flatten(
    readJson<Json>(path.join(LOCALES_DIR, lng, "translation.json"), {}),
  );
}

/**
 * Keys that need an override in `lng`, in English key order: every key whose
 * English text names Handy, plus any key whose `lng` text does.
 */
export function brandKeys(
  english: Map<string, string>,
  local: Map<string, string>,
): string[] {
  const keys = [...english.keys()].filter((k) =>
    UPSTREAM_BRAND.test(english.get(k)!),
  );
  for (const [k, v] of local) {
    if (UPSTREAM_BRAND.test(v) && !keys.includes(k)) keys.push(k);
  }
  return keys;
}

export interface LocaleResult {
  lng: string;
  overrides: Map<string, string>;
  flags: Flags;
  missing: string[];
  stale: string[];
  stillBranded: string[];
  markupChanged: string[];
}

/** Computes the overrides for one locale from the files on disk. */
export function computeLocale(
  lng: string,
  english: Map<string, string>,
): LocaleResult {
  const dir = path.join(LOCALES_DIR, lng);
  const fork = readJson<Json>(path.join(dir, "fork.json"), {});
  const current = flatten((fork[OVERRIDES_KEY] as Json | undefined) ?? {});
  const flags = readJson<Flags>(path.join(dir, "fork.flags.json"), {});
  const local = lng === SOURCE_LANG ? english : handyStrings(lng);

  const keys = brandKeys(english, local);
  const result: LocaleResult = {
    lng,
    overrides: new Map(),
    flags: {},
    missing: [],
    stale: [],
    stillBranded: [],
    markupChanged: [],
  };
  for (const key of keys) {
    const base = local.get(key) ?? english.get(key)!;
    const existing = current.get(key);
    const flagKey = FLAG_PREFIX + key;
    if (existing === undefined) result.missing.push(key);
    if (existing !== undefined && UPSTREAM_BRAND.test(existing)) {
      result.stillBranded.push(key);
    }
    const reviewed =
      existing !== undefined &&
      (flags[flagKey] === undefined || flags[flagKey] === "ai-reviewed") &&
      !UPSTREAM_BRAND.test(existing);
    let value: string;
    if (reviewed) {
      value = existing;
      // An AI review keeps its flag (wb-translate-missing.ts): it is not a
      // person's review, but it must not be rebranded over either.
      if (flags[flagKey] === "ai-reviewed")
        result.flags[flagKey] = "ai-reviewed";
    } else {
      value = rebrand(base, lng);
      if (!local.has(key)) result.flags[flagKey] = "untranslated";
      else if (value !== base) result.flags[flagKey] = "machine";
    }
    if (markup(value) !== markup(base)) result.markupChanged.push(key);
    result.overrides.set(key, value);
  }
  for (const key of current.keys()) {
    if (!keys.includes(key)) result.stale.push(key);
  }
  return result;
}

/** Writes `overrides` (last key of `fork.json`) and its flags. */
function writeLocale(r: LocaleResult): void {
  const dir = path.join(LOCALES_DIR, r.lng);
  const jsonPath = path.join(dir, "fork.json");
  const flagsPath = path.join(dir, "fork.flags.json");
  const fork = readJson<Json>(jsonPath, {});
  delete fork[OVERRIDES_KEY];
  fork[OVERRIDES_KEY] = nest([...r.overrides]);
  writeJson(jsonPath, fork);

  const flags = readJson<Flags>(flagsPath, {});
  for (const key of Object.keys(flags)) {
    if (key.startsWith(FLAG_PREFIX)) delete flags[key];
  }
  writeJson(flagsPath, { ...flags, ...r.flags });
}

function main(): void {
  const check = process.argv.includes("--check");
  const english = handyStrings(SOURCE_LANG);
  let drift = false;
  for (const lng of locales()) {
    const r = computeLocale(lng, english);
    const problems = check
      ? [...r.missing, ...r.stale, ...r.stillBranded, ...r.markupChanged]
      : r.markupChanged;
    if (problems.length) drift = true;
    if (!check) writeLocale(r);
    const parts = [
      r.missing.length && `${check ? "missing" : "added"} ${r.missing.length}`,
      r.stale.length && `${check ? "stale" : "removed"} ${r.stale.length}`,
      r.stillBranded.length &&
        `still names Handy: ${r.stillBranded.join(", ")}`,
      r.markupChanged.length &&
        `placeholder/tag change: ${r.markupChanged.join(", ")}`,
      `${r.overrides.size} overrides, ${Object.keys(r.flags).length} flagged`,
    ].filter(Boolean);
    console.log(`${problems.length ? "✗" : "✓"} ${lng}: ${parts.join(", ")}`);
  }
  if (drift) process.exit(1);
}

if (import.meta.main) main();
