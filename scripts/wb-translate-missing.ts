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
 *
 * Marking scheme (documented in docs/fork/ARCHITECTURE.md, "i18n"):
 *   Every locale may have a sidecar `fork.flags.json` next to its `fork.json`:
 *     { "debug.title": "untranslated", "debug.close": "machine" }
 *   Keys are dotted paths into `fork.json`; values are
 *     - "untranslated": the value is a copy of the English string,
 *     - "machine": the value was machine-translated and awaits review.
 *   A key that a person has reviewed has no flag. `fork.json` itself stays a
 *   plain i18next resource file (the same shape as English), so i18next,
 *   Handy's `check-translations.ts` (which reads only `translation.json`) and
 *   ESLint never see the flags. Flags pass CI until S-37 (R5 §4.12).
 *
 * The script also:
 *   - drops keys that no longer exist in English (and their flags),
 *   - checks that `{{placeholders}}` match English for every key; a backend
 *     result with different placeholders is rejected and the English copy is
 *     kept, flagged "untranslated",
 *   - never touches Handy's `translation.json` files.
 *
 * Backends are pluggable (`TranslationBackend`). Today: "copy-english".
 * S-10 adds a local `llama-server` backend (flag "machine"); no network
 * backend may be added (R5 §1 privacy rule).
 */
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const LOCALES_DIR = path.join(__dirname, "..", "src", "i18n", "locales");
const SOURCE_LANG = "en";
const NAMESPACE_FILE = "fork.json";
const FLAGS_FILE = "fork.flags.json";

type Json = { [key: string]: string | Json };
type Flag = "untranslated" | "machine";
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
  ): Promise<string | null>;
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

export function placeholders(text: string): string[] {
  return [...text.matchAll(/\{\{\s*([^}\s]+)\s*\}\}/g)].map((m) => m[1]).sort();
}

function samePlaceholders(a: string, b: string): boolean {
  return placeholders(a).join("\u0000") === placeholders(b).join("\u0000");
}

function readJson<T>(file: string, fallback: T): T {
  if (!fs.existsSync(file)) return fallback;
  return JSON.parse(fs.readFileSync(file, "utf8")) as T;
}

function writeJson(file: string, data: unknown): void {
  fs.writeFileSync(file, JSON.stringify(data, null, 2) + "\n");
}

interface LocaleReport {
  lang: string;
  added: string[];
  removed: string[];
  placeholderMismatch: string[];
  flagged: number;
}

async function processLocale(
  lang: string,
  source: Map<string, string>,
  backend: TranslationBackend,
  write: boolean,
): Promise<LocaleReport> {
  const dir = path.join(LOCALES_DIR, lang);
  const jsonPath = path.join(dir, NAMESPACE_FILE);
  const flagsPath = path.join(dir, FLAGS_FILE);
  const current = flatten(readJson<Json>(jsonPath, {}));
  const flags = readJson<Flags>(flagsPath, {});
  const report: LocaleReport = {
    lang,
    added: [],
    removed: [],
    placeholderMismatch: [],
    flagged: 0,
  };

  const next = new Map<string, string>();
  for (const [key, english] of source) {
    const existing = current.get(key);
    if (existing !== undefined) {
      if (!samePlaceholders(existing, english)) {
        report.placeholderMismatch.push(key);
      }
      next.set(key, existing);
      continue;
    }
    report.added.push(key);
    const translated = await backend.translate(english, lang, key);
    if (translated !== null && samePlaceholders(translated, english)) {
      next.set(key, translated);
      flags[key] = backend.flag;
    } else {
      next.set(key, english);
      flags[key] = "untranslated";
    }
  }
  for (const key of current.keys()) {
    if (!source.has(key)) report.removed.push(key);
  }
  for (const key of Object.keys(flags)) {
    if (!source.has(key)) delete flags[key];
  }
  const sortedFlags: Flags = {};
  for (const key of source.keys()) {
    if (flags[key]) sortedFlags[key] = flags[key];
  }
  report.flagged = Object.keys(sortedFlags).length;

  if (write) {
    writeJson(jsonPath, unflatten([...source.keys()], next));
    writeJson(flagsPath, sortedFlags);
  }
  return report;
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const check = args.includes("--check");
  const backendIndex = args.indexOf("--backend");
  const backendName =
    backendIndex >= 0 ? args[backendIndex + 1] : copyEnglishBackend.name;
  const backend = BACKENDS[backendName];
  if (!backend) {
    console.error(
      `Unknown backend "${backendName}". Available: ${Object.keys(BACKENDS).join(", ")}`,
    );
    process.exit(2);
  }

  const source = flatten(
    readJson<Json>(path.join(LOCALES_DIR, SOURCE_LANG, NAMESPACE_FILE), {}),
  );
  for (const [key, value] of source) {
    if (!value.trim()) throw new Error(`en/${NAMESPACE_FILE}: ${key} is empty`);
  }

  const langs = fs
    .readdirSync(LOCALES_DIR, { withFileTypes: true })
    .filter((e) => e.isDirectory() && e.name !== SOURCE_LANG)
    .map((e) => e.name)
    .sort();

  let drift = false;
  for (const lang of langs) {
    const r = await processLocale(lang, source, backend, !check);
    const problems =
      r.added.length + r.removed.length + r.placeholderMismatch.length;
    // In write mode missing and extra keys are fixed; placeholder mismatches
    // in existing translations always need a person.
    const unresolved = check ? problems : r.placeholderMismatch.length;
    if (unresolved > 0) drift = true;
    const parts = [
      r.added.length && `${check ? "missing" : "added"} ${r.added.length}`,
      r.removed.length && `${check ? "extra" : "removed"} ${r.removed.length}`,
      r.placeholderMismatch.length &&
        `placeholder mismatch: ${r.placeholderMismatch.join(", ")}`,
      `${r.flagged} flagged`,
    ].filter(Boolean);
    console.log(`${unresolved ? "✗" : "✓"} ${lang}: ${parts.join(", ")}`);
  }

  console.log(
    `\n${langs.length} locales, ${source.size} keys, backend ${backend.name}${check ? " (check only)" : ""}`,
  );
  if (drift) process.exit(1);
}

if (import.meta.main) {
  await main();
}
