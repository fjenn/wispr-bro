// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, test } from "bun:test";
import { readFileSync } from "fs";
import {
  duplicateKeys,
  englishPluralProblems,
  foreignScripts,
  hasWords,
  changedSources,
  localeSource,
  sourceHashes,
  lostNames,
  type LocaleReport,
  oneMeansOnlyOne,
  PLURAL_CATEGORIES,
  pluralCategories,
  placeholders,
  samePlaceholders,
  STRICT_SWITCH_FILE,
  strictFailures,
  strictSwitchOn,
  syncLocale,
  type TranslationBackend,
} from "./wb-translate-missing";

describe("duplicateKeys", () => {
  test("finds a key repeated in the same object, with its path", () => {
    const text = `{"a":{"b":1,"c":"x\\"y","b":2},"d":[{"e":1},{"e":2}],"a":3}`;
    expect(duplicateKeys(text)).toEqual(["a.b", "a"]);
  });
  test("same key in sibling objects or array items is fine", () => {
    expect(
      duplicateKeys(`{"x":{"k":1},"y":{"k":2},"z":[{"k":1},{"k":1}]}`),
    ).toEqual([]);
  });
});

describe("plurals", () => {
  const source = new Map([
    ["a.title", "Words"],
    ["a.words_one", "{{count}} word"],
    ["a.words_other", "{{count}} words"],
    ["a.saved_one", "A recording was saved"],
    ["a.saved_other", "{{count}} recordings were saved"],
    ["rust.menu.n_one", "{{count}} snippet"],
    ["rust.menu.n_other", "{{count}} snippets"],
  ]);

  test("CLDR categories come from Intl.PluralRules, in CLDR order", () => {
    expect(pluralCategories("en")).toEqual(["one", "other"]);
    expect(pluralCategories("ja")).toEqual(["other"]);
    expect(pluralCategories("ru")).toEqual(["one", "few", "many", "other"]);
    expect(pluralCategories("pl")).toEqual(["one", "few", "many", "other"]);
    expect(pluralCategories("ar")).toEqual([...PLURAL_CATEGORIES]);
    expect(pluralCategories("he")).toContain("two");
  });

  test("a group expands to the locale's categories, rust.* included", () => {
    expect([...localeSource(source, "ru").keys()]).toEqual([
      "a.title",
      "a.words_one",
      "a.words_few",
      "a.words_many",
      "a.words_other",
      "a.saved_one",
      "a.saved_few",
      "a.saved_many",
      "a.saved_other",
      "rust.menu.n_one",
      "rust.menu.n_few",
      "rust.menu.n_many",
      "rust.menu.n_other",
    ]);
    expect([...localeSource(source, "ja").keys()]).toEqual([
      "a.title",
      "a.words_other",
      "a.saved_other",
      "rust.menu.n_other",
    ]);
  });

  test("'one' is translated from English _one only where it means 1 alone", () => {
    expect(oneMeansOnlyOne("de")).toBe(true);
    expect(oneMeansOnlyOne("ru")).toBe(false); // 21, 31, …
    expect(oneMeansOnlyOne("fr")).toBe(false); // 0
    expect(localeSource(source, "de").get("a.saved_one")?.english).toBe(
      "A recording was saved",
    );
    const ru = localeSource(source, "ru");
    expect(ru.get("a.saved_one")?.english).toBe(
      "{{count}} recordings were saved",
    );
    // With {{count}} in both, the singular text is right for 21 and 0 too.
    expect(ru.get("a.words_one")?.english).toBe("{{count}} word");
    expect(localeSource(source, "fr").get("a.words_one")?.english).toBe(
      "{{count}} word",
    );
    expect(ru.get("a.words_few")?.hint).toContain('"few"');
    expect(ru.get("a.words_few")?.hint).toContain("2, 3, 4, 22");
    expect(
      localeSource(source, "ja").get("a.words_other")?.hint,
    ).toBeUndefined();
  });

  test("English needs exactly _one and _other", () => {
    expect(englishPluralProblems(source)).toEqual([]);
    expect(
      englishPluralProblems(
        new Map([
          ["x_other", "{{count}} x"],
          ["y_one", "1 y"],
          ["y_other", "{{count}} y"],
          ["y_few", "{{count}} y"],
          ["y", "y"],
        ]),
      ),
    ).toEqual([
      "x: English needs exactly _one and _other (has _other)",
      "y: also exists without a suffix",
      "y: English needs exactly _one and _other (has _one, _other, _few)",
    ]);
  });
});

