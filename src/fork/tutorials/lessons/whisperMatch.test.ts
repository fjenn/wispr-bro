// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/tutorials/lessons/whisperMatch.test.ts
// S-32: the whisper practice's success threshold (≥ 70 % of the words).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  matchWords,
  WHISPER_PASS,
  whisperPasses,
  wordMatch,
} from "./whisperMatch";

const en = JSON.parse(
  readFileSync(
    new URL("../../../i18n/locales/en/fork.json", import.meta.url),
    "utf8",
  ),
);
const TARGET: string = en.lessons.whisper.practice.sentence;
const words = matchWords(TARGET);
assert.ok(words.length >= 10, "the sentence is long enough to grade");

// Normalisation: case, punctuation, apostrophes, accents.
assert.deepEqual(matchWords("Hi Sam, I’m here."), ["hi", "sam", "im", "here"]);
assert.deepEqual(matchWords("Café NAÏVE"), ["cafe", "naive"]);
assert.deepEqual(matchWords("  "), []);
// Unspaced scripts compare per character.
assert.deepEqual(matchWords("静かに話す"), ["静", "か", "に", "話", "す"]);

// The exact sentence, with other punctuation, and with extra words.
assert.equal(wordMatch(TARGET, TARGET), 1);
assert.equal(wordMatch(TARGET, TARGET.toUpperCase().replace(/[,.?]/g, "")), 1);
assert.ok(whisperPasses(TARGET, `um so ${TARGET} okay`));

// The threshold itself: exactly 70 % passes, one word fewer does not.
const need = Math.ceil(words.length * WHISPER_PASS);
const heard = (n: number) => words.slice(0, n).join(" ");
assert.ok(wordMatch(TARGET, heard(need)) >= WHISPER_PASS);
assert.ok(whisperPasses(TARGET, heard(need)));
assert.ok(!whisperPasses(TARGET, heard(need - 1)));

// A synthetic 10-word case pins the boundary independent of the copy.
const ten = "one two three four five six seven eight nine ten";
assert.ok(whisperPasses(ten, "one two three four five six seven"));
assert.ok(!whisperPasses(ten, "one two three four five six"));

// Repeated words count once per hearing.
assert.equal(wordMatch("the cat and the dog", "the cat and dog"), 0.8);
assert.equal(wordMatch("a a a a", "a"), 0.25);

// Nothing heard, or nothing to match.
assert.equal(wordMatch(TARGET, ""), 0);
assert.ok(!whisperPasses(TARGET, ""));
assert.equal(wordMatch("", "anything"), 0);

// A typical soft-speech miss: the ends of phrases dropped.
const dropped = words.filter((_, i) => i % 3 !== 2).join(" ");
assert.ok(wordMatch(TARGET, dropped) < WHISPER_PASS);

console.log("whisperMatch: ok");
