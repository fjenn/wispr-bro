// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The bar's waveform (Design W4): thin bars, a dotted line until the first
 * audio arrives, then bars that follow the voice.
 *
 * The backend sends one frame of levels per ~33 ms, already shaped (noise
 * floor, loudness, attack and release: `fork::pill::levels`), lowest
 * frequency first. Here the loudest buckets go to the middle and the
 * heights taper toward the ends (the bar reads as one centred shape), and
 * every animation frame eases each bar toward its level with a small
 * per-bar shimmer while there is sound, so speech looks alive between
 * frames. Silence never shimmers: a quiet room stays a still dotted line.
 * The DOM is updated directly (no React render per frame).
 *
 * Reduce Motion: the bars still follow the voice (it is information), at
 * once and without shimmer.
 */
import { useEffect, useRef } from "react";
import { useReducedMotion } from "motion/react";
import { BARS } from "../geometry";
import type { LevelStore } from "../usePill";

/** Bar height range (px); the lowest is a round dot. */
const MIN_H = 2;
const MAX_H = 18;
/** Edge bars reach this share of the middle one. */
const EDGE = 0.55;
/** Attack and release time constants (ms). */
const ATTACK_MS = 30;
const RELEASE_MS = 110;

/** Bar position → backend bucket: bucket 0 in the middle, then outward. */
export const ORDER: number[] = (() => {
  const mid = Math.floor(BARS / 2);
  const order = Array<number>(BARS).fill(0);
  let bucket = 0;
  order[mid] = bucket++;
  for (let d = 1; bucket < BARS; d++) {
    if (mid - d >= 0) order[mid - d] = bucket++;
    if (mid + d < BARS && bucket < BARS) order[mid + d] = bucket++;
  }
  return order;
})();

/** Height weight per position: 1 in the middle, `EDGE` at the ends. */
export const WEIGHT: number[] = Array.from({ length: BARS }, (_, i) => {
  const mid = (BARS - 1) / 2;
  const d = mid === 0 ? 0 : Math.abs(i - mid) / mid;
  return 1 - (1 - EDGE) * Math.pow(d, 1.4);
});

export function Waveform({ levels }: { levels: { current: LevelStore } }) {
  const ref = useRef<HTMLDivElement>(null);
  const reduced = useReducedMotion() ?? false;

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const bars = Array.from(el.children) as HTMLElement[];
    const shown = new Float64Array(BARS);
    let last = performance.now();
    let raf = 0;
    const tick = (now: number) => {
      // Real time between frames (a slow frame catches up, it never lags).
      const dt = Math.min(1000, Math.max(0, now - last));
      last = now;
      const { bars: level, heard } = levels.current;
      for (let i = 0; i < BARS; i++) {
        let target = (level[ORDER[i]] ?? 0) * WEIGHT[i];
        if (!reduced && target > 0.03) {
          target *= 0.86 + 0.14 * Math.sin(now / 160 + i * 1.9 + (i % 3));
        }
        if (reduced) shown[i] = target;
        else {
          const tau = target > shown[i] ? ATTACK_MS : RELEASE_MS;
          shown[i] += (target - shown[i]) * (1 - Math.exp(-dt / tau));
        }
        const h = MIN_H + Math.min(1, Math.max(0, shown[i])) * (MAX_H - MIN_H);
        bars[i].style.height = `${h.toFixed(2)}px`;
      }
      const state = heard ? "audio" : "idle";
      if (el.dataset.state !== state) el.dataset.state = state;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [levels, reduced]);

  return (
    <div
      ref={ref}
      className="wb-pill-wave"
      data-testid="wb-pill-wave"
      data-state="idle"
      aria-hidden
    >
      {Array.from({ length: BARS }, (_, i) => (
        <i key={i} style={{ height: MIN_H }} />
      ))}
    </div>
  );
}