describe("syncLocale", () => {
  const backend = (seen: string[] = []): TranslationBackend => ({
    name: "fake",
    flag: "machine",
    async translate(text, lang, key, hint) {
      seen.push(`${key}${hint ? " (hint)" : ""}`);
      return `[${lang}] ${text}`;
    },
  });
  const source = new Map([
    ["a.words_one", "{{count}} word"],
    ["a.words_other", "{{count}} words"],
    ["a.pair", "{{from}} → {{to}}"],
    ["a.reviewed", "Hello {{name}}"],
  ]);

  test("adds missing plural forms, drops forms the language lacks", async () => {
    const seen: string[] = [];
    const { next, flags, report } = await syncLocale(
      "ru",
      localeSource(source, "ru"),
      new Map([
        ["a.words_one", "{{count}} слово"],
        ["a.words_other", "{{count}} слова"],
        ["a.words_two", "stale"],
        ["a.reviewed", "Привет {{name}}"],
      ]),
      { "a.words_one": "machine" },
      backend(seen),
      true,
    );
    expect(report.added).toEqual(["a.words_few", "a.words_many", "a.pair"]);
    expect(report.removed).toEqual(["a.words_two"]);
    expect(seen).toEqual(["a.words_few (hint)", "a.words_many (hint)"]);
    expect(next.get("a.words_few")).toBe("[ru] {{count}} words");
    // Nothing to translate: copied, never flagged.
    expect(next.get("a.pair")).toBe("{{from}} → {{to}}");
    expect(flags).toEqual({
      "a.words_one": "machine",
      "a.words_few": "machine",
      "a.words_many": "machine",
    });
    expect(report.machine).toBe(3);
  });

  test("a flagged entry with wrong placeholders is translated again; a reviewed one is reported", async () => {
    const current = new Map([
      ["a.words_one", "одно слово"],
      ["a.words_few", "{{count}} слова"],
      ["a.words_many", "{{count}} слов"],
      ["a.words_other", "{{count}} слова"],
      ["a.pair", "{{from}} → {{to}}"],
      ["a.reviewed", "Привет"],
    ]);
    const { next, report } = await syncLocale(
      "ru",
      localeSource(source, "ru"),
      current,
      { "a.words_one": "machine", "a.pair": "machine" },
      backend(),
      true,
    );
    expect(report.retranslated).toEqual(["a.words_one"]);
    expect(next.get("a.words_one")).toBe("[ru] {{count}} word");
    expect(report.placeholderMismatch).toEqual(["a.reviewed"]);
    const check = await syncLocale(
      "ru",
      localeSource(source, "ru"),
      current,
      { "a.words_one": "machine" },
      backend(),
      false,
    );
    expect(check.report.placeholderMismatch).toEqual([
      "a.words_one",
      "a.reviewed",
    ]);
  });

  test("a backend reply with other placeholders falls back to English, untranslated", async () => {
    const bad: TranslationBackend = {
      name: "bad",
      flag: "machine",
      async translate() {
        return "ohne Platzhalter";
      },
    };
    const { next, flags } = await syncLocale(
      "de",
      localeSource(new Map([["k", "Hi {{name}}"]]), "de"),
      new Map(),
      {},
      bad,
      true,
    );
    expect(next.get("k")).toBe("Hi {{name}}");
    expect(flags).toEqual({ k: "untranslated" });
  });

  test("placeholders: formats count, whitespace does not", () => {
    expect(placeholders("{{count, number}} days")).toEqual(["count,number"]);
    expect(samePlaceholders("{{ count,number }}", "{{count, number}}")).toBe(
      true,
    );
    expect(samePlaceholders("{{count}}", "{{count, number}}")).toBe(false);
    expect(placeholders("{{inserted as you said it}}")).toEqual([
      "insertedasyousaidit",
    ]);
    expect(hasWords("{{from}} → {{to}}")).toBe(false);
    expect(hasWords("v{{version}}")).toBe(true);
  });
});

describe("strict mode", () => {
  const report = (lang: string, u: number, m: number, o = 0): LocaleReport => ({
    lang,
    added: [],
    removed: [],
    retranslated: [],
    stale: [],
    placeholderMismatch: [],
    untranslated: u,
    machine: m,
    overrideFlags: o,
  });
  test("fails on any untranslated, machine or brand-override flag", () => {
    expect(
      strictFailures([
        report("de", 0, 0),
        report("fr", 0, 3),
        report("ja", 1, 0),
        report("ko", 0, 0, 2),
      ]),
    ).toEqual([
      "fr: 0 untranslated, 3 machine, 0 brand overrides flagged",
      "ja: 1 untranslated, 0 machine, 0 brand overrides flagged",
      "ko: 0 untranslated, 0 machine, 2 brand overrides flagged",
    ]);
  });
  test("the CI switch reads I18N_STRICT=on|off", () => {
    expect(strictSwitchOn("# comment\nI18N_STRICT=off\n")).toBe(false);
    expect(strictSwitchOn("I18N_STRICT=on")).toBe(true);
    expect(() => strictSwitchOn("I18N_STRICT=yes")).toThrow();
    expect(() => strictSwitchOn("nothing")).toThrow();
  });
  test("the committed switch file parses", () => {
    expect(
      typeof strictSwitchOn(readFileSync(STRICT_SWITCH_FILE, "utf8")),
    ).toBe("boolean");
  });
});

