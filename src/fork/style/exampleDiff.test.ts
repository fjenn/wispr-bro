// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/style/exampleDiff.test.ts
import assert from "node:assert/strict";
import { diffFromFormal, markedCount } from "./exampleDiff";
import { EXAMPLES } from "./presets";

const text = (parts: ReturnType<typeof diffFromFormal>) =>
  parts
    .filter((p) => p.kind !== "removed")
    .map((p) => p.text)
    .join("");

// Relaxed: the commas and the final period are dropped.
const relaxed = diffFromFormal(
  "Hey Sam, free? Pizza, my treat.",
  "Hey Sam free? Pizza my treat",
);
assert.deepEqual(
  relaxed.filter((p) => p.kind === "removed").map((p) => p.text),
  [",", ",", "."],
);
assert.equal(text(relaxed), "Hey Sam free? Pizza my treat");

// Very casual: the sentence starts change case.
const lower = diffFromFormal("Hey Sam. I'm in.", "hey Sam. i'm in.");
assert.deepEqual(
  lower.filter((p) => p.kind === "changed").map((p) => p.text),
  ["h", "i"],
);

// Upbeat: the final period becomes "!".
const upbeat = diffFromFormal("Great work.", "Great work!");
assert.deepEqual(upbeat, [
  { kind: "same", text: "Great work" },
  { kind: "changed", text: "!" },
]);
assert.equal(markedCount(diffFromFormal("Same.", "Same.")), 0);

// Every card on the Style page marks something, and its text is intact.
for (const [group, example] of Object.entries(EXAMPLES)) {
  const formal = example.after.formal!;
  for (const [preset, after] of Object.entries(example.after)) {
    if (preset === "formal") continue;
    const parts = diffFromFormal(formal, after!);
    assert.equal(text(parts), after, `${group} ${preset}`);
    assert.ok(markedCount(parts) >= 1, `${group} ${preset} marks nothing`);
  }
}

console.log("exampleDiff.test.ts: ok");
