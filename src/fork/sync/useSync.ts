// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * `useSync()`: the sync flow both apps share (`src-tauri/src/fork/sync`).
 *
 *   pick() / open(path) ─► review (what would change) ─► apply(choices) ─► done
 *                                    └─ cancel()
 *   send(path | null, withCsv)   writes this device's file (Mac: to `path`,
 *                                shown in Finder; iPhone: the share sheet)
 *
 * Nothing is merged before `apply`. Errors come back as a problem id the
 * page has a sentence for (`sync.problems.*`).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { useSettingsStore } from "@/stores/settingsStore";
import {
  commands,
  events,
  type MergeReport,
  type SyncChoices,
  type SyncExport,
  type SyncReview,
  type SyncStatus,
} from "../bindings";

export type ReadyReview = Extract<SyncReview, { status: "ready" }>;
export type SyncProblem =
  | Extract<SyncReview, { status: "problem" }>["problem"]["problem"]
  | "failed";

export type SyncStep =
  | { step: "idle" }
  | { step: "reading" }
  | { step: "review"; path: string; review: ReadyReview }
  | { step: "merging"; path: string; review: ReadyReview }
  | { step: "done"; review: ReadyReview }
  | { step: "problem"; problem: SyncProblem };

export const ALL_PARTS: SyncChoices = {
  settings: true,
  dictionary: true,
  snippets: true,
  stats: true,
};

/** Whether a report changes anything here. */
export function changesAnything(report: MergeReport): boolean {
  const moved = (c: MergeReport["dictionary"]) =>
    c.added + c.changed + c.removed > 0;
  return (
    moved(report.dictionary) ||
    moved(report.snippets) ||
    report.settings.length > 0 ||
    report.stats_changed
  );
}

export function useSync() {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [state, setState] = useState<SyncStep>({ step: "idle" });
  const [sending, setSending] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const result = await commands.forkSyncStatus();
      // `?? null`: a backend without sync answers nothing.
      const data = result.status === "ok" ? (result.data ?? null) : null;
      if (data) setStatus(data);
      return data;
    } catch {
      return null;
    }
  }, []);

  // The file under review: the same file can be announced twice (waiting
  // at start and by the event).
  const reviewing = useRef<string | null>(null);

  const open = useCallback(async (path: string) => {
    if (reviewing.current === path) return;
    reviewing.current = path;
    setState({ step: "reading" });
    try {
      const result = await commands.forkSyncReview(path, ALL_PARTS);
      if (result.status !== "ok" || !result.data) {
        setState({ step: "problem", problem: "failed" });
      } else if (result.data.status === "problem") {
        void commands.forkSyncDiscard(path).catch(() => undefined);
        setState({ step: "problem", problem: result.data.problem.problem });
      } else {
        setState({ step: "review", path, review: result.data });
      }
    } catch {
      setState({ step: "problem", problem: "failed" });
    }
  }, []);

  // A file handed over by another app (AirDrop, Files): at start and live.
  useEffect(() => {
    let alive = true;
    const waiting = () =>
      refresh().then((s) => {
        if (alive && s?.pending_file) void open(s.pending_file);
      });
    void waiting();
    const unlisten = events.forkSyncFileOpened.listen((event) => {
      if (alive) void open(event.payload.path);
    });
    // A file announced while the listener was still being set up.
    void unlisten.then(waiting).catch(() => undefined);
    return () => {
      alive = false;
      void unlisten.then((f) => f()).catch(() => undefined);
    };
  }, [refresh, open]);

  const pick = useCallback(async () => {
    try {
      const path = await openFileDialog({ multiple: false, directory: false });
      if (typeof path === "string" && path) await open(path);
    } catch {
      setState({ step: "problem", problem: "unreadable" });
    }
  }, [open]);

  const apply = useCallback(
    async (choices: SyncChoices) => {
      if (state.step !== "review") return;
      const { path, review } = state;
      setState({ step: "merging", path, review });
      try {
        const result = await commands.forkSyncApply(path, choices);
        if (result.status === "ok" && result.data?.status === "ready") {
          // Handy's settings (languages, sounds) may have changed under
          // the page's copy of them.
          void useSettingsStore
            .getState()
            .refreshSettings()
            .catch(() => undefined);
          setState({ step: "done", review: result.data });
        } else if (
          result.status === "ok" &&
          result.data?.status === "problem"
        ) {
          setState({ step: "problem", problem: result.data.problem.problem });
        } else {
          setState({ step: "problem", problem: "failed" });
        }
      } catch {
        setState({ step: "problem", problem: "failed" });
      }
      void refresh();
    },
    [state, refresh],
  );

  const close = useCallback(() => {
    reviewing.current = null;
    if (state.step === "review") {
      void commands.forkSyncDiscard(state.path).catch(() => undefined);
    }
    setState({ step: "idle" });
    void refresh();
  }, [state, refresh]);

  const send = useCallback(
    async (
      path: string | null,
      withCsv: boolean,
    ): Promise<SyncExport | null> => {
      setSending(true);
      try {
        const result = await commands.forkSyncExport(path, withCsv);
        void refresh();
        return result.status === "ok" ? (result.data ?? null) : null;
      } catch {
        return null;
      } finally {
        setSending(false);
      }
    },
    [refresh],
  );

  return { status, state, sending, pick, open, apply, close, send, refresh };
}

export type Sync = ReturnType<typeof useSync>;

/** A date and time as the reader's locale writes them. */
export function formatWhen(ms: number, language: string): string {
  try {
    return new Intl.DateTimeFormat(language, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(ms));
  } catch {
    return new Date(ms).toLocaleString();
  }
}
