// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/dictionary/suggestions.test.ts
// flowmatch: the Dictionary banner's chips come from names and terms in
// recent dictations that are not in the dictionary yet.
import assert from "node:assert/strict";
import { suggestWords } from "./suggestions";

const texts = [
  "Can you send the deck to Siobhán before Friday?",
  "I told Siobhán we use Kubernetes now. Then we met Anna-Lena.",
  "The iOS build broke again on Monday",
  "Hello there. We said ok.",
];

assert.deepEqual(suggestWords(texts, []), [
  "Siobhán",
  "Anna-Lena",
  "iOS",
  "Kubernetes",
]);
// Words already in the dictionary are left out, case-insensitively.
assert.deepEqual(suggestWords(texts, ["siobhán", "Kubernetes"], 2), [
  "Anna-Lena",
  "iOS",
]);
// Sentence starts, weekdays and "I" are not names.
assert.deepEqual(
  suggestWords(["Hello. Today I go. On Monday we rest."], []),
  [],
);
console.log("suggestions: ok");
