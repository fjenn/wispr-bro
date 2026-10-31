// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Style page's data (S-19a/b): `fork_style_overview`, kept live.
 *
 * Reads on mount, after every `ForkAppRulesChanged` (a change in any
 * window), after `fork-stats-updated` (a take in a new app or site lists it) and
 * when the window comes back into view. A change made here applies the
 * overview the command returns at once. `NotReady` is retried at 1, 2, 4,
 * 8 and 16 s. App icons are fetched once per app (`fork_style_app_icon`).
 */
import { useEffect, useState } from "react";
import { create } from "zustand";
import {
  commands,
  events,
  type ForkError,
  type Result,
  type StyleApp,
  type StyleGroup,
  type StyleOverview,
} from "../bindings";

export type StyleStatus = "loading" | "ready" | "unavailable";

export const STYLE_GROUPS: readonly StyleGroup[] = [
  "personal",
  "work",
  "email",
  "other",
];

interface StyleState {
  apps: StyleApp[];
  sites: StyleOverview["sites"];
  status: StyleStatus;
  /** The last change failed (shown until the next success). */
  changeFailed: boolean;
  reload: () => Promise<void>;
  /** Runs a change command and applies its overview. */
  change: (
    run: () => Promise<Result<StyleOverview, ForkError>>,
  ) => Promise<boolean>;
}

const MAX_RETRIES = 5;
let inFlight = false;
let again = false;
let retries = 0;
let retryTimer: ReturnType<typeof setTimeout> | undefined;

export const useStyleStore = create<StyleState>()((set, get) => {
  const failed = () => {
    if (get().status !== "ready") set({ status: "unavailable" });
    if (retries < MAX_RETRIES) {
      const delay = 1_000 * 2 ** retries;
      retries += 1;
      clearTimeout(retryTimer);
      retryTimer = setTimeout(() => void get().reload(), delay);
    }
  };
  return {
    apps: [],
    sites: [],
    status: "loading",
    changeFailed: false,
    reload: async () => {
      if (inFlight) {
        again = true;
        return;
      }
      inFlight = true;
      try {
        const result = await commands.forkStyleOverview();
        if (result?.status === "ok" && result.data) {
          retries = 0;
          clearTimeout(retryTimer);
          set({
            apps: result.data.apps,
            sites: result.data.sites,
            status: "ready",
          });
        } else {
          failed();
        }
      } catch (e) {
        console.warn("fork: reading the style apps failed", e);
        failed();
      } finally {
        inFlight = false;
        if (again) {
          again = false;
          void get().reload();
        }
      }
    },
    change: async (run) => {
      try {
        const result = await run();
        if (result.status === "ok") {
          set({
            apps: result.data.apps,
            sites: result.data.sites,
            status: "ready",
            changeFailed: false,
          });
          return true;
        }
        console.warn("fork: style change refused", result.error);
      } catch (e) {
        console.warn("fork: style change failed", e);
      }
      set({ changeFailed: true });
      return false;
    },
  };
});

/** The page's apps and sites, loaded and kept live while mounted. */
export function useStyleApps() {
  const apps = useStyleStore((s) => s.apps);
  const sites = useStyleStore((s) => s.sites);
  const status = useStyleStore((s) => s.status);
  const changeFailed = useStyleStore((s) => s.changeFailed);
  useEffect(() => {
    const reload = () => void useStyleStore.getState().reload();
    reload();
    const unlisteners = [
      events.forkAppRulesChanged.listen(reload),
      events.forkStatsUpdated.listen(reload),
    ];
    const onVisible = () => {
      if (document.visibilityState === "visible") reload();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      document.removeEventListener("visibilitychange", onVisible);
      for (const u of unlisteners) void u.then((f) => f()).catch(() => {});
    };
  }, []);
  return { apps, sites, status, changeFailed };
}

const icons = new Map<string, Promise<string | null>>();

/** An app's icon (`data:` URL), or null while loading or when it has none. */
export function useAppIcon(bundleId: string): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    let p = icons.get(bundleId);
    if (!p) {
      p = commands
        .forkStyleAppIcon(bundleId)
        .then((r) => (r.status === "ok" ? r.data : null))
        .catch(() => null);
      icons.set(bundleId, p);
    }
    void p.then((u) => {
      if (live) setUrl(u);
    });
    return () => {
      live = false;
    };
  }, [bundleId]);
  return url;
}

/** Test hook: forget cached icons. */
export function resetStyleIconsForTests(): void {
  icons.clear();
}
