// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * `useInsights()`: both Insights tabs' data, kept live (S-25).
 *
 * Reads `fork_insights_usage` and `fork_insights_voice` on mount, after
 * every `ForkStatsUpdated` (a counted take, retry or backfill), when the
 * window comes back into view and just after local midnight (the heatmap
 * and the streak roll over); the same triggers as the Home rail
 * (`useStatsSummary`). Overlapping triggers collapse into one extra read.
 * `NotReady` is retried at 1, 2, 4, 8 and 16 s. Data already shown stays if
 * a later read fails.
 */
import { useEffect, useState } from "react";
import {
  commands,
  events,
  type UsageInsights,
  type VoiceProfile,
} from "../bindings";
import { msUntilAfterMidnight } from "../home/useStatsSummary";

export type InsightsStatus = "loading" | "ready" | "unavailable";

export interface InsightsState {
  usage: UsageInsights | null;
  voice: VoiceProfile | null;
  status: InsightsStatus;
}

const MAX_RETRIES = 5;

export function useInsights(): InsightsState {
  const [state, setState] = useState<InsightsState>({
    usage: null,
    voice: null,
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
      setState((s) => (s.usage ? s : { ...s, status: "unavailable" }));
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
        const [usage, voice] = await Promise.all([
          commands.forkInsightsUsage(),
          commands.forkInsightsVoice(),
        ]);
        if (!alive) return;
        if (usage?.status === "ok" && usage.data) {
          retries = 0;
          clearTimeout(retryTimer);
          setState((s) => ({
            usage: usage.data,
            voice: voice?.status === "ok" && voice.data ? voice.data : s.voice,
            status: "ready",
          }));
        } else {
          failed();
        }
      } catch (e) {
        if (!alive) return;
        console.warn("fork: reading insights failed", e);
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
