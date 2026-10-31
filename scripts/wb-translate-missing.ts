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

export function placeholders(text: string): string[] {
  return [...text.matchAll(/\{\{\s*([^}\s]+)\s*\}\}/g)].map((m) => m[1]).sort();
}

function samePlaceholders(a: string, b: string): boolean {
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
  const { strings, overrides } = splitOverrides(readJson<Json>(jsonPath, {}));
  const current = flatten(strings);
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
    if (!source.has(key) && !isOverrideFlag(key)) delete flags[key];
  }
  const sortedFlags: Flags = {};
  for (const key of source.keys()) {
    if (flags[key]) sortedFlags[key] = flags[key];
  }
  report.flagged = Object.keys(sortedFlags).length;
  for (const key of Object.keys(flags).sort()) {
    if (isOverrideFlag(key)) sortedFlags[key] = flags[key];
  }

  if (write) {
    const out = unflatten([...source.keys()], next);
    if (overrides !== undefined) out[OVERRIDES_KEY] = overrides;
    writeJson(jsonPath, out);
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
  const value = (flag: string) => {
    const i = args.indexOf(flag);
    return i >= 0 ? args[i + 1] : undefined;
  };
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

  await backend.close?.();
  console.log(
    `\n${langs.length} locales, ${source.size} keys, backend ${backend.name}${check ? " (check only)" : ""}`,
  );
  if (drift) process.exit(1);
}

if (import.meta.main) {
  await main();
}
