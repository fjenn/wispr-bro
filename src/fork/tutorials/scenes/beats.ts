// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A looping beat script for lesson scenes (S-31, DEC-15: animated scenes,
 * not videos). A script is a list of `[ms, beat]`; the loop applies each
 * beat at its time and starts over after `ms`. Under reduced motion the
 * last beat (the finished scene) is shown and nothing moves; `paused`
 * freezes the current beat (WCAG 2.2.2).
 *
 * Generalised from the first-use scene engine (`onboarding/scenes/
 * engine.ts`), itself ported from Handy's onboarding demo
 * (`src/components/onboarding/screens/Demo.tsx`: the timed `play` loop,
 * `later`/`clear`, the reduced-motion still; Copyright (c) 2025 CJ Pais,
 * MIT, LICENSES/MIT-Handy.txt). Modified by Wispr Bro contributors,
 * 2026-11-03: any beat names, any script, no caption logic.
 */
import { useEffect, useRef, useState } from "react";
import { prefersReducedMotion } from "../../onboarding/scenes/engine";

export interface BeatScript<B extends string> {
  beats: readonly (readonly [number, B])[];
  ms: number;
}

/** The beat shown under reduced motion: the script's last one. */
export function finalBeat<B extends string>(script: BeatScript<B>): B {
  return script.beats[script.beats.length - 1][1];
}

export function useBeatScript<B extends string>(
  script: BeatScript<B>,
  paused: boolean,
): { beat: B; still: boolean } {
  const [still] = useState(prefersReducedMotion);
  const [beat, setBeat] = useState<B>(() =>
    still ? finalBeat(script) : script.beats[0][1],
  );
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);

  useEffect(() => {
    const clear = () => {
      timers.current.forEach(clearTimeout);
      timers.current = [];
    };
    if (still) {
      setBeat(finalBeat(script));
      return;
    }
    if (paused) return clear;
    const play = () => {
      clear();
      for (const [time, next] of script.beats) {
        timers.current.push(setTimeout(() => setBeat(next), time));
      }
      timers.current.push(setTimeout(play, script.ms));
    };
    play();
    return clear;
  }, [script, still, paused]);

  return { beat, still };
}
