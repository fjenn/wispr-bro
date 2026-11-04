// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/settings/meterBars.test.ts
import assert from "node:assert/strict";
import {
  barHeights,
  baseHeight,
  litBars,
  METER_BARS,
  QUIET_LEVEL,
} from "./meterBars";

// A synthetic sequence as the backend sends it (levels above the noise
// floor): steady café noise, then speech, then silence.
const noise = [0, 0.01, 0.03, 0.02, 0, 0.035, 0.01];
const speech = [0.35, 0.8, 0.95, 0.6, 0.85, 0.4, 0.9, 0.7];
const silence = [0.45, 0.2, 0.08, 0.03, 0];

// Steady noise lights nothing (no single flickering bar).
for (const l of noise) assert.equal(litBars(l), 0, `noise ${l}`);
assert.equal(litBars(QUIET_LEVEL), 1);

// Speech covers most of the meter and moves over a wide range.
const litSpeech = speech.map((l) => litBars(l));
assert.ok(Math.max(...litSpeech) >= 11, `${litSpeech}`);
assert.ok(Math.max(...litSpeech) - Math.min(...litSpeech) >= 6, `${litSpeech}`);

// OT2-B: a whisper in a café (the backend's levels for whispered speech at
// 0-6 dB SNR) lights a few bars and moves; noise around it lights none.
const whisper = [0.12, 0.22, 0.31, 0.18, 0.36, 0.25];
const litWhisper = whisper.map((l) => litBars(l));
assert.ok(Math.min(...litWhisper) >= 2, `${litWhisper}`);
assert.ok(
  Math.max(...litWhisper) - Math.min(...litWhisper) >= 2,
  `${litWhisper}`,
);
for (const l of [0.02, 0.03, 0.045]) assert.equal(litBars(l), 0, `noise ${l}`);

// Silence falls back to zero.
const litSilence = silence.map((l) => litBars(l));
assert.deepEqual(litSilence.slice(-2), [0, 0]);
assert.ok(litSilence[0] > litSilence[2]);

// Bad input is nothing.
assert.equal(litBars(Number.NaN), 0);
assert.equal(litBars(-1), 0);
assert.equal(litBars(7), METER_BARS);

// Lit bars vary per bar and per frame; unlit bars rest.
const a = barHeights(0.8, 1);
const b = barHeights(0.8, 2);
const lit = litBars(0.8);
assert.equal(a.length, METER_BARS);
assert.ok(
  a.slice(0, lit).some((h, i) => Math.abs(h - b[i]) > 0.5),
  "frames differ",
);
assert.ok(new Set(a.slice(0, lit).map((h, i) => h / baseHeight(i))).size > 3);
for (let i = lit; i < METER_BARS; i++) assert.equal(a[i], baseHeight(i));
for (let i = 0; i < METER_BARS; i++) {
  assert.ok(a[i] <= baseHeight(i) && a[i] >= baseHeight(i) * 0.7, `${i}`);
}
// Reduce Motion: no variation.
assert.deepEqual(
  barHeights(0.8, 1, true),
  Array.from({ length: METER_BARS }, (_, i) => baseHeight(i)),
);

console.log("meterBars: ok");
