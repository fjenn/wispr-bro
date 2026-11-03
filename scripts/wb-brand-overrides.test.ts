// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun test scripts/  (F-01 script test)
import { describe, expect, test } from "bun:test";
import fs from "fs";
import path from "path";
import {
  LOCALES_DIR,
  UPSTREAM_BRAND,
  brandKeys,
  computeLocale,
  flatten,
  handyStrings,
  locales,
  markup,
  readJson,
  rebrand,
} from "./wb-brand-overrides";

type Json = { [key: string]: string | Json };

const english = handyStrings("en");
const englishBrandKeys = [...english]
  .filter(([, v]) => UPSTREAM_BRAND.test(v))
  .map(([k]) => k);

function overridesOf(lng: string): Map<string, string> {
  const fork = readJson<Json>(path.join(LOCALES_DIR, lng, "fork.json"), {});
  return flatten((fork.overrides as Json | undefined) ?? {});
}

describe("brand overrides", () => {
  test("there are 26 locales and English names Handy somewhere", () => {
    expect(locales()).toHaveLength(26);
    expect(englishBrandKeys.length).toBeGreaterThan(0);
  });

  for (const lng of locales()) {
    test(`${lng}: every Handy key has a rebranded override`, () => {
      const local = lng === "en" ? english : handyStrings(lng);
      const overrides = overridesOf(lng);
      // Every key whose English text names Handy, in every locale…
      for (const key of englishBrandKeys) {
        expect(overrides.has(key), `${lng}: ${key}`).toBe(true);
      }
      // …and every key whose text in this locale names Handy.
      for (const key of brandKeys(english, local)) {
        expect(overrides.has(key), `${lng}: ${key}`).toBe(true);
      }
      for (const [key, value] of overrides) {
        expect(UPSTREAM_BRAND.test(value), `${lng}: ${key} = ${value}`).toBe(
          false,
        );
        const base = local.get(key) ?? english.get(key)!;
        expect(markup(value), `${lng}: ${key}`).toBe(markup(base));
      }
    });
  }

  test("Handy's translation.json files are never written", () => {
    // The script only writes fork.json / fork.flags.json.
    const src = fs.readFileSync(
      path.join(import.meta.dir, "wb-brand-overrides.ts"),
      "utf8",
    );
    expect(src).not.toMatch(/writeJson\([^)]*translation\.json/);
  });
});

describe("ai-reviewed overrides", () => {
  test("a regenerate keeps AI-reviewed values and their flags", () => {
    const flags = readJson<Record<string, string>>(
      path.join(LOCALES_DIR, "fr", "fork.flags.json"),
      {},
    );
    const reviewed = Object.keys(flags).filter(
      (k) => k.startsWith("overrides.") && flags[k] === "ai-reviewed",
    );
    expect(reviewed.length).toBeGreaterThan(0);
    const result = computeLocale("fr", english);
    const current = overridesOf("fr");
    for (const flagKey of reviewed) {
      const key = flagKey.slice("overrides.".length);
      if (!result.overrides.has(key)) continue;
      expect(result.overrides.get(key), key).toBe(current.get(key));
      expect(result.flags[flagKey], key).toBe("ai-reviewed");
    }
  });
});

describe("rebrand grammar", () => {
  test("generic forms", () => {
    expect(rebrand("Handy's in your menu bar", "en")).toBe(
      "Wispr Bro's in your menu bar",
    );
    expect(rebrand("Handy-Symbol", "de")).toBe("Wispr Bro-Symbol");
    expect(rebrand("Handys gränssnitt", "sv")).toBe("Wispr Bros gränssnitt");
    expect(rebrand("(HANDY_DISABLE_UPDATER)", "fr")).toBe(
      "(WISPRBRO_DISABLE_UPDATER)",
    );
  });

  test("Turkish suffixes follow vowel harmony for Bro", () => {
    expect(rebrand("Handy'nin modelleri", "tr")).toBe(
      "Wispr Bro'nun modelleri",
    );
    expect(rebrand("Handy'yi başlat", "tr")).toBe("Wispr Bro'yu başlat");
    expect(rebrand("Handy'ye", "tr")).toBe("Wispr Bro'ya");
  });
});
