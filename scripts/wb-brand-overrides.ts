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
 *
 * Sentence case (OT2-E, Mac wave 6: Handy's hosted rows in Title Case next
 * to the fork's sentence case). Every key outside `tray.*` (the native
 * macOS menu, which keeps Apple's Title Case and is built from Handy's
 * files at compile time) whose English text is a Title Case label
 * ("Launch on Startup") gets an override in sentence case ("Launch on
 * startup"), in English and in each locale from that locale's own text:
 *   - a word after the first that starts with a capital and is otherwise
 *     lower case is lowered (`toLocaleLowerCase(lng)`, so Turkish "I" →
 *     "ı"), also after a hyphen ("Auto-Delete" → "Auto-delete");
 *   - names stay: `KEEP_WORDS`/`KEEP_PHRASES` (Wispr Bro, GitHub, Whisper,
 *     Secure Input, Hong Kong, Auto, …), all-caps (API, GPU) and mixed case
 *     (OpenAI) words; English keeps its language names;
 *   - locales without letter case (ar, he, hi, ja, ko, ne, zh, zh-TW) and
 *     German (nouns are capitalised, which a rule cannot tell from Title
 *     Case) are left as translated (`NO_SENTENCE_CASE`).
 * A case change on a reviewed override keeps its flag; a new override
 * whose only change is the case is flagged "machine" like a rebrand.
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

/** Locales left alone by the sentence-case rule (see the header). */
export const NO_SENTENCE_CASE = new Set([
  "ar",
  "de",
  "he",
  "hi",
  "ja",
  "ko",
  "ne",
  "zh",
  "zh-TW",
]);
/** Words that keep their capital (names, product and option names). */
export const KEEP_WORDS = new Set([
  "Wispr",
  "Bro",
  "Bros",
  "Handy",
  "GitHub",
  "Whisper",
  "Parakeet",
  "Breeze",
  "Moonshine",
  "SenseVoice",
  "Apple",
  "Mac",
  "Windows",
  "Linux",
  "Auto",
  "Hugging",
  "Face",
  // Places and languages that every language capitalises, as the locales
  // spell them (Traditional (Taiwan), (Hong Kong), Translate to English).
  "Taiwan",
  "Hong",
  "Kong",
  "Тайван",
  "Хонконг",
  "Tchaj",
  "Hongkong",
  "Taiwán",
  "Taïwan",
  "Tajwan",
  "Тайвань",
  "Гонконг",
  "Tayvan",
  "Đài",
  "Loan",
  "Hồng",
  "Kông",
  "Inggris",
  "Tionghoa",
  "Engels",
  "İngilizce",
  "Anh",
  "Hán",
]);
/**
 * Other locales keep the English product terms Handy's translators left in
 * English (Anthropic Messages, (Chat Completions), GitHub Releases).
 */
const KEEP_WORDS_FOREIGN = new Set([
  "Messages",
  "Chat",
  "Completions",
  "Releases",
]);
/** English also keeps language and place names. */
const KEEP_WORDS_EN = new Set(["English", "Chinese"]);
/** Multi-word names kept whole (their later words stay capitalised). */
export const KEEP_PHRASES = ["Secure Input", "Hong Kong", "Wispr Bro"];
/** Small words a Title Case label may leave in lower case. */
const MINOR = new Set(
  "a an and as at between by for from in into of on or per the to via vs with without".split(
    " ",
  ),
);

/** True for an English Title Case label (not a sentence). */
export function isTitleCase(text: string): boolean {
  const plain = text
    .replace(/\{\{[^}]*\}\}/g, "")
    .replace(/<[^>]+>/g, "")
    .trim();
  if (/[.!?:…]$/.test(plain)) return false;
  const words = plain
    .split(/[\s/]+/)
    .map((w) => w.replace(/^[^\p{L}]+|[^\p{L}]+$/gu, ""))
    .filter((w) => /\p{L}/u.test(w));
  if (words.length < 2) return false;
  let capitals = 0;
  for (const w of words.slice(1)) {
    if (MINOR.has(w)) continue;
    if (/^\p{Lu}/u.test(w)) capitals++;
    else return false;
  }
  return capitals > 0;
}

/** A locale's text is Title Case: every later word of ≥ 4 letters capitalised. */
function isTitleCaseIn(text: string): boolean {
  const words = text
    .replace(/\{\{[^}]*\}\}|<[^>]+>/g, "")
    .split(/[\s/]+/)
    .map((w) => w.replace(/^[^\p{L}]+|[^\p{L}]+$/gu, ""))
    .filter((w) => /\p{L}/u.test(w))
    .slice(1)
    .filter((w) => [...w].length >= 4);
  return words.length > 0 && words.every((w) => /^\p{Lu}/u.test(w));
}

