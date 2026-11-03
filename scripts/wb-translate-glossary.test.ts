// SPDX-License-Identifier: GPL-3.0-or-later
// The glossary covers every locale, and the review guide prints it (S-37b).
import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "fs";
import path from "path";
import { GLOSSARY, TERMS } from "./wb-translate-glossary";

const ROOT = path.join(import.meta.dir, "..");

describe("translation glossary", () => {
  test("every non-English locale has a tone and every term", () => {
    const locales = readdirSync(path.join(ROOT, "src/i18n/locales"))
      .filter((l) => l !== "en")
      .sort();
    expect(Object.keys(GLOSSARY).sort()).toEqual(locales);
    for (const [code, entry] of Object.entries(GLOSSARY)) {
      expect(entry.tone.length, code).toBeGreaterThan(10);
      for (const term of TERMS)
        expect(entry.terms[term], `${code} ${term}`).toBeTruthy();
    }
  });

  test("docs/fork/TRANSLATION-REVIEW.md shows the same terms and tone", () => {
    const doc = readFileSync(
      path.join(ROOT, "docs/fork/TRANSLATION-REVIEW.md"),
      "utf8",
    );
    for (const [code, entry] of Object.entries(GLOSSARY)) {
      const rows = doc.split("\n").filter((l) => l.startsWith(`| \`${code}\``));
      const cells = rows.flatMap((r) => r.split("|").map((c) => c.trim()));
      for (const term of TERMS) {
        expect(cells, `${code} ${term}`).toContain(entry.terms[term]);
      }
      expect(cells, `${code} tone`).toContain(entry.tone);
    }
  });
});
