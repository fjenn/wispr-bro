// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * wb-translate-missing (W-25): keep the 25 non-English `fork.json` files in
 * key parity with `src/i18n/locales/en/fork.json`, the English source.
 *
 * Usage:
 *   bun scripts/wb-translate-missing.ts            # fill missing keys
 *   bun scripts/wb-translate-missing.ts --check    # report only, exit 1 on drift
 *   (both modes exit 1 on a placeholder mismatch in an existing translation)
 *   bun scripts/wb-translate-missing.ts --backend copy-english
 *   bun scripts/wb-translate-missing.ts --backend llama-server \
 *     --server <llama-server> --model <gguf>   # local LLM, flag "machine"
 *   ... --locale de,fr                              # only these locales
 *   bun scripts/wb-translate-missing.ts --review de   # flagged keys, as TSV
 *   bun scripts/wb-translate-missing.ts --strict   # --check, and fail on any
 *     `untranslated` or `machine` flag (incl. `overrides.*`; S-37)
 *   bun scripts/wb-translate-missing.ts --strict-switch
 *     # CI: --strict when `ci/i18n-strict` says `I18N_STRICT=on`, otherwise
 *     # --check plus a report of the flags left (exit 0 for flags)
 *
 * Marking scheme (documented in docs/fork/ARCHITECTURE.md, "i18n"):
 *   Every locale may have a sidecar `fork.flags.json` next to its `fork.json`:
 *     { "debug.title": "untranslated", "debug.close": "machine" }
 *   Keys are dotted paths into `fork.json`; values are
 *     - "untranslated": the value is a copy of the English string,
 *     - "machine": the value was machine-translated and awaits review,
 *     - "ai-reviewed": a strong model reviewed and corrected the value
 *       against the glossary (docs/fork/TRANSLATION-REVIEW.md); it passes
 *       strict mode but is not a native speaker's review.
 *   A key that a person has reviewed has no flag. `fork.json` itself stays a
 *   plain i18next resource file (the same shape as English), so i18next,
 *   Handy's `check-translations.ts` (which reads only `translation.json`) and
 *   ESLint never see the flags. Flags pass CI until S-37 (R5 §4.12).
 *
 * Plurals (S-37b): a key ending in an i18next plural suffix (`_zero`, `_one`,
 * `_two`, `_few`, `_many`, `_other`) belongs to a plural group. English has
 * exactly `<base>_one` and `<base>_other`; every other locale has exactly the
 * CLDR cardinal categories of its language
 * (`Intl.PluralRules(lang).resolvedOptions().pluralCategories`: `ja` only
 * `_other`, `ru` `_one/_few/_many/_other`, `ar` all six). A locale's `_one` is
 * translated from English `_one` when that has the same placeholders as
 * `_other` or the locale's "one" is the number 1 alone; otherwise (English
 * "A recording was saved" for Russian, whose "one" also covers 21) from
 * English `_other`. Every other form comes from `_other`. The category and
 * example counts go to the backend as a hint. Rust resolves the same
 * groups with ICU4X (`fork/i18n.rs` `t_plural`).
 *
 * The script also:
 *   - drops keys that no longer exist in English (and their flags),
 *   - notices an English string whose text changed (hashes in
 *     `en/fork.source-hashes.json`) and translates it again in every locale,
 *   - re-translates a flagged (never reviewed) entry whose placeholders no
 *     longer match its English source, instead of failing,
 *   - with a translating backend (flag "machine"), also translates every
 *     entry still flagged "untranslated" (an English copy) and every
 *     "machine" entry that lost a kept name (`KEPT_NAMES`, retried once with
 *     the names spelled out) or mixes in another language's script
 *     (`foreignScripts`, "הذا"),
 *   - leaves strings without letters ("{{from}} → {{to}}") unflagged: there
 *     is nothing to translate or review,
 *   - checks that `{{placeholders}}` match English for every key; a backend
 *     result with different placeholders is rejected and the English copy is
 *     kept, flagged "untranslated",
 *   - never touches Handy's `translation.json` files,
 *   - leaves the top-level `overrides` object (brand overrides for Handy's
 *     `translation` namespace) and its `overrides.*` flags alone: they are
 *     per-locale and owned by `scripts/wb-brand-overrides.ts` (F-01).
 *
 * Backends are pluggable (`TranslationBackend`): "copy-english" (default)
 * and, since S-10, "llama-server" (`scripts/wb-translate-llama.ts`: the
 * bundled sidecar on a Unix socket, `--offline`; flag "machine"). No network
 * backend may be added (R5 §1 privacy rule).
 */
