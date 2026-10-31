// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The bar's state and levels from the backend (S-03).
 *
 * - State: `ForkPillState` events, plus `fork_pill_snapshot` once the
 *   listener is in place (the page may load after the take started). A
 *   lower `seq` never replaces a higher one.
 * - Ready: after each state is painted (two animation frames), the page
 *   reports its `seq` with `fork_pill_ready`; the window shows or resizes
 *   only then.
 * - Levels: `ForkPillLevels` (≤ 30 per second while a take shows),
 *   smoothed like Handy's overlay waveform.
 */
import { useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { commands, events, type ForkPillState } from "../bindings";
import { BARS } from "./geometry";

function newer(
  prev: ForkPillState | null,
  next: ForkPillState | null,
): ForkPillState | null {
  if (!next) return prev;
  return !prev || next.seq >= prev.seq ? next : prev;
}

export function usePillState(): ForkPillState | null {
  const [state, setState] = useState<ForkPillState | null>(null);

  useEffect(() => {
    let alive = true;
    const offs: UnlistenFn[] = [];
    void (async () => {
      const off = await events.forkPillState.listen((event) => {
        setState((prev) => newer(prev, event.payload));
      });
      if (!alive) {
        off();
        return;
      }
      offs.push(off);
      try {
        const snapshot = await commands.forkPillSnapshot();
        if (alive) setState((prev) => newer(prev, snapshot));
      } catch (e) {
        console.warn("pill: snapshot failed", e);
      }
    })();
    return () => {
      alive = false;
      offs.forEach((off) => off());
    };
  }, []);

  // Report each painted state.
  useEffect(() => {
    if (!state) return;
    const seq = state.seq;
    let second = 0;
    const first = requestAnimationFrame(() => {
      second = requestAnimationFrame(() => {
        void commands.forkPillReady(seq);
      });
    });
    return () => {
      cancelAnimationFrame(first);
      cancelAnimationFrame(second);
    };
  }, [state]);

  return state;
}

/** Smoothed waveform bars (0..1). */
export function usePillLevels(active: boolean): number[] {
  const [bars, setBars] = useState<number[]>(() => Array(BARS).fill(0));
  const smoothed = useRef<number[]>(Array(BARS).fill(0));

  useEffect(() => {
    if (!active) {
      smoothed.current = Array(BARS).fill(0);
      setBars(smoothed.current);
      return;
    }
    let alive = true;
    let off: UnlistenFn | null = null;
    void events.forkPillLevels
      .listen((event) => {
        const next = smoothed.current.map((prev, i) => {
          const target = event.payload.bars[i] ?? 0;
          return prev * 0.6 + target * 0.4;
        });
        smoothed.current = next;
        setBars(next);
      })
      .then((unlisten) => {
        if (alive) off = unlisten;
        else unlisten();
      });
    return () => {
      alive = false;
      off?.();
    };
  }, [active]);

  return bars;
}
