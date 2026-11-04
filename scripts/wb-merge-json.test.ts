// SPDX-License-Identifier: GPL-3.0-or-later
// The locale merge driver (`wb-merge-json.py`): three-way JSON merges, and
// flat files (fork.flags.json, en/fork.source-hashes.json) written in the
// sorted order `wb-translate-missing.ts` writes, so a merge of two branches
// that both added strings leaves no key-order churn behind.
import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { sortedKeys, sourceHashes } from "./wb-translate-missing";

const DRIVER = path.join(__dirname, "wb-merge-json.py");

function merge(
  base: unknown,
  ours: unknown,
  theirs: unknown,
): { status: number | null; text: string; stderr: string } {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "wb-merge-json-"));
  const files = ["base", "ours", "theirs"].map((n) => path.join(dir, n));
  [base, ours, theirs].forEach((v, i) =>
    fs.writeFileSync(files[i], JSON.stringify(v, null, 2) + "\n"),
  );
  const r = spawnSync("python3", [DRIVER, ...files], { encoding: "utf8" });
  const text = fs.readFileSync(files[1], "utf8");
  fs.rmSync(dir, { recursive: true, force: true });
  return { status: r.status, text, stderr: r.stderr };
}

describe("wb-merge-json", () => {
  test("flat files come out sorted, as the translate script writes them", () => {
    const base = { "b.one": "ai-reviewed", "d.four": "ai-reviewed" };
    // Each branch appended its new keys at the end, unsorted.
    const ours = { ...base, "z.ours": "ai-reviewed", "c.ours": "machine" };
    const theirs = { ...base, "a.theirs": "ai-reviewed" };
    const r = merge(base, ours, theirs);
    expect(r.status).toBe(0);
    const merged = JSON.parse(r.text) as Record<string, string>;
    expect(Object.keys(merged)).toEqual([
      "a.theirs",
      "b.one",
      "c.ours",
      "d.four",
      "z.ours",
    ]);
    // Byte for byte what the translate script would write for the result.
    expect(r.text).toBe(JSON.stringify(sortedKeys(merged), null, 2) + "\n");
  });

  test("merged source hashes match a fresh write of the merged English", () => {
    const english = new Map([
      ["home.title", "Home"],
      ["aiEngine.name", "AI engine"],
      ["lessons.whisper.title", "Whisper"],
    ]);
    const all = sourceHashes(english);
    const base = sourceHashes(new Map([["home.title", "Home"]]));
    const ours = {
      ...base,
      "lessons.whisper.title": all["lessons.whisper.title"],
    };
    const theirs = { ...base, "aiEngine.name": all["aiEngine.name"] };
    const r = merge(base, ours, theirs);
    expect(r.status).toBe(0);
    expect(r.text).toBe(JSON.stringify(all, null, 2) + "\n");
  });

  test("deletions on one side and conflicts behave as before", () => {
    const base = { a: "1", b: "2", c: "3" };
    const r = merge(base, { a: "1", c: "3" }, { a: "1", b: "2", c: "x" });
    expect(r.status).toBe(0);
    expect(JSON.parse(r.text)).toEqual({ a: "1", c: "x" });
    const clash = merge(base, { ...base, a: "ours" }, { ...base, a: "theirs" });
    expect(clash.status).toBe(1);
    expect(JSON.parse(clash.text).a).toBe("ours");
    expect(clash.stderr).toContain("both sides changed a");
  });

  test("nested fork.json keeps ours' order with theirs' keys appended", () => {
    const base = { home: { title: "Home" } };
    const ours = { home: { title: "Home", zeta: "Z" } };
    const theirs = { home: { title: "Home", alpha: "A" } };
    const r = merge(base, ours, theirs);
    expect(r.status).toBe(0);
    expect(Object.keys(JSON.parse(r.text).home)).toEqual([
      "title",
      "zeta",
      "alpha",
    ]);
  });

  test("the committed flat files are already in that order", () => {
    const root = path.join(__dirname, "..", "src", "i18n", "locales");
    const files = [path.join(root, "en", "fork.source-hashes.json")];
    for (const lang of fs.readdirSync(root)) {
      const f = path.join(root, lang, "fork.flags.json");
      if (fs.existsSync(f)) files.push(f);
    }
    expect(files.length).toBeGreaterThan(20);
    for (const f of files) {
      const text = fs.readFileSync(f, "utf8");
      expect(
        text,
        `${path.relative(root, f)} is not sorted (run bun run wb:translate)`,
      ).toBe(JSON.stringify(sortedKeys(JSON.parse(text)), null, 2) + "\n");
    }
  });
});