import fs from "fs";
import path from "path";
import { createHash } from "crypto";
import { fileURLToPath } from "url";
import { startLlamaServerBackend } from "./wb-translate-llama";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const LOCALES_DIR = path.join(__dirname, "..", "src", "i18n", "locales");
const SOURCE_LANG = "en";
const NAMESPACE_FILE = "fork.json";
const FLAGS_FILE = "fork.flags.json";
/** Top-level key owned by `wb-brand-overrides.ts`; not part of key parity. */
export const OVERRIDES_KEY = "overrides";

type Json = { [key: string]: string | Json };
type Flag = "untranslated" | "machine" | "ai-reviewed";
type Flags = Record<string, Flag>;

/** A translation backend: returns a translated string, or null to fall back. */
export interface TranslationBackend {
  readonly name: string;
  /** Flag recorded for strings this backend produced. */
  readonly flag: Flag;
  translate(
    text: string,
    targetLang: string,
    key: string,
    /** Extra instruction, e.g. which plural form to write. */
    hint?: string,
    /** Retry number: a sampling backend varies its output (0 = greedy). */
    attempt?: number,
  ): Promise<string | null>;
  /** Releases resources (the llama-server process). */
  close?(): Promise<void>;
}

/** Copies English and flags the entry "untranslated". */
export const copyEnglishBackend: TranslationBackend = {
  name: "copy-english",
  flag: "untranslated",
  async translate(text) {
    return text;
  },
};

const BACKENDS: Record<string, TranslationBackend> = {
  [copyEnglishBackend.name]: copyEnglishBackend,
};

export function flatten(obj: Json, prefix = ""): Map<string, string> {
  const out = new Map<string, string>();
  for (const [key, value] of Object.entries(obj)) {
    const dotted = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "string") {
      out.set(dotted, value);
    } else if (value && typeof value === "object") {
      for (const [k, v] of flatten(value, dotted)) out.set(k, v);
    } else {
      throw new Error(`${dotted}: only strings and objects are allowed`);
    }
  }
  return out;
}

