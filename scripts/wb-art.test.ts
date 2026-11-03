// SPDX-License-Identifier: GPL-3.0-or-later
//
// Design W7: every bundled art file is listed in
// src/assets/fork/art/SHA256SUMS with its current hash, and named in the
// licence register (docs/fork/LICENSE-REGISTER.md §2a), so no picture
// ships without a recorded source and licence.
import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { REPO_ROOT } from "./wb-guard-lib";

const ART = join(REPO_ROOT, "src/assets/fork/art");

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    return statSync(full).isDirectory() ? files(full) : [full];
  });
}

const art = files(ART)
  .map((f) => relative(ART, f))
  .filter((f) => /\.(svg|webp|avif|png|jpe?g)$/.test(f))
  .sort();

describe("art register", () => {
  test("SHA256SUMS lists every art file with its current hash", () => {
    const sums = new Map(
      readFileSync(join(ART, "SHA256SUMS"), "utf8")
        .trim()
        .split("\n")
        .map((l) => {
          const [hash, file] = l.split(/\s+/);
          return [file, hash] as const;
        }),
    );
    expect([...sums.keys()].sort()).toEqual(art);
    for (const f of art) {
      const hash = createHash("sha256")
        .update(readFileSync(join(ART, f)))
        .digest("hex");
      expect({ f, hash }).toEqual({ f, hash: sums.get(f) });
    }
  });

  test("the licence register names every art file", () => {
    const register = readFileSync(
      join(REPO_ROOT, "docs/fork/LICENSE-REGISTER.md"),
      "utf8",
    );
    const missing = art.filter((f) => {
      const stem = f.replace(/@[12]x\.(webp|avif)$/, "").replace(/\.svg$/, "");
      return !register.includes(stem);
    });
    expect(missing).toEqual([]);
  });

  test("photos stay inside the size budget (WebP 2x ≤ 120 KB)", () => {
    for (const f of art.filter((f) => f.endsWith("@2x.webp")))
      expect({ f, kb: statSync(join(ART, f)).size <= 120 * 1024 }).toEqual({
        f,
        kb: true,
      });
  });
});