describe("translating backends replace English copies", () => {
  test("untranslated entries go to a machine backend, not to copy-english", async () => {
    const source = new Map([["k", "Ready"]]);
    const fake: TranslationBackend = {
      name: "fake",
      flag: "machine",
      async translate() {
        return "Bereit";
      },
    };
    const current = new Map([["k", "Ready"]]);
    const flags = { k: "untranslated" as const };
    const machine = await syncLocale(
      "de",
      localeSource(source, "de"),
      current,
      flags,
      fake,
      true,
    );
    expect(machine.next.get("k")).toBe("Bereit");
    expect(machine.flags).toEqual({ k: "machine" });
    const copy = await syncLocale(
      "de",
      localeSource(source, "de"),
      current,
      flags,
      { name: "copy", flag: "untranslated", translate: async (t) => t },
      true,
    );
    expect(copy.report.retranslated).toEqual([]);
    expect(copy.flags).toEqual({ k: "untranslated" });
  });
});

describe("kept names", () => {
  test("brand, transform names and key symbols must survive", () => {
    expect(lostNames("Polish selected text", "Полски избран текст")).toEqual([
      "Polish",
    ]);
    expect(lostNames("Hold fn and ⌥", "Halte Fn und ⌥")).toEqual([]);
    expect(lostNames("Hold fn", "Halte die Taste")).toEqual(["fn"]);
    expect(lostNames("Open Wispr Bro", "Ouvrir Wispr Bro")).toEqual([]);
    expect(lostNames("Polishing…", "Pulido…")).toEqual([]);
    expect(lostNames("Press fn", "fn을 누르세요")).toEqual([]);
  });

  test("machine output that lost a name is translated again, with the names spelled out", async () => {
    const hints: (string | undefined)[] = [];
    const fake: TranslationBackend = {
      name: "fake",
      flag: "machine",
      async translate(_t, _l, _k, hint) {
        hints.push(hint);
        return hints.length === 1 ? "Polski tekst" : "Polish: tekst";
      },
    };
    const { next, report } = await syncLocale(
      "pl",
      localeSource(new Map([["k", "Polish text"]]), "pl"),
      new Map([["k", "Polski tekst"]]),
      { k: "machine" },
      fake,
      true,
    );
    expect(report.retranslated).toEqual(["k"]);
    expect(next.get("k")).toBe("Polish: tekst");
    expect(hints[1]).toContain('"Polish"');
  });
});

describe("foreign scripts", () => {
  test("another language's letters in a translation are caught", () => {
    expect(foreignScripts("he", "הذا הוא {{count}} מילים")).toEqual(["Arabic"]);
    expect(foreignScripts("he", "Wispr Bro פתוח")).toEqual([]);
    expect(foreignScripts("ja", "設定を開く")).toEqual([]);
    expect(foreignScripts("pl", "Słowo слово")).toEqual(["Cyrillic"]);
    expect(foreignScripts("ko", "설정 열기")).toEqual([]);
  });
});

describe("changed English sources", () => {
  test("a key whose English text changed is stale in every locale", async () => {
    const before = new Map([
      ["a", "Practice: an email"],
      ["n_one", "{{count}} word"],
      ["n_other", "{{count}} words"],
      ["b", "Same"],
    ]);
    const recorded = sourceHashes(before);
    const after = new Map(before);
    after.set("a", "Practice: a whole email, hands-free");
    after.set("n_other", "{{count}} words dictated");
    after.set("c", "New");
    expect([...changedSources(after, recorded)]).toEqual(["a", "n_other"]);

    const fake: TranslationBackend = {
      name: "fake",
      flag: "machine",
      async translate(t) {
        return `[ru] ${t}`;
      },
    };
    const current = new Map([
      ["a", "Практика: письмо"],
      ["n_one", "{{count}} слово"],
      ["n_few", "{{count}} слова"],
      ["n_many", "{{count}} слов"],
      ["n_other", "{{count}} слова"],
      ["b", "То же"],
    ]);
    const changed = changedSources(after, recorded);
    const check = await syncLocale(
      "ru",
      localeSource(after, "ru"),
      current,
      {},
      fake,
      false,
      changed,
    );
    // Reviewed or not, every form of a changed group is stale.
    expect(check.report.stale).toEqual([
      "a",
      "n_one",
      "n_few",
      "n_many",
      "n_other",
    ]);
    const write = await syncLocale(
      "ru",
      localeSource(after, "ru"),
      current,
      {},
      fake,
      true,
      changed,
    );
    expect(write.next.get("a")).toBe(
      "[ru] Practice: a whole email, hands-free",
    );
    expect(write.next.get("b")).toBe("То же");
    expect(write.flags.a).toBe("machine");
  });
});
