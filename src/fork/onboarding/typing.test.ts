// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/onboarding/typing.test.ts
import assert from "node:assert/strict";
import {
  clampHours,
  correctChars,
  DEFAULT_HOURS,
  hoursSavedPerWeek,
  savableWpm,
  wordsPerMinute,
} from "./typing";

const sample = "The quick brown fox jumps over the lazy dog.";

// Only characters in the right place count.
assert.equal(correctChars(sample, ""), 0);
assert.equal(correctChars(sample, "The quick"), 9);
assert.equal(correctChars(sample, "Teh quick"), 7);
assert.equal(correctChars(sample, sample + " extra"), sample.length);
// Accented characters are one character each.
assert.equal(correctChars("café crème", "café crème"), 10);

// 50 correct characters = 10 words; in 30 s that is 20 words a minute.
const typed50 = "x".repeat(50);
assert.equal(wordsPerMinute(typed50, typed50, 30), 20);
// The same in 15 s is twice as fast.
assert.equal(wordsPerMinute(typed50, typed50, 15), 40);
// No time yet: no number.
assert.equal(wordsPerMinute(typed50, typed50, 0), 0);
// Typos lower the number.
assert.ok(
  wordsPerMinute(sample, "Teh quikc brwon", 10) <
    wordsPerMinute(sample, "The quick brown", 10),
);

assert.equal(savableWpm(0), null);
assert.equal(savableWpm(Number.NaN), null);
assert.equal(savableWpm(42.4), 42);
assert.equal(savableWpm(900), 300);

// rc8 defaults: 2 h a day at 45 wpm → 2 × 5 × (1 − 45/200) = 7.75 → 8.
assert.equal(hoursSavedPerWeek(45, 2), 8);
// A faster typist saves less; at or above speaking speed nothing.
assert.equal(hoursSavedPerWeek(80, 2), 6);
assert.equal(hoursSavedPerWeek(200, 2), 0);
assert.equal(hoursSavedPerWeek(250, 8), 0);
// More typing time, more saved; none without typing.
assert.ok(hoursSavedPerWeek(40, 4) > hoursSavedPerWeek(40, 2));
assert.equal(hoursSavedPerWeek(40, 0), 0);
assert.equal(hoursSavedPerWeek(0, 2), 0);

assert.equal(clampHours(undefined), DEFAULT_HOURS);
assert.equal(clampHours(2.2), 2);
assert.equal(clampHours(0), 0.5);
assert.equal(clampHours(24), 8);

console.log("typing.test.ts: ok");
