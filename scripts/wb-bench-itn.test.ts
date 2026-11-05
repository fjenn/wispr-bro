// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, test } from "bun:test";
import fs from "fs";
import path from "path";
import { classify, expressions, spokenText } from "./wb-bench-itn";

describe("RS-13 corpus markup", () => {
  test("strips markup for say and lists expressions", () => {
    const s =
      "We need [fifteen hundred|cardinal|1500] units by [twenty twenty six|year|2026].";
    expect(spokenText(s)).toBe(
      "We need fifteen hundred units by twenty twenty six.",
    );
    expect(expressions(s).map((e) => e.digits)).toEqual(["1500", "2026"]);
  });

  test("every corpus item has at least one expression", () => {
    const corpus = JSON.parse(
      fs.readFileSync(
        path.join(import.meta.dir, "..", "eval", "itn", "corpus.json"),
        "utf8",
      ),
    ) as { items: { id: string; lang: string; say: string }[] };
    for (const lang of ["en", "fr", "nl"]) {
      expect(
        corpus.items.filter((i) => i.lang === lang).length,
      ).toBeGreaterThanOrEqual(20);
    }
    for (const item of corpus.items) {
      expect(expressions(item.say).length).toBeGreaterThan(0);
      expect(spokenText(item.say)).not.toContain("[");
    }
  });
});

describe("classify", () => {
  const exprs = expressions(
    "Send me [two hundred fifty|money|250] dollars at [nine thirty|time|930].",
  );
  test("digits with separators count as written", () => {
    expect(classify("Send me $250 at 9:30.", exprs, "en")).toEqual([
      "written",
      "written",
    ]);
  });
  test("number words count as spoken, joiners and hyphens ignored", () => {
    expect(
      classify(
        "Send me two hundred and fifty dollars at nine-thirty.",
        exprs,
        "en",
      ),
    ).toEqual(["spoken", "spoken"]);
  });
  test("anything else is other", () => {
    expect(classify("Send me some money later.", exprs, "en")).toEqual([
      "other",
      "other",
    ]);
  });
  test("Dutch compounds match split or joined, accents folded", () => {
    const nl = expressions("[tweeënveertig|cardinal|42] mensen");
    expect(classify("twee en veertig mensen", nl, "nl")).toEqual(["spoken"]);
    expect(classify("tweeenveertig mensen", nl, "nl")).toEqual(["spoken"]);
    expect(classify("Tweeënveertig mensen", nl, "nl")).toEqual(["spoken"]);
    expect(classify("42 mensen", nl, "nl")).toEqual(["written"]);
  });
  test("two expressions cannot share one match", () => {
    const two = expressions(
      "[twenty twenty three|year|2023] and [twenty twenty three|year|2023]",
    );
    expect(classify("2023 and twenty twenty three", two, "en")).toEqual([
      "written",
      "spoken",
    ]);
  });
});

describe("owner markup", () => {
  test("matches the RS-09 reading script sentences word for word", () => {
    const root = path.join(import.meta.dir, "..");
    const corpus = JSON.parse(
      fs.readFileSync(path.join(root, "eval", "itn", "corpus.json"), "utf8"),
    ) as { owner: { items: Record<string, string> } };
    const script = fs.readFileSync(
      path.join(root, "eval", "asr", "READING-SCRIPT.md"),
      "utf8",
    );
    for (const say of Object.values(corpus.owner.items)) {
      expect(script).toContain(spokenText(say));
    }
  });
});
