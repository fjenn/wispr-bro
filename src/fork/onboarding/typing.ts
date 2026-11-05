// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The typing test's and the time-saved estimate's math (S-29b, W-18).
 * Pure, so `typing.test.ts` checks it without a browser.
 *
 * - Words per minute: the usual typing-test word of five characters,
 *   counting only characters typed correctly (in place against the sample),
 *   over the time since the first key press. A typo therefore lowers the
 *   number instead of being ignored.
 * - Time saved (DEC-50): of the hours a week spent typing (the slider,
 *   4 h by default, owner's rc9 call; stored per day as the fork setting
 *   `typing_hours_per_day` over a five-day week), the share that speaking
 *   would save. Speaking is taken at
 *   200 words a minute (owner's call, rc8: dictation runs faster than
 *   conversation), so a fast typist sees a smaller number, and nobody who
 *   types at 200 or faster is promised any saving. The typing speed until
 *   the test is taken is 45 words a minute.
 */

/** Characters per typing-test word. */
export const CHARS_PER_WORD = 5;
/** Length of the typing test. */
export const TEST_SECONDS = 30;
/** Below this the live number would jump around: show none yet. */
export const MIN_LIVE_SECONDS = 2;
/** The speaking rate the estimate assumes (see above). */
export const SPEAKING_WPM = 200;
/** Typing speed assumed until the test is taken (S-25 uses the same). */
export const DEFAULT_TYPING_WPM = 45;
/** Working days a week. */
export const WORKDAYS = 5;
/** The fork setting's bound (`MAX_TYPING_WPM` in `fork/settings.rs`). */
export const MAX_TYPING_WPM = 300;
/** The slider's range and step, hours of typing a week. */
export const HOURS_MIN = 1;
export const HOURS_MAX = 40;
export const HOURS_STEP = 1;
export const DEFAULT_HOURS = 4;

/** The stored setting (hours a day) as the slider's hours a week. */
export function weeklyHours(hoursPerDay: number | null | undefined): number {
  return clampHours(hoursPerDay == null ? null : hoursPerDay * WORKDAYS);
}

/** The slider's hours a week as the stored setting (hours a day). */
export function dailyHours(hoursPerWeek: number): number {
  return Math.round((hoursPerWeek / WORKDAYS) * 100) / 100;
}

/** Characters of `typed` that match `sample` at the same place. */
export function correctChars(sample: string, typed: string): number {
  const a = Array.from(sample);
  const b = Array.from(typed);
  let n = 0;
  for (let i = 0; i < b.length && i < a.length; i++) {
    if (a[i] === b[i]) n++;
  }
  return n;
}

/** Words per minute after `seconds` of typing (0 before any time passed). */
export function wordsPerMinute(
  sample: string,
  typed: string,
  seconds: number,
): number {
  if (!(seconds > 0)) return 0;
  const words = correctChars(sample, typed) / CHARS_PER_WORD;
  return Math.round(words / (seconds / 60));
}

/** A result the fork setting accepts (1..MAX), or `null` (nothing typed). */
export function savableWpm(wpm: number): number | null {
  if (!Number.isFinite(wpm) || wpm < 1) return null;
  return Math.min(Math.round(wpm), MAX_TYPING_WPM);
}

/**
 * Hours a week dictating could save, rounded to the half hour (an estimate
 * is not more precise than that).
 */
export function hoursSavedPerWeek(
  typingWpm: number,
  hoursPerWeek: number,
): number {
  if (!(typingWpm > 0) || !(hoursPerWeek > 0)) return 0;
  const share = Math.max(0, 1 - typingWpm / SPEAKING_WPM);
  return Math.round(hoursPerWeek * share * 2) / 2;
}

/** How many times faster speaking is than `typingWpm`, to one decimal. */
export function timesFaster(typingWpm: number): number {
  if (!(typingWpm > 0)) return 0;
  return Math.round((SPEAKING_WPM / typingWpm) * 10) / 10;
}

/** Keeps a slider value (hours a week) on the slider's steps. */
export function clampHours(hours: number | null | undefined): number {
  if (hours == null || !Number.isFinite(hours)) return DEFAULT_HOURS;
  const stepped = Math.round(hours / HOURS_STEP) * HOURS_STEP;
  return Math.min(HOURS_MAX, Math.max(HOURS_MIN, stepped));
}