/** Rebuilds a nested object in English key order. */
function unflatten(order: string[], values: Map<string, string>): Json {
  const root: Json = {};
  for (const dotted of order) {
    const value = values.get(dotted);
    if (value === undefined) continue;
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

/** `{{name}}` and `{{name, format}}` placeholders, whitespace-normalized. */
export function placeholders(text: string): string[] {
  return [...text.matchAll(/\{\{([^}]*)\}\}/g)]
    .map((m) => m[1].replace(/\s+/g, ""))
    .sort();
}

export function samePlaceholders(a: string, b: string): boolean {
  return placeholders(a).join("\u0000") === placeholders(b).join("\u0000");
}

/** Splits `fork.json` into its namespace strings and the `overrides` object. */
export function splitOverrides(data: Json): {
  strings: Json;
  overrides: Json | undefined;
} {
  const { [OVERRIDES_KEY]: overrides, ...strings } = data;
  if (overrides !== undefined && typeof overrides !== "object") {
    throw new Error(`${OVERRIDES_KEY} must be an object`);
  }
  return { strings, overrides: overrides as Json | undefined };
}

function isOverrideFlag(key: string): boolean {
  return key.startsWith(`${OVERRIDES_KEY}.`);
}

function readJson<T>(file: string, fallback: T): T {
  if (!fs.existsSync(file)) return fallback;
  return JSON.parse(fs.readFileSync(file, "utf8")) as T;
}

/**
 * Keys that appear twice in one JSON object. `JSON.parse` silently keeps the
 * last one, so a bad merge that duplicates a block (and drops keys) passes
 * every other check; this scans the raw text instead.
 */
export function duplicateKeys(text: string): string[] {
  const dups: string[] = [];
  const stack: { keys: Set<string>; isObj: boolean }[] = [];
  const path: string[] = [];
  let i = 0;
  let expectKey = false;
  while (i < text.length) {
    const c = text[i];
    if (c === '"') {
      let j = i + 1;
      while (text[j] !== '"') j += text[j] === "\\" ? 2 : 1;
      const s = JSON.parse(text.slice(i, j + 1)) as string;
      i = j + 1;
      const top = stack[stack.length - 1];
      if (top?.isObj && expectKey) {
        if (top.keys.has(s))
          dups.push([...path.slice(0, stack.length - 1), s].join("."));
        top.keys.add(s);
        path[stack.length - 1] = s;
        expectKey = false;
      }
      continue;
    }
    if (c === "{" || c === "[") {
      stack.push({ keys: new Set(), isObj: c === "{" });
      expectKey = c === "{";
    } else if (c === "}" || c === "]") {
      stack.pop();
      path.length = stack.length;
    } else if (c === ",") {
      expectKey = stack[stack.length - 1]?.isObj ?? false;
    }
    i++;
  }
  return dups;
}

function writeJson(file: string, data: unknown): void {
  fs.writeFileSync(file, JSON.stringify(data, null, 2) + "\n");
}

// ---------------------------------------------------------------------------
// Plurals

/** i18next's plural suffixes, in CLDR order. */
export const PLURAL_CATEGORIES = [
  "zero",
  "one",
  "two",
  "few",
  "many",
  "other",
] as const;
export type PluralCategory = (typeof PLURAL_CATEGORIES)[number];

/** `{ base, category }` for a plural key, else null. */
export function pluralParts(
  key: string,
): { base: string; category: PluralCategory } | null {
  const m = key.match(/^(.+)_(zero|one|two|few|many|other)$/);
  return m ? { base: m[1], category: m[2] as PluralCategory } : null;
}

/** The locale's CLDR cardinal categories, in CLDR order. */
export function pluralCategories(lang: string): PluralCategory[] {
  const have = new Set(
    new Intl.PluralRules(lang).resolvedOptions().pluralCategories,
  );
  return PLURAL_CATEGORIES.filter((c) => have.has(c));
}

const SAMPLE_COUNTS = [
  ...Array.from({ length: 201 }, (_, i) => i),
  1000,
  1_000_000,
  0.5,
  1.5,
  2.5,
];

/** A few counts of `category` in `lang` ("2, 3, 4, 22, 23"). */
export function exampleCounts(
  lang: string,
  category: PluralCategory,
): number[] {
  const rules = new Intl.PluralRules(lang);
  return SAMPLE_COUNTS.filter((n) => rules.select(n) === category).slice(0, 6);
}

/** Is the locale's "one" category the number 1 alone (among integers)? */
export function oneMeansOnlyOne(lang: string): boolean {
  const rules = new Intl.PluralRules(lang);
  for (let n = 0; n <= 1000; n++) {
    if (n !== 1 && rules.select(n) === "one") return false;
  }
  return true;
}

/** Problems with the English plural groups (each needs `_one` + `_other`). */
export function englishPluralProblems(source: Map<string, string>): string[] {
  const groups = new Map<string, Set<PluralCategory>>();
  for (const key of source.keys()) {
    const p = pluralParts(key);
    if (!p) continue;
    if (!groups.has(p.base)) groups.set(p.base, new Set());
    groups.get(p.base)!.add(p.category);
  }
  const problems: string[] = [];
  for (const [base, cats] of groups) {
    if (source.has(base))
      problems.push(`${base}: also exists without a suffix`);
    const extra = [...cats].filter((c) => c !== "one" && c !== "other");
    if (!cats.has("one") || !cats.has("other") || extra.length) {
      problems.push(
        `${base}: English needs exactly _one and _other (has ${[...cats].map((c) => `_${c}`).join(", ")})`,
      );
    }
  }
  return problems;
}

/** What one locale key is translated from. */
export interface SourceEntry {
  /** The English text the translation must mean. */
  english: string;
  /** For a plural form: which form to write. */
  hint?: string;
  /** The English keys it is translated from (both forms of a plural). */
  from: string[];
}

/**
 * The keys `lang` must have, in English order, each with its English source:
 * plain keys map 1:1, a plural group expands to the locale's categories.
 */
export function localeSource(
  source: Map<string, string>,
  lang: string,
): Map<string, SourceEntry> {
  const out = new Map<string, SourceEntry>();
  const categories = pluralCategories(lang);
  const onlyOne = oneMeansOnlyOne(lang);
  const done = new Set<string>();
  for (const [key, english] of source) {
    const p = pluralParts(key);
    if (!p) {
      out.set(key, { english, from: [key] });
      continue;
    }
    if (done.has(p.base)) continue;
    done.add(p.base);
    const one = source.get(`${p.base}_one`);
    const other = source.get(`${p.base}_other`) ?? english;
    for (const category of categories) {
      // English `_one` is the singular text; it is the source wherever it
      // carries the same placeholders (it then reads right for Russian 21 or
      // French 0 too) or the locale's "one" is 1 alone.
      const fromOne =
        category === "one" &&
        one !== undefined &&
        (onlyOne || samePlaceholders(one, other));
      const text = fromOne ? one : other;
      const hint =
        categories.length > 1
          ? `Plural form "${category}" of this message, used when the count is ${exampleCounts(lang, category).join(", ")}${category === "other" && categories.length > 2 ? " (and every count no other form covers)" : ""}.`
          : undefined;
      out.set(`${p.base}_${category}`, {
        english: text,
        hint,
        from: [...source.keys()].filter((k) => pluralParts(k)?.base === p.base),
      });
    }
  }
  return out;
}

/**
 * Names and key symbols that stay as written in every language
 * (docs/fork/TRANSLATION-REVIEW.md, "Kept as is").
 */
export const KEPT_NAMES = [
  "Wispr Bro",
  "Polish",
  "Prompt Engineer",
  "Apple Intelligence",
  "Hugging Face",
  "GitHub",
  "macOS",
  "fn",
  "⌥",
  "⌘",
  "⌃",
  "⇧",
] as const;

/** Kept names that `english` has and `translated` lost. */
export function lostNames(english: string, translated: string): string[] {
  // Latin names count as whole words ("Polishing" is not the name "Polish";
  // "fn을" in Korean is fn).
  const has = (text: string, name: string) =>
    /^[A-Za-z]/.test(name)
      ? new RegExp(
          `(^|[^A-Za-z])${name}([^A-Za-z]|$)`,
          name === "fn" ? "i" : "",
        ).test(text)
      : text.includes(name);
  return KEPT_NAMES.filter((n) => has(english, n) && !has(translated, n));
}

/** The non-Latin scripts each locale writes in (Latin is always allowed). */
const LOCALE_SCRIPTS: Record<string, string[]> = {
  ar: ["Arabic"],
  he: ["Hebrew"],
  bg: ["Cyrillic"],
  ru: ["Cyrillic"],
  uk: ["Cyrillic"],
  hi: ["Devanagari"],
  ne: ["Devanagari"],
  ja: ["Han", "Hiragana", "Katakana"],
  ko: ["Hangul", "Han"],
  zh: ["Han"],
  "zh-TW": ["Han"],
};
const SCRIPTS = [
  "Arabic",
  "Hebrew",
  "Cyrillic",
  "Devanagari",
  "Han",
  "Hiragana",
  "Katakana",
  "Hangul",
  "Greek",
  "Thai",
];

/** Scripts in `text` that `lang` does not write in ("הذا": Arabic in Hebrew). */
export function foreignScripts(lang: string, text: string): string[] {
  const allowed = LOCALE_SCRIPTS[lang] ?? [];
  return SCRIPTS.filter(
    (script) =>
      !allowed.includes(script) &&
      new RegExp(`\\p{Script=${script}}`, "u").test(text),
  );
}

/** Does `text` have anything to translate (a letter outside placeholders)? */
export function hasWords(text: string): boolean {
  return /\p{L}/u.test(
    text.replace(/\{\{[^}]*\}\}/g, "").replace(/<[^>]*>/g, ""),
  );
}

