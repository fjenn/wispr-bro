// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Dictionary page's data (S-16): `fork_dictionary_list`, kept live.
 *
 * Reads on mount, after every `ForkDictionaryChanged` (edits from any
 * window, imports, words taken over from Handy's settings), after every
 * `ForkStatsUpdated` (a take may have used a replacement: hit counts), and
 * when the window comes back into view. Overlapping triggers collapse into
 * one extra read. `NotReady` (before Handy's setup finishes) is retried at
 * 1, 2, 4, 8 and 16 s; entries already shown stay on screen if a later
 * read fails.
 */
import { useEffect } from "react";
import { create } from "zustand";
import {
  commands,
  events,
  type DictionaryCounts,
  type DictionaryEntry,
} from "../bindings";

/**
 * `unavailable`: the app is still starting (`NotReady`, retried);
 * `broken`: the dictionary's data file cannot be opened (`Internal`, e.g.
 * `fork.db` failed to migrate), so adding a word would fail too.
 */
export type DictionaryStatus = "loading" | "ready" | "unavailable" | "broken";

interface DictionaryState {
  entries: DictionaryEntry[];
  counts: DictionaryCounts;
  status: DictionaryStatus;
  /** Re-reads the list now (the Refresh button, after an edit). */
  reload: () => Promise<void>;
}

const MAX_RETRIES = 5;
let inFlight = false;
let again = false;
let retries = 0;
let retryTimer: ReturnType<typeof setTimeout> | undefined;

export const useDictionaryStore = create<DictionaryState>()((set, get) => {
  const failed = (error?: unknown) => {
    const broken =
      typeof error === "object" &&
      error !== null &&
      (error as { kind?: unknown }).kind === "Internal";
    if (get().status !== "ready") {
      set({ status: broken ? "broken" : "unavailable" });
    }
    if (retries < MAX_RETRIES) {
      const delay = 1_000 * 2 ** retries;
      retries += 1;
      clearTimeout(retryTimer);
      retryTimer = setTimeout(() => void get().reload(), delay);
    }
  };
  return {
    entries: [],
    counts: { words: 0, rules: 0, suggested: 0 },
    status: "loading",
    reload: async () => {
      if (inFlight) {
        again = true;
        return;
      }
      inFlight = true;
      try {
        const result = await commands.forkDictionaryList();
        if (result?.status === "ok" && result.data) {
          retries = 0;
          clearTimeout(retryTimer);
          set({
            entries: result.data.entries,
            counts: result.data.counts,
            status: "ready",
          });
        } else {
          if (result?.status === "error") {
            console.warn("fork: reading the dictionary failed", result.error);
          }
          failed(result?.status === "error" ? result.error : undefined);
        }
      } catch (e) {
        console.warn("fork: reading the dictionary failed", e);
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
export function useDictionaryLive(): void {
  useEffect(() => {
    const reload = () => void useDictionaryStore.getState().reload();
    const onVisible = () => {
      if (document.visibilityState === "visible") reload();
    };
    const listeners = [
      events.forkDictionaryChanged.listen(reload),
      events.forkStatsUpdated.listen(reload),
    ].map((p) =>
      p.catch((e) => {
        console.warn("fork: listening for dictionary changes failed", e);
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

/** Folds case and accents for search ("siobhan" finds "Siobhán"). */
export function searchKey(s: string): string {
  return s
    .normalize("NFD")
    .replace(/\p{M}+/gu, "")
    .toLocaleLowerCase()
    .replace(/[‘’ʼ]/g, "'");
}

export type SortOrder = "newest" | "oldest" | "az" | "za" | "used";
export const SORT_ORDERS: SortOrder[] = [
  "newest",
  "oldest",
  "az",
  "za",
  "used",
];

export function sortEntries(
  entries: DictionaryEntry[],
  order: SortOrder,
  language: string,
): DictionaryEntry[] {
  const collator = new Intl.Collator(language, { sensitivity: "base" });
  const sorted = [...entries];
  const byNewest = (a: DictionaryEntry, b: DictionaryEntry) =>
    b.created_at - a.created_at || b.id - a.id;
  switch (order) {
    case "newest":
      return sorted.sort(byNewest);
    case "oldest":
      return sorted.sort((a, b) => byNewest(b, a));
    case "az":
      return sorted.sort((a, b) => collator.compare(a.phrase, b.phrase));
    case "za":
      return sorted.sort((a, b) => collator.compare(b.phrase, a.phrase));
    case "used":
      return sorted.sort((a, b) => b.hit_count - a.hit_count || byNewest(a, b));
  }
}

/** S-34: how many auto-learn suggestions wait for review. */
export function useSuggestedCount(): number {
  return useDictionaryStore((s) => s.counts.suggested);
}

/**
 * Keeps [`useSuggestedCount`] live where the Dictionary page is not mounted
 * (the sidebar's badge): reads the counts once, then follows
 * `ForkDictionaryChanged`, which carries them.
 */
export function useSuggestedCountLive(): void {
  useEffect(() => {
    if (useDictionaryStore.getState().status !== "ready") {
      void useDictionaryStore.getState().reload();
    }
    const off = events.forkDictionaryChanged
      .listen((event) => {
        useDictionaryStore.setState({ counts: event.payload.counts });
      })
      .catch((e) => {
        console.warn("fork: listening for dictionary changes failed", e);
        return null;
      });
    return () => void off.then((stop) => stop?.());
  }, []);
}
