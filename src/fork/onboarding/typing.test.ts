// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/onboarding/typing.test.ts
import assert from "node:assert/strict";
import {
  clampHours,
  correctChars,
  dailyHours,
  DEFAULT_HOURS,
  hoursSavedPerWeek,
  timesFaster,
  weeklyHours,
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

// rc9 default: 4 h of typing a week at 45 wpm → 4 × (1 − 45/200) = 3.1 → 3.
assert.equal(hoursSavedPerWeek(45, DEFAULT_HOURS), 3);
assert.equal(DEFAULT_HOURS, 4);
// 10 h a week (the old 2 h a day) at 45 wpm → 7.75 → 8.
assert.equal(hoursSavedPerWeek(45, 10), 8);
// A faster typist saves less; at or above speaking speed nothing.
assert.equal(hoursSavedPerWeek(80, 10), 6);
assert.equal(hoursSavedPerWeek(200, 10), 0);
assert.equal(hoursSavedPerWeek(250, 40), 0);
// More typing time, more saved; none without typing.
assert.ok(hoursSavedPerWeek(40, 20) > hoursSavedPerWeek(40, 10));
assert.equal(hoursSavedPerWeek(40, 0), 0);
assert.equal(hoursSavedPerWeek(0, 10), 0);

// The setting is stored per day over a five-day week.
assert.equal(clampHours(undefined), DEFAULT_HOURS);
assert.equal(clampHours(2.2), 2);
assert.equal(clampHours(0), 1);
assert.equal(clampHours(99), 40);
assert.equal(weeklyHours(undefined), 4);
assert.equal(weeklyHours(0.8), 4);
assert.equal(weeklyHours(2), 10);
assert.equal(dailyHours(4), 0.8);
assert.equal(dailyHours(10), 2);

// The celebration's multiple: 200 / 45 = 4.4; a measured 80 wpm → 2.5.
assert.equal(timesFaster(45), 4.4);
assert.equal(timesFaster(80), 2.5);
assert.equal(timesFaster(0), 0);

console.log("typing.test.ts: ok");