// ---------------------------------------------------------------------------
// Changed English sources

/**
 * `en/fork.source-hashes.json`: a short hash of every English string as of
 * the last full write run. A key whose English text changed since then is
 * "stale" in every locale: its translation (reviewed or not) means the old
 * sentence. Write mode translates it again (copy-english: an English copy,
 * flagged "untranslated"); `--check` fails until that has happened.
 */
export const SOURCE_HASHES_FILE = path.join(
  LOCALES_DIR,
  SOURCE_LANG,
  "fork.source-hashes.json",
);

export function sourceHash(text: string): string {
  return createHash("sha256").update(text).digest("hex").slice(0, 12);
}

export function sourceHashes(
  source: Map<string, string>,
): Record<string, string> {
  return Object.fromEntries(
    [...source].map(([key, text]) => [key, sourceHash(text)]),
  );
}

/** English keys whose text differs from the recorded hash. */
export function changedSources(
  source: Map<string, string>,
  recorded: Record<string, string>,
): Set<string> {
  return new Set(
    [...source]
      .filter(
        ([key, text]) => key in recorded && recorded[key] !== sourceHash(text),
      )
      .map(([key]) => key),
  );
}

// ---------------------------------------------------------------------------
// One locale

export interface LocaleReport {
  lang: string;
  added: string[];
  removed: string[];
  retranslated: string[];
  /** Keys whose English source changed since the last write run. */
  stale: string[];
  placeholderMismatch: string[];
  untranslated: number;
  machine: number;
  aiReviewed: number;
  overrideFlags: number;
}