/** One word's sentence-case form (not the first word). */
function lowerWord(word: string, lng: string, keep: Set<string>): string {
  return word
    .split("-")
    .map((part, i) => {
      const core = part.replace(/^[^\p{L}]+|[^\p{L}]+$/gu, "");
      // "Releases’i": a suffix after an apostrophe belongs to the name.
      if (!core || keep.has(core) || keep.has(core.split(/['’]/)[0]))
        return part;
      // All caps (API) or mixed case (OpenAI, GitHub) stays.
      if (core.length > 1 && core === core.toLocaleUpperCase(lng)) return part;
      if (/\p{Lu}/u.test(core.slice(1))) return part;
      if (!/^\p{Lu}/u.test(core)) return part;
      // "Auto-Delete": the first part of a word keeps its capital only
      // when it is the label's first word (handled by the caller).
      void i;
      const at = part.indexOf(core);
      return (
        part.slice(0, at) +
        core[0].toLocaleLowerCase(lng) +
        core.slice(1) +
        part.slice(at + core.length)
      );
    })
    .join("-");
}

/**
 * Sentence case for one label in `lng`: the first word keeps its capital
 * (its later hyphen parts are lowered), names and acronyms stay.
 */
export function sentenceCase(text: string, lng: string): string {
  if (NO_SENTENCE_CASE.has(lng)) return text;
  // A translation that already has a lower-case word of four or more
  // letters after its first word is in sentence case already; its other
  // capitals are names the translator chose (an option's name, "Inglês").
  if (lng !== "en" && !isTitleCaseIn(text)) return text;
  const keep =
    lng === "en"
      ? new Set([...KEEP_WORDS, ...KEEP_WORDS_EN])
      : new Set([...KEEP_WORDS, ...KEEP_WORDS_FOREIGN]);
  // Protect placeholders, tags and kept phrases.
  const held: string[] = [];
  const hold = (m: string) => {
    held.push(m);
    return `\u0000${held.length - 1}\u0000`;
  };
  // Quoted option names (« Auto », 'Wat is er nieuw') keep their case.
  let out = text.replace(
    /\{\{[^}]*\}\}|<[^>]+>|„[^“]*“|«[^»]*»|“[^”]*”|"[^"]*"|(?<=^|\s)'[^']*'/g,
    hold,
  );
  for (const phrase of KEEP_PHRASES) out = out.split(phrase).join(hold(phrase));
  let first = true;
  out = out.replace(/[^\s/]+/g, (token) => {
    if (token.includes("\u0000") || !/\p{L}/u.test(token)) {
      if (/\p{L}|\u0000/u.test(token)) first = false;
      return token;
    }
    if (first) {
      first = false;
      const [head, ...rest] = token.split("-");
      return rest.length
        ? [head, lowerWord(rest.join("-"), lng, keep)].join("-")
        : token;
    }
    return lowerWord(token, lng, keep);
  });
  return out.replace(/\u0000(\d+)\u0000/g, (_, i) => held[Number(i)]);
}

/** Keys the sentence-case rule covers: English Title Case labels, no tray. */
export function caseKeys(english: Map<string, string>): string[] {
  return [...english.keys()].filter(
    (k) =>
      !k.startsWith("tray.") &&
      // Model names ("Whisper Small") are names, not labels.
      !/\.models\.[^.]+\.name$/.test(k) &&
      isTitleCase(english.get(k)!),
  );
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
  /** Overrides on disk still in Title Case (OT2-E). */
  notSentenceCase: string[];
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
  const branded = new Set(keys);
  // Sentence case applies to English Title Case labels only, never to a
  // brand override that is a sentence.
  const labels = new Set(caseKeys(english));
  const cased = (key: string, text: string) =>
    labels.has(key) ? sentenceCase(text, lng) : text;
  for (const key of labels) {
    if (branded.has(key)) continue;
    const base = local.get(key) ?? english.get(key)!;
    if (sentenceCase(base, lng) !== base) keys.push(key);
  }
  const result: LocaleResult = {
    lng,
    overrides: new Map(),
    flags: {},
    missing: [],
    stale: [],
    stillBranded: [],
    notSentenceCase: [],
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
    if (existing !== undefined && cased(key, existing) !== existing) {
      result.notSentenceCase.push(key);
    }
    const reviewed =
      existing !== undefined &&
      (flags[flagKey] === undefined || flags[flagKey] === "ai-reviewed") &&
      !UPSTREAM_BRAND.test(existing);
    let value: string;
    if (reviewed) {
      value = cased(key, existing);
      // An AI review keeps its flag (wb-translate-missing.ts): it is not a
      // person's review, but it must not be rebranded over either.
      if (flags[flagKey] === "ai-reviewed")
        result.flags[flagKey] = "ai-reviewed";
    } else {
      value = cased(key, rebrand(base, lng));
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
  // Keep `overrides` where it is in the file (a new one goes last), so a
  // regenerate changes only the values.
  fork[OVERRIDES_KEY] = nest([...r.overrides]);
  writeJson(jsonPath, fork);

  const flags = readJson<Flags>(flagsPath, {});
  for (const key of Object.keys(flags)) {
    if (key.startsWith(FLAG_PREFIX)) delete flags[key];
  }
  // Sorted, like `wb-translate-missing.ts` and the merge driver.
  const merged: Flags = { ...flags, ...r.flags };
  const sorted: Flags = {};
  for (const key of Object.keys(merged).sort()) sorted[key] = merged[key];
  writeJson(flagsPath, sorted);
}

function main(): void {
  const check = process.argv.includes("--check");
  const english = handyStrings(SOURCE_LANG);
  let drift = false;
  for (const lng of locales()) {
    const r = computeLocale(lng, english);
    const problems = check
      ? [
          ...r.missing,
          ...r.stale,
          ...r.stillBranded,
          ...r.notSentenceCase,
          ...r.markupChanged,
        ]
      : r.markupChanged;
    if (problems.length) drift = true;
    if (!check) writeLocale(r);
    const parts = [
      r.missing.length && `${check ? "missing" : "added"} ${r.missing.length}`,
      r.stale.length && `${check ? "stale" : "removed"} ${r.stale.length}`,
      r.stillBranded.length &&
        `still names Handy: ${r.stillBranded.join(", ")}`,
      r.notSentenceCase.length &&
        `not in sentence case: ${r.notSentenceCase.join(", ")}`,
      r.markupChanged.length &&
        `placeholder/tag change: ${r.markupChanged.join(", ")}`,
      `${r.overrides.size} overrides, ${Object.keys(r.flags).length} flagged`,
    ].filter(Boolean);
    console.log(`${problems.length ? "✗" : "✓"} ${lng}: ${parts.join(", ")}`);
  }
  if (drift) process.exit(1);
}

if (import.meta.main) main();
