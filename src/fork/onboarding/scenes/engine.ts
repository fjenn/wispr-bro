// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The first-use scenes' beat engine (S-30, DEC-15: animated scenes, not
 * videos). A scene is a timed script of beats; the loop applies each beat
 * at its time, then starts over. Under reduced motion one still frame (the
 * finished scene) is shown and nothing moves.
 *
 * Ported from Handy's onboarding demo scene engine
 * (`src/components/onboarding/screens/Demo.tsx`: `CHAPTERS`, `play`,
 * `later`/`clear`, the reduced-motion still; Copyright (c) 2025 CJ Pais,
 * MIT, LICENSES/MIT-Handy.txt). Modified by Wispr Bro contributors,
 * 2026-11-02: one generic hook over any script, a word-by-word caption,
 * pause/play (WCAG 2.2.2), no shortcut capture.
 */
import { useCallback, useEffect, useRef, useState } from "react";

export type SceneBeat = "reset" | "press" | "speak" | "release" | "land";

/** Beats in ms from the scene's start, and the loop's full length. */
export const SCENE_SCRIPT: { beats: [number, SceneBeat][]; ms: number } = {
  beats: [
    [0, "reset"],
    [600, "press"],
    [900, "speak"],
    [4200, "release"],
    [5000, "land"],
  ],
  ms: 8000,
};

/** Time per spoken word in the caption while "speak" runs. */
export const WORD_MS = 230;

export interface SceneFrame {
  beat: SceneBeat;
  /** Words of the spoken caption shown so far. */
  words: number;
}

/** The finished scene (reduced motion, paused before the first beat). */
export function stillFrame(wordCount: number): SceneFrame {
  return { beat: "land", words: wordCount };
}

export function prefersReducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

/**
 * Plays the script in a loop for a caption of `wordCount` words. A new
 * `sceneKey` (another choice) starts over. `paused` freezes the current
 * frame.
 */
export function useSceneLoop(
  sceneKey: string,
  wordCount: number,
  paused: boolean,
): { frame: SceneFrame; still: boolean } {
  const [still] = useState(prefersReducedMotion);
  const [frame, setFrame] = useState<SceneFrame>(() =>
    still ? stillFrame(wordCount) : { beat: "reset", words: 0 },
  );
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);

  const clear = () => {
    timers.current.forEach(clearTimeout);
    timers.current = [];
  };
  const later = (fn: () => void, ms: number) =>
    timers.current.push(setTimeout(fn, ms));

  const play = useCallback(() => {
    clear();
    for (const [time, beat] of SCENE_SCRIPT.beats) {
      later(() => {
        setFrame((f) => ({
          beat,
          words: beat === "reset" ? 0 : beat === "land" ? wordCount : f.words,
        }));
        if (beat === "speak") {
          for (let i = 1; i <= wordCount; i++) {
            later(
              () => setFrame((f) => ({ ...f, words: Math.max(f.words, i) })),
              Math.min(i * WORD_MS, 3000),
            );
          }
        }
      }, time);
    }
    later(play, SCENE_SCRIPT.ms);
    // `later` and `clear` only touch the ref.
  }, [wordCount]);

  useEffect(() => {
    if (still) {
      setFrame(stillFrame(wordCount));
      return;
    }
    if (paused) {
      clear();
      return;
    }
    setFrame({ beat: "reset", words: 0 });
    play();
    return clear;
  }, [sceneKey, still, paused, play, wordCount]);

  return { frame, still };
}