/**
 * Brings one locale's strings and flags in line with `expected`. Pure: the
 * caller reads and writes the files. In check mode (`translate` false) the
 * values are not changed and every difference is reported.
 */
export async function syncLocale(
  lang: string,
  expected: Map<string, SourceEntry>,
  current: Map<string, string>,
  flags: Flags,
  backend: TranslationBackend,
  translate: boolean,
  changed: Set<string> = new Set(),
): Promise<{ next: Map<string, string>; flags: Flags; report: LocaleReport }> {
  const report: LocaleReport = {
    lang,
    added: [],
    removed: [],
    retranslated: [],
    stale: [],
    placeholderMismatch: [],
    untranslated: 0,
    machine: 0,
    aiReviewed: 0,
    overrideFlags: 0,
  };
  const nextFlags: Flags = { ...flags };
  const next = new Map<string, string>();
  const fill = async (key: string, src: SourceEntry) => {
    if (!hasWords(src.english)) {
      next.set(key, src.english);
      delete nextFlags[key];
      return;
    }
    let out = await backend.translate(src.english, lang, key, src.hint);
    if (out !== null && !samePlaceholders(out, src.english)) {
      // One retry that names the placeholders the reply must keep.
      const keep = [...src.english.matchAll(/\{\{[^}]*\}\}/g)].map((m) => m[0]);
      const retry = `Your reply must contain exactly these placeholders, written exactly like this: ${keep.join(" ") || "(none)"}.`;
      out = await backend.translate(
        src.english,
        lang,
        key,
        src.hint ? `${src.hint}\n${retry}` : retry,
      );
    }
    const lost = out === null ? [] : lostNames(src.english, out);
    if (out !== null && lost.length) {
      const retry = `Keep these names exactly as written, untranslated: ${lost.map((n) => `"${n}"`).join(", ")}.`;
      const again = await backend.translate(
        src.english,
        lang,
        key,
        src.hint ? `${src.hint}\n${retry}` : retry,
      );
      if (
        again !== null &&
        samePlaceholders(again, src.english) &&
        lostNames(src.english, again).length < lost.length
      ) {
        out = again;
      }
    }
    if (out !== null && foreignScripts(lang, out).length) {
      // A word in another language's script: up to three more tries that
      // say so (sampled, so each differs), else English.
      const retry = `Write only in the target language's own script: no ${foreignScripts(lang, out).join(" or ")} letters.`;
      let clean: string | null = null;
      for (let attempt = 1; attempt <= 3 && clean === null; attempt++) {
        const again = await backend.translate(
          src.english,
          lang,
          key,
          src.hint ? `${src.hint}\n${retry}` : retry,
          attempt,
        );
        if (again !== null && !foreignScripts(lang, again).length)
          clean = again;
      }
      out = clean;
    }
    if (out !== null && samePlaceholders(out, src.english)) {
      next.set(key, out);
      nextFlags[key] = backend.flag;
    } else {
      next.set(key, src.english);
      nextFlags[key] = "untranslated";
    }
  };
  for (const [key, src] of expected) {
    const existing = current.get(key);
    if (existing === undefined) {
      report.added.push(key);
      if (translate) await fill(key, src);
      continue;
    }
    if (src.from.some((k) => changed.has(k))) {
      report.stale.push(key);
      if (translate) await fill(key, src);
      else next.set(key, existing);
      continue;
    }
    if (!samePlaceholders(existing, src.english)) {
      // A never-reviewed entry is simply translated again; a reviewed one
      // needs a person.
      if (
        translate &&
        (nextFlags[key] === "untranslated" || nextFlags[key] === "machine")
      ) {
        report.retranslated.push(key);
        await fill(key, src);
      } else {
        report.placeholderMismatch.push(key);
        next.set(key, existing);
      }
      continue;
    }
    if (!hasWords(src.english) && existing === src.english) {
      next.set(key, existing);
      delete nextFlags[key];
      continue;
    }
    // A translating backend also replaces the English copies and machine
    // output that lost a kept name ("Polish" as the language, "Fn").
    if (
      translate &&
      backend.flag === "machine" &&
      (nextFlags[key] === "untranslated" ||
        (nextFlags[key] === "machine" &&
          (lostNames(src.english, existing).length > 0 ||
            foreignScripts(lang, existing).length > 0)))
    ) {
      report.retranslated.push(key);
      await fill(key, src);
      continue;
    }
    next.set(key, existing);
  }
  for (const key of current.keys()) {
    if (!expected.has(key)) report.removed.push(key);
  }
  const sorted: Flags = {};
  for (const key of expected.keys()) {
    const flag = nextFlags[key];
    if (!flag) continue;
    sorted[key] = flag;
    if (flag === "machine") report.machine++;
    else if (flag === "ai-reviewed") report.aiReviewed++;
    else report.untranslated++;
  }
  for (const key of Object.keys(nextFlags).sort()) {
    if (isOverrideFlag(key)) {
      sorted[key] = nextFlags[key];
      if (nextFlags[key] !== "ai-reviewed") report.overrideFlags++;
    }
  }
  return { next, flags: sorted, report };
}

