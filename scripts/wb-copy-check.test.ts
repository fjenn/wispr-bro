// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, test } from "bun:test";
import {
  check,
  forkStrings,
  quotesIn,
  sharedRun,
  wisprQuotes,
  words,
} from "./wb-copy-check";

describe("wb-copy-check", () => {
  test("normalises case, punctuation, the brand and the Option key", () => {
    expect(words("Flow spells the way you do!")).toEqual([
      "appname",
      "spells",
      "the",
      "way",
      "you",
      "do",
    ]);
    expect(words("{{appName}} spells")).toEqual(["appname", "spells"]);
    expect(words("Wispr Bro’s")).toEqual(["appname", "s"]);
    expect(words("Press ⌥Opt O")).toEqual(["press", "option", "o"]);
  });

  test("five shared words in a row are reused copy; four are not", () => {
    expect(
      sharedRun(
        "{{appName}} spells the way you do",
        "Flow spells the way you do",
      ),
    ).toBe("appname spells the way you do");
    expect(
      sharedRun(
        "Select a text box first, then dictate",
        "Select a textbox first, then dictate",
      ),
    ).toBeNull();
    expect(
      sharedRun(
        "Hold to say something, then let go",
        "Hold to say something short",
      ),
    ).toBeNull();
    expect(
      sharedRun(
        "Practice: write an email hands-free",
        "Write an email hands free",
      ),
    ).toBe("write an email hands free");
  });

  test("quotes come from double quotes with five or more words", () => {
    const md =
      'The bar says "Change hotkeys" and "**Your turn! Try whispering this email**".';
    expect(quotesIn(md, "x").map((q) => q.text)).toEqual([
      "Your turn! Try whispering this email",
    ]);
  });

  test("the allow list silences a key and reports stale entries", () => {
    const strings = new Map([
      ["a", "Transform works anywhere you write"],
      ["b", "Our own words here"],
    ]);
    const quotes = [
      { text: "Transform works anywhere you write", source: "R1" },
    ];
    expect(check(strings, quotes).findings.map((f) => f.key)).toEqual(["a"]);
    const allowed = check(strings, quotes, { a: "reviewed", gone: "old" });
    expect(allowed.findings).toEqual([]);
    expect(allowed.stale).toEqual(["gone"]);
  });

  test("the current tree passes", () => {
    const quotes = wisprQuotes();
    expect(quotes.length).toBeGreaterThan(50);
    const strings = forkStrings();
    expect(strings.has("rust.pill.announce.listening")).toBe(true);
    expect([...strings.keys()].some((k) => k.startsWith("overrides."))).toBe(
      false,
    );
    expect(check(strings, quotes).findings).toEqual([]);
  });
});
