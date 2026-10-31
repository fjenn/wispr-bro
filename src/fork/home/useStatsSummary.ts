// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * `useStatsSummary()`: the stats rail's numbers, kept live (S-15a, C-213).
 *
 * Reads `fork_stats_summary` (S-09: lifetime and today's totals from
 * `daily_stats`, which survive retention and deletes) on mount, after every
 * `ForkStatsUpdated` event (each counted take, retry or backfill), when the
 * window comes back into view, and just after local midnight (the streak and
 * "today" roll over). Overlapping triggers collapse into one extra read, so a
 * burst of events costs at most two calls.
 *
 * Before Handy's setup finishes the command answers `NotReady`; the hook
 * then retries a few times with a growing delay (1, 2, 4, 8, 16 s). Numbers
 * already shown stay on screen if a later read fails.
 */
import { useEffect, useState } from "react";
import { commands, events, type StatsSummary } from "../bindings";

export type StatsStatus = "loading" | "ready" | "unavailable";

export interface StatsState {
  summary: StatsSummary | null;
  status: StatsStatus;
}

const MAX_RETRIES = 5;

/** Milliseconds until a few seconds after the next local midnight. */
export function msUntilAfterMidnight(now: Date = new Date()): number {
  const next = new Date(now);
  next.setHours(24, 0, 5, 0);
  return Math.max(1_000, next.getTime() - now.getTime());
}

export function useStatsSummary(): StatsState {
  const [state, setState] = useState<StatsState>({
    summary: null,
    status: "loading",
  });

  useEffect(() => {
    let alive = true;
    let inFlight = false;
    let again = false;
    let retries = 0;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;
    let midnightTimer: ReturnType<typeof setTimeout> | undefined;

    const failed = () => {
      setState((s) =>
        s.summary ? s : { summary: null, status: "unavailable" },
      );
      if (retries < MAX_RETRIES) {
        const delay = 1_000 * 2 ** retries;
        retries += 1;
        clearTimeout(retryTimer);
        retryTimer = setTimeout(() => void load(), delay);
      }
    };

    const load = async (): Promise<void> => {
      if (inFlight) {
        again = true;
        return;
      }
      inFlight = true;
      try {
        const result = await commands.forkStatsSummary();
        if (!alive) return;
        // A missing reply (null) is treated like an error.
        if (result?.status === "ok" && result.data) {
          retries = 0;
          clearTimeout(retryTimer);
          setState({ summary: result.data, status: "ready" });
        } else {
          failed();
        }
      } catch (e) {
        if (!alive) return;
        console.warn("fork: reading the stats failed", e);
        failed();
      } finally {
        inFlight = false;
        if (again && alive) {
          again = false;
          void load();
        }
      }
    };

    const scheduleMidnight = () => {
      midnightTimer = setTimeout(() => {
        void load();
        scheduleMidnight();
      }, msUntilAfterMidnight());
    };

    const onVisible = () => {
      if (document.visibilityState === "visible") void load();
    };

    const unlisten = events.forkStatsUpdated
      .listen(() => void load())
      .catch((e) => {
        console.warn("fork: listening for stats updates failed", e);
        return null;
      });
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("focus", onVisible);
    void load();
    scheduleMidnight();

    return () => {
      alive = false;
      clearTimeout(retryTimer);
      clearTimeout(midnightTimer);
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("focus", onVisible);
      void unlisten.then((stop) => stop?.());
    };
  }, []);

  return state;
}