async function processLocale(
  lang: string,
  source: Map<string, string>,
  backend: TranslationBackend,
  write: boolean,
  changed: Set<string>,
): Promise<LocaleReport> {
  const dir = path.join(LOCALES_DIR, lang);
  const jsonPath = path.join(dir, NAMESPACE_FILE);
  const flagsPath = path.join(dir, FLAGS_FILE);
  const { strings, overrides } = splitOverrides(readJson<Json>(jsonPath, {}));
  const expected = localeSource(source, lang);
  const { next, flags, report } = await syncLocale(
    lang,
    expected,
    flatten(strings),
    readJson<Flags>(flagsPath, {}),
    backend,
    write,
    changed,
  );
  if (write) {
    const out = unflatten([...expected.keys()], next);
    if (overrides !== undefined) out[OVERRIDES_KEY] = overrides;
    writeJson(jsonPath, out);
    writeJson(flagsPath, flags);
  }
  return report;
}

// ---------------------------------------------------------------------------
// Strict mode (S-37) and its CI switch

/** The file that turns strict mode on in CI (`I18N_STRICT=on|off`). */
export const STRICT_SWITCH_FILE = path.join(
  __dirname,
  "..",
  "ci",
  "i18n-strict",
);

/** Reads `I18N_STRICT=on|off` from the switch file's text. */
export function strictSwitchOn(text: string): boolean {
  const line = text
    .split("\n")
    .map((l) => l.trim())
    .find((l) => l.startsWith("I18N_STRICT="));
  if (!line) throw new Error("ci/i18n-strict has no I18N_STRICT= line");
  const value = line.slice("I18N_STRICT=".length);
  if (value !== "on" && value !== "off") {
    throw new Error(
      `ci/i18n-strict: I18N_STRICT must be on or off, not ${value}`,
    );
  }
  return value === "on";
}

