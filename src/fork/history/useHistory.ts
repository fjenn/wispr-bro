// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * History's data (S-15b): pages of `fork_history_page`, or of
 * `fork_history_find` while a search is on, kept live.
 *
 * - Pages of 100 are read as the list scrolls near its end (`loadMore`);
 *   a 10,000-row history is never read at once.
 * - Live: Handy's `history-update-payload` adds a new take on top, replaces
 *   a re-transcribed row and drops a deleted one. A new row is read again
 *   shortly after it appears, because what was pasted (a snippet, S-17)
 *   and the cleanup result are written just after Handy's save.
 * - `NotReady` (before Handy's setup finishes) is retried at 1-16 s.
 */
import { useEffect } from "react";
import { create } from "zustand";
import { events as handyEvents } from "@/bindings";
import { commands, type HistoryPage, type HistoryRow } from "../bindings";

export const PAGE_SIZE = 100;
/** A new row is read again after this long (see the module comment). */
export const SETTLE_MS = 1_500;

export type HistoryStatus = "loading" | "ready" | "unavailable";

interface HistoryState {
  rows: HistoryRow[];
  /** Cursor (or search offset) of the next page; null at the end. */
  next: number | null;
  status: HistoryStatus;
  loadingMore: boolean;
  /** The search text; empty = the whole history. */
  query: string;
  /** Reads the first page again (for the current query). */
  reload: () => Promise<void>;
  loadMore: () => Promise<void>;
  setQuery: (query: string) => void;
  /** Puts fresh copies of rows in place (or on top when new). */
  put: (rows: HistoryRow[]) => void;
  remove: (ids: number[]) => void;
  /** Reads these rows again from the backend. */
  refresh: (ids: number[]) => Promise<void>;
}

const MAX_RETRIES = 5;
let generation = 0;
/** The query whose first page is being read, if any. */
let loadingQuery: string | null = null;
let retries = 0;
let retryTimer: ReturnType<typeof setTimeout> | undefined;

async function fetchPage(
  query: string,
  next: number | null,
): Promise<HistoryPage | null> {
  const result = query
    ? await commands.forkHistoryFind(query, next ?? 0, PAGE_SIZE)
    : await commands.forkHistoryPage(next, PAGE_SIZE);
  return result?.status === "ok" && result.data ? result.data : null;
}

/** Newest first: record time, then id. */
function byNewest(a: HistoryRow, b: HistoryRow): number {
  return b.created_at - a.created_at || b.id - a.id;
}

export const useHistoryStore = create<HistoryState>()((set, get) => ({
  rows: [],
  next: null,
  status: "loading",
  loadingMore: false,
  query: "",
  reload: async () => {
    const { query } = get();
    // A read of the same first page is already on its way (two mounts at
    // once, or a retry timer racing a visibility refresh).
    if (loadingQuery === query) return;
    loadingQuery = query;
    const mine = ++generation;
    try {
      const page = await fetchPage(query, null);
      if (mine !== generation) return;
      if (!page) throw new Error("no page");
      retries = 0;
      clearTimeout(retryTimer);
      set({ rows: page.rows, next: page.next, status: "ready" });
    } catch (e) {
      if (mine !== generation) return;
      console.warn("fork: reading history failed", e);
      if (get().status !== "ready") set({ status: "unavailable" });
      if (retries < MAX_RETRIES) {
        const delay = 1_000 * 2 ** retries;
        retries += 1;
        clearTimeout(retryTimer);
        retryTimer = setTimeout(() => void get().reload(), delay);
      }
    } finally {
      if (mine === generation) loadingQuery = null;
    }
  },
  loadMore: async () => {
    const { next, loadingMore, query, status } = get();
    if (next === null || loadingMore || status !== "ready") return;
    const mine = generation;
    set({ loadingMore: true });
    try {
      const page = await fetchPage(query, next);
      if (mine !== generation || !page) return;
      const known = new Set(get().rows.map((r) => r.id));
      set({
        rows: [...get().rows, ...page.rows.filter((r) => !known.has(r.id))],
        next: page.next,
      });
    } catch (e) {
      console.warn("fork: reading more history failed", e);
    } finally {
      set({ loadingMore: false });
    }
  },
  setQuery: (query) => {
    if (query === get().query) return;
    set({ query, rows: [], next: null, status: "loading" });
    void get().reload();
  },
  put: (fresh) => {
    if (fresh.length === 0) return;
    const byId = new Map(fresh.map((r) => [r.id, r]));
    const rows = get().rows.map((r) => byId.get(r.id) ?? r);
    const known = new Set(rows.map((r) => r.id));
    const added = fresh.filter((r) => !known.has(r.id));
    set({ rows: added.length ? [...added, ...rows].sort(byNewest) : rows });
  },
  remove: (ids) => {
    const gone = new Set(ids);
    set({ rows: get().rows.filter((r) => !gone.has(r.id)) });
  },
  refresh: async (ids) => {
    try {
      const result = await commands.forkHistoryRows(ids);
      if (result?.status !== "ok" || !result.data) return;
      const found = new Set(result.data.map((r) => r.id));
      // Only rows already listed are replaced; others are not part of the
      // current view (a search) unless they are new takes (see `added`).
      const listed = new Set(get().rows.map((r) => r.id));
      get().put(result.data.filter((r) => listed.has(r.id)));
      const missing = ids.filter((id) => listed.has(id) && !found.has(id));
      if (missing.length) get().remove(missing);
    } catch (e) {
      console.warn("fork: refreshing history rows failed", e);
    }
  },
}));

/** A take Handy just saved goes on top (not while searching). */
async function added(id: number) {
  const store = useHistoryStore.getState();
  if (store.query) return;
  try {
    const result = await commands.forkHistoryRows([id]);
    if (result?.status === "ok" && result.data) store.put(result.data);
  } catch (e) {
    console.warn("fork: reading a new history row failed", e);
  }
  setTimeout(() => void useHistoryStore.getState().refresh([id]), SETTLE_MS);
}

/** Loads History on mount and follows Handy's history events. */
export function useHistoryLive() {
  useEffect(() => {
    // Always read again on mount: events that came while the page was
    // closed were not followed (rows on screen stay until the read lands).
    void useHistoryStore.getState().reload();
    const unlisten = handyEvents.historyUpdatePayload.listen(({ payload }) => {
      const s = useHistoryStore.getState();
      switch (payload.action) {
        case "added":
          void added(payload.entry.id);
          break;
        case "updated":
          void s.refresh([payload.entry.id]);
          break;
        case "deleted":
          s.remove([payload.id]);
          break;
        default:
          break;
      }
    });
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        const ids = useHistoryStore
          .getState()
          .rows.slice(0, 20)
          .map((r) => r.id);
        if (ids.length) void useHistoryStore.getState().refresh(ids);
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      document.removeEventListener("visibilitychange", onVisible);
      void unlisten.then((fn) => fn());
    };
  }, []);
}
