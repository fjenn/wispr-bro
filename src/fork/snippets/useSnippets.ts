// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Snippets page's data (S-17): `fork_snippets_list`, kept live.
 *
 * Reads on mount, after every `ForkSnippetsChanged` (edits from any window,
 * imports, and every take that expanded a snippet: the use counts), after
 * every `ForkStatsUpdated`, and when the window comes back into view.
 * Overlapping triggers collapse into one extra read. `NotReady` is retried
 * at 1, 2, 4, 8 and 16 s; snippets already shown stay if a later read
 * fails. Same shape as the Dictionary page's `useDictionary` (S-16).
 */
import { useEffect } from "react";
import { create } from "zustand";
import {
  commands,
  events,
  type SnippetCounts,
  type SnippetEntry,
} from "../bindings";
import {
  searchKey,
  SORT_ORDERS,
  type SortOrder,
} from "../dictionary/useDictionary";

export { searchKey, SORT_ORDERS, type SortOrder };

export type SnippetsStatus = "loading" | "ready" | "unavailable";

interface SnippetsState {
  snippets: SnippetEntry[];
  counts: SnippetCounts;
  status: SnippetsStatus;
  reload: () => Promise<void>;
}

const MAX_RETRIES = 5;
let inFlight = false;
let again = false;
let retries = 0;
let retryTimer: ReturnType<typeof setTimeout> | undefined;

export const useSnippetsStore = create<SnippetsState>()((set, get) => {
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
    snippets: [],
    counts: { snippets: 0, uses: 0 },
    status: "loading",
    reload: async () => {
      if (inFlight) {
        again = true;
        return;
      }
      inFlight = true;
      try {
        const result = await commands.forkSnippetsList();
        if (result?.status === "ok" && result.data) {
          retries = 0;
          clearTimeout(retryTimer);
          set({
            snippets: result.data.snippets,
            counts: result.data.counts,
            status: "ready",
          });
        } else {
          failed();
        }
      } catch (e) {
        console.warn("fork: reading the snippets failed", e);
        failed();
      } finally {
        inFlight = false;
        if (again) {
          again = false;
          void get().reload();
        }
      }
    },
  };
});

/** Keeps the store live while the page is mounted. */
export function useSnippetsLive(): void {
  useEffect(() => {
    const reload = () => void useSnippetsStore.getState().reload();
    const onVisible = () => {
      if (document.visibilityState === "visible") reload();
    };
    const listeners = [
      events.forkSnippetsChanged.listen(reload),
      events.forkStatsUpdated.listen(reload),
    ].map((p) =>
      p.catch((e) => {
        console.warn("fork: listening for snippet changes failed", e);
        return null;
      }),
    );
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("focus", onVisible);
    reload();
    return () => {
      clearTimeout(retryTimer);
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("focus", onVisible);
      for (const l of listeners) void l.then((stop) => stop?.());
    };
  }, []);
}

export function sortSnippets(
  snippets: SnippetEntry[],
  order: SortOrder,
  language: string,
): SnippetEntry[] {
  const collator = new Intl.Collator(language, { sensitivity: "base" });
  const sorted = [...snippets];
  const byNewest = (a: SnippetEntry, b: SnippetEntry) =>
    b.created_at - a.created_at || b.id - a.id;
  switch (order) {
    case "newest":
      return sorted.sort(byNewest);
    case "oldest":
      return sorted.sort((a, b) => byNewest(b, a));
    case "az":
      return sorted.sort((a, b) => collator.compare(a.trigger, b.trigger));
    case "za":
      return sorted.sort((a, b) => collator.compare(b.trigger, a.trigger));
    case "used":
      return sorted.sort((a, b) => b.hit_count - a.hit_count || byNewest(a, b));
  }
}