/** Locales whose flags fail strict mode, as report lines. */
export function strictFailures(reports: LocaleReport[]): string[] {
  return reports
    .filter((r) => r.untranslated + r.machine + r.overrideFlags > 0)
    .map(
      (r) =>
        `${r.lang}: ${r.untranslated} untranslated, ${r.machine} machine, ${r.overrideFlags} brand overrides flagged`,
    );
}

/**
 * `--review <lang>`: every flagged key of one locale as tab-separated
 * `key  flag  English source  current value` lines, the reviewer's worklist.
 */
function printReview(lang: string): void {
  const source = flatten(
    splitOverrides(
      readJson<Json>(path.join(LOCALES_DIR, SOURCE_LANG, NAMESPACE_FILE), {}),
    ).strings,
  );
  const expected = localeSource(source, lang);
  const dir = path.join(LOCALES_DIR, lang);
  const current = flatten(
    splitOverrides(readJson<Json>(path.join(dir, NAMESPACE_FILE), {})).strings,
  );
  const flags = readJson<Flags>(path.join(dir, FLAGS_FILE), {});
  const one = (s: string | undefined) => (s ?? "").replace(/\s+/g, " ");
  for (const [key, flag] of Object.entries(flags)) {
    if (isOverrideFlag(key)) continue;
    const src = expected.get(key);
    console.log(
      [
        key,
        flag,
        one(src?.english),
        one(current.get(key)),
        one(src?.hint),
      ].join("\t"),
    );
  }
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const value = (flag: string) => {
    const i = args.indexOf(flag);
    return i >= 0 ? args[i + 1] : undefined;
  };
  const reviewLang = value("--review");
  if (reviewLang) {
    printReview(reviewLang);
    return;
  }
  let strict = args.includes("--strict");
  const strictSwitch = args.includes("--strict-switch");
  if (strictSwitch) {
    strict = strictSwitchOn(fs.readFileSync(STRICT_SWITCH_FILE, "utf8"));
  }
  const check = args.includes("--check") || strict || strictSwitch;
  const only = value("--locale")?.split(",");
  const backendIndex = args.indexOf("--backend");
  const backendName =
    backendIndex >= 0 ? args[backendIndex + 1] : copyEnglishBackend.name;
  let backend: TranslationBackend | undefined = BACKENDS[backendName];
  if (backendName === "llama-server" && !check) {
    const server = value("--server");
    const model = value("--model");
    if (!server || !model) {
      console.error(
        "--backend llama-server needs --server <path> and --model <gguf>",
      );
      process.exit(2);
    }
    backend = await startLlamaServerBackend(server, model);
  } else if (backendName === "llama-server") {
    backend = copyEnglishBackend; // --check never translates
  }
  if (!backend) {
    console.error(
      `Unknown backend "${backendName}". Available: ${[...Object.keys(BACKENDS), "llama-server"].join(", ")}`,
    );
    process.exit(2);
  }

  const source = flatten(
    splitOverrides(
      readJson<Json>(path.join(LOCALES_DIR, SOURCE_LANG, NAMESPACE_FILE), {}),
    ).strings,
  );
  for (const [key, value] of source) {
    if (!value.trim()) throw new Error(`en/${NAMESPACE_FILE}: ${key} is empty`);
  }

  const langs = fs
    .readdirSync(LOCALES_DIR, { withFileTypes: true })
    .filter((e) => e.isDirectory() && e.name !== SOURCE_LANG)
    .map((e) => e.name)
    .filter((name) => !only || only.includes(name))
    .sort();

  let drift = false;
  for (const problem of englishPluralProblems(source)) {
    drift = true;
    console.log(`✗ en: ${problem}`);
  }
  for (const lang of [SOURCE_LANG, ...langs]) {
    for (const name of [NAMESPACE_FILE, FLAGS_FILE]) {
      const file = path.join(LOCALES_DIR, lang, name);
      if (!fs.existsSync(file)) continue;
      const dups = duplicateKeys(fs.readFileSync(file, "utf8"));
      if (dups.length) {
        drift = true;
        console.log(`✗ ${lang}/${name}: duplicate keys ${dups.join(", ")}`);
      }
    }
  }
  const recorded = readJson<Record<string, string>>(SOURCE_HASHES_FILE, {});
  const changed = changedSources(source, recorded);
  const hashes = sourceHashes(source);
  const hashesCurrent = JSON.stringify(recorded) === JSON.stringify(hashes);
  const reports: LocaleReport[] = [];
  for (const lang of langs) {
    const r = await processLocale(lang, source, backend, !check, changed);
    reports.push(r);
    const problems =
      r.added.length +
      r.removed.length +
      r.stale.length +
      r.placeholderMismatch.length;
    // In write mode missing and extra keys are fixed; placeholder mismatches
    // in reviewed translations always need a person.
    const unresolved = check ? problems : r.placeholderMismatch.length;
    if (unresolved > 0) drift = true;
    const parts = [
      r.added.length && `${check ? "missing" : "added"} ${r.added.length}`,
      r.removed.length && `${check ? "extra" : "removed"} ${r.removed.length}`,
      r.stale.length &&
        `${check ? "English changed" : "re-translated (English changed)"} ${r.stale.length}`,
      r.retranslated.length && `re-translated ${r.retranslated.length}`,
      r.placeholderMismatch.length &&
        `placeholder mismatch: ${r.placeholderMismatch.join(", ")}`,
      `${r.untranslated} untranslated, ${r.machine} machine, ${r.aiReviewed} ai-reviewed`,
    ].filter(Boolean);
    console.log(`${unresolved ? "✗" : "✓"} ${lang}: ${parts.join(", ")}`);
  }

  await backend.close?.();
  if (!hashesCurrent) {
    if (check) {
      drift = true;
      console.log(
        `✗ en/fork.source-hashes.json is out of date: run bun run wb:translate`,
      );
    } else if (!only && !drift) {
      // Only a full, clean run records the English it translated from.
      writeJson(SOURCE_HASHES_FILE, hashes);
    }
  }
  console.log(
    `\n${langs.length} locales, ${source.size} keys, backend ${backend.name}${check ? " (check only)" : ""}`,
  );
  const flagged = strictFailures(reports);
  if (strict && flagged.length) {
    drift = true;
    console.log(
      `\n✗ strict: flagged strings are left (a person reviews them, docs/fork/TRANSLATION-REVIEW.md):\n  ${flagged.join("\n  ")}`,
    );
  } else if (strictSwitch) {
    console.log(
      `\nstrict i18n is off (ci/i18n-strict); ${flagged.length} locales still have flags.`,
    );
  }
  if (drift) process.exit(1);
}

if (import.meta.main) {
  await main();
}
