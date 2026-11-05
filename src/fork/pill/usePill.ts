// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The bar's state and levels from the backend (S-03).
 *
 * - State: `ForkPillState` events, plus `fork_pill_snapshot` once the
 *   listener is in place (the page may load after the take started). A
 *   lower `seq` never replaces a higher one.
 * - Ready: after each state is painted, the page reports its `seq` with
 *   `fork_pill_ready` (`useReportPainted`); the window shows or resizes only
 *   then. A state that makes the window smaller is reported once the
 *   capsule has shrunk into the new size (Design W4 morphs).
 * - Levels: `ForkPillLevels` (≤ 30 per second while a take shows), kept in
 *   a ref the waveform reads every animation frame (no React render per
 *   frame).
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

  return state;
}

/**
 * Reports `seq` with `fork_pill_ready` once the page has painted it: after
 * two animation frames, or after `delayMs` when the page first plays a
 * shrink into a smaller window.
 */
export function useReportPainted(seq: number | null, delayMs: number): void {
  useEffect(() => {
    if (seq == null) return;
    let second = 0;
    let timer = 0;
    const report = () => void commands.forkPillReady(seq);
    const first = requestAnimationFrame(() => {
      second = requestAnimationFrame(() => {
        if (delayMs > 0) timer = window.setTimeout(report, delayMs);
        else report();
      });
    });
    return () => {
      cancelAnimationFrame(first);
      cancelAnimationFrame(second);
      window.clearTimeout(timer);
    };
  }, [seq, delayMs]);
}

/** The latest waveform frame (0..1 per bar) and whether audio has come. */
export type LevelStore = { bars: number[]; heard: boolean };

/**
 * The backend's waveform frames, in a ref. The backend already shapes the
 * bars (noise floor, loudness, attack and release:
 * `fork::pill::levels::Waveform`); the waveform animates toward them.
 */
export function usePillLevels(active: boolean): {
  current: LevelStore;
} {
  const store = useRef<LevelStore>({ bars: Array(BARS).fill(0), heard: false });

  useEffect(() => {
    store.current = { bars: Array(BARS).fill(0), heard: false };
    if (!active) return;
    let alive = true;
    let off: UnlistenFn | null = null;
    void events.forkPillLevels
      .listen((event) => {
        const bars = Array.from({ length: BARS }, (_, i) => {
          const v = event.payload.bars[i] ?? 0;
          return Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0;
        });
        store.current = { bars, heard: true };
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

  return store;
}
