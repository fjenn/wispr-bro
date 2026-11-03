// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * How the microphone meter draws a level (owner test 1). The backend sends
 * a level that is already relative to the room's noise floor, with a fast
 * attack and a smooth release (`fork::audio::dynamics`); this turns it
 * into bars:
 *
 * - below {@link QUIET_LEVEL} no bar lights, so a steady room reads empty
 *   instead of one flickering bar;
 * - every lit bar gets its own small height change per frame, so speech
 *   looks alive rather than one block switching on (none with Reduce
 *   Motion).
 */

/** Number of bars in the meter. */
export const METER_BARS = 12;

/** Levels below this light nothing (residual room noise). */
export const QUIET_LEVEL = 0.04;

/** How many bars a 0..1 level lights. */
export function litBars(level: number, bars = METER_BARS): number {
  if (!Number.isFinite(level) || level < QUIET_LEVEL) return 0;
  return Math.min(bars, Math.ceil(level * bars));
}

/** A bar's resting height, in % of the meter: rising left to right. */
export function baseHeight(i: number, bars = METER_BARS): number {
  return 30 + (i * 70) / (bars - 1);
}

/** Deterministic noise in 0..1 for bar `i` at frame `frame`. */
function wobble(i: number, frame: number): number {
  const x = Math.sin(i * 12.9898 + frame * 78.233) * 43758.5453;
  return x - Math.floor(x);
}

/**
 * Bar heights (% of the meter) for `level` at animation `frame`. Unlit bars
 * rest at {@link baseHeight}; lit bars vary between 70 % and 100 % of it,
 * more the louder the level. `still` (Reduce Motion) keeps every bar at its
 * resting height.
 */
export function barHeights(
  level: number,
  frame: number,
  still = false,
  bars = METER_BARS,
): number[] {
  const lit = litBars(level, bars);
  const depth = Math.min(1, Math.max(0, level)) * 0.3;
  return Array.from({ length: bars }, (_, i) => {
    const base = baseHeight(i, bars);
    if (still || i >= lit) return base;
    return base * (1 - depth * wobble(i, frame));
  });
}
