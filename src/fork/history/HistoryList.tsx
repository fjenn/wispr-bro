// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * History on the Dictation home (S-15b, C-34..C-45, C-207): every dictation
 * and file transcription, grouped by local day, newest first, with search
 * and the row actions.
 *
 * - **Scale (C-207):** a virtual list (@tanstack/react-virtual 3.14.23,
 *   MIT) inside the page's own scroll area (`wb-main`), rows measured as
 *   they render, pages of 100 read as the end comes near. Only the rows on
 *   screen (plus a few) are in the DOM, and the audio player mounts only
 *   when Play is pressed.
 * - **Copy (C-39, RE-04)** copies the shown text: after "Undo AI edit" the
 *   text before the cleanup, else what was inserted.
 * - **Undo / Redo AI edit (C-42, DEC-19)** only changes what History shows
 *   and copies; nothing is pasted again.
 * - **Retry (C-43)** runs Handy's background job, at the cleanup setting or
 *   at another level; the text it replaces is kept (`previous_text`).
 * - **Delete (C-44)** hides the row at once and offers "Undo" for the 10 s
 *   before S-09 purges it through Handy.
 * - **Save audio as… (C-45)** copies the recording to a file the user
 *   picks.
 * - **Flag (C-40, DEC-18)** opens "Wrong word?" (`FlagDialog`).
 */
import { useVirtualizer } from "@tanstack/react-virtual";
import { convertFileSrc } from "@tauri-apps/api/core";
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { AudioLines, FilePlus, SearchX } from "../ui/icons";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands as handyCommands } from "@/bindings";
import { AudioPlayerGroup } from "@/components/ui/AudioPlayer";
import { copyToClipboard } from "@/components/settings/history/clipboard";
import { pickAndTranscribeFiles } from "@/components/settings/history/transcribeFiles";
import { useTranscriptionJobsStore } from "@/stores/transcriptionJobsStore";
import {
  commands,
  type CleanupLevel,
  type DictionaryEntry,
  type HistoryRow as Row,
} from "../bindings";
import { navigate } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { EmptyState, FOCUS_RING } from "../ui";
import { dayKey, dayLabel, groupByDay, useToday } from "./dates";
import { FlagDialog } from "./FlagDialog";
import { HistoryRow } from "./HistoryRow";
import { Search } from "./Search";
import { useHistoryLive, useHistoryStore } from "./useHistory";
import { ForkArt } from "@/assets/fork/art";

/** S-09's undo window is 10 s; the toast closes a little before. */
export const UNDO_TOAST_MS = 9_000;
/** Start reading the next page this many items before the end. */
const PREFETCH_ITEMS = 15;

type Item =
  | { kind: "day"; key: string }
  | { kind: "row"; row: Row; key: string };

/** The closest scrolling ancestor (the Hub's `main`), else the page. */
function scrollParent(el: HTMLElement | null): HTMLElement | null {
  let node = el?.parentElement ?? null;
  while (node) {
    const overflow = getComputedStyle(node).overflowY;
    if (overflow === "auto" || overflow === "scroll") return node;
    node = node.parentElement;
  }
  return (document.scrollingElement as HTMLElement | null) ?? null;
}

/** A file name for "Save audio as…": `dictation-2026-10-31-0941.wav`. */
export function audioFileName(row: Row): string {
  const d = new Date(row.created_at);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `dictation-${dayKey(row.created_at)}-${pad(d.getHours())}${pad(d.getMinutes())}.wav`;
}

export function HistoryList() {
  useHistoryLive();
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { rows, status, next, query, loadingMore } = useHistoryStore();
  const jobs = useTranscriptionJobsStore((s) => s.jobs);
  const { settings } = useForkSettings();
  const level: CleanupLevel = settings?.cleanup_level ?? "light";
  const audioDays = settings ? (settings.audio_retention_days ?? null) : 14;
  const now = useToday();
  const [flagging, setFlagging] = useState<Row | null>(null);

  const listRef = useRef<HTMLDivElement>(null);
  const [scrollEl, setScrollEl] = useState<HTMLElement | null>(null);
  const [scrollMargin, setScrollMargin] = useState(0);

  useEffect(() => {
    void useTranscriptionJobsStore.getState().initialize();
  }, []);

  const items = useMemo<Item[]>(() => {
    const out: Item[] = [];
    for (const group of groupByDay(rows)) {
      out.push({ kind: "day", key: `day-${group.key}` });
      for (const row of group.rows)
        out.push({ kind: "row", row, key: `row-${row.id}` });
    }
    return out;
  }, [rows]);

  useLayoutEffect(() => {
    const list = listRef.current;
    const parent = scrollParent(list);
    setScrollEl(parent);
    if (!list || !parent) return;
    const measure = () => {
      const top =
        list.getBoundingClientRect().top -
        parent.getBoundingClientRect().top +
        parent.scrollTop;
      setScrollMargin((m) => (Math.abs(m - top) > 0.5 ? top : m));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(parent);
    if (parent.firstElementChild) observer.observe(parent.firstElementChild);
    return () => observer.disconnect();
  }, [status]);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollEl,
    estimateSize: (i) => (items[i]?.kind === "day" ? 40 : 76),
    getItemKey: (i) => items[i]?.key ?? i,
    overscan: 8,
    scrollMargin,
  });
  const virtualItems = virtualizer.getVirtualItems();
  const lastIndex = virtualItems[virtualItems.length - 1]?.index ?? -1;

  useEffect(() => {
    if (next !== null && lastIndex >= items.length - PREFETCH_ITEMS) {
      void useHistoryStore.getState().loadMore();
    }
  }, [lastIndex, items.length, next]);

  const loadAudio = useCallback(async (fileName: string) => {
    try {
      const result = await handyCommands.getAudioFilePath(fileName);
      return result.status === "ok"
        ? convertFileSrc(result.data, "asset")
        : null;
    } catch (e) {
      console.warn("fork: finding the recording failed", e);
      return null;
    }
  }, []);

  const onAudioMissing = useCallback(
    (row: Row) => {
      toast.error(t("history.audio.missing"));
      void useHistoryStore.getState().refresh([row.id]);
    },
    [t],
  );

  const onCopy = useCallback(
    async (row: Row) => {
      const ok = await copyToClipboard(row.copy_text);
      if (!ok) toast.error(t("history.copyFailed"));
      return ok;
    },
    [t],
  );

  const onToggleVariant = useCallback(
    async (row: Row) => {
      const variant = row.variant === "final" ? "raw" : "final";
      try {
        const result = await commands.forkHistorySetVariant(row.id, variant);
        if (result.status === "ok" && result.data) {
          useHistoryStore.getState().put([result.data]);
          return;
        }
      } catch (e) {
        console.warn("fork: changing the shown text failed", e);
      }
      toast.error(t("history.actionFailed"));
    },
    [t],
  );

  const onRetry = useCallback(
    async (row: Row, retryLevel: CleanupLevel | null) => {
      try {
        const result = await commands.forkHistoryRetry(row.id, retryLevel);
        if (result.status === "ok") return;
      } catch (e) {
        console.warn("fork: retry failed", e);
      }
      toast.error(t("history.actionFailed"));
    },
    [t],
  );

  const onSaveAudio = useCallback(
    async (row: Row) => {
      let path: string | null;
      try {
        path = await saveFileDialog({
          defaultPath: audioFileName(row),
          filters: [{ name: t("history.audio.filter"), extensions: ["wav"] }],
        });
      } catch (e) {
        console.warn("fork: the save panel failed", e);
        toast.error(t("history.audio.saveFailed"));
        return;
      }
      if (!path) return;
      try {
        const result = await commands.forkHistorySaveAudio(row.id, path);
        if (result.status === "ok" && result.data === null) {
          toast.success(t("history.audio.saved"));
          return;
        }
        if (result.status === "ok" && result.data?.kind === "NoAudio") {
          toast.error(t("history.audio.missing"));
          void useHistoryStore.getState().refresh([row.id]);
          return;
        }
      } catch (e) {
        console.warn("fork: saving the recording failed", e);
      }
      toast.error(t("history.audio.saveFailed"));
    },
    [t],
  );

  const onDelete = useCallback(
    async (row: Row) => {
      const store = useHistoryStore.getState();
      store.remove([row.id]);
      let hidden = false;
      try {
        const result = await commands.forkHistoryDelete([row.id]);
        hidden = result.status === "ok";
      } catch (e) {
        console.warn("fork: deleting the dictation failed", e);
      }
      if (!hidden) {
        useHistoryStore.getState().put([row]);
        toast.error(t("history.deleteFailed"));
        return;
      }
      toast(t("history.deleted"), {
        id: `wb-history-deleted-${row.id}`,
        duration: UNDO_TOAST_MS,
        action: {
          label: t("history.undo"),
          onClick: async () => {
            try {
              const undone = await commands.forkHistoryUndoDelete([row.id]);
              if (undone.status === "ok" && undone.data > 0) {
                const fresh = await commands.forkHistoryRows([row.id]);
                useHistoryStore
                  .getState()
                  .put(
                    fresh.status === "ok" && fresh.data.length
                      ? fresh.data
                      : [row],
                  );
                return;
              }
            } catch (e) {
              console.warn("fork: undoing the delete failed", e);
            }
            toast.error(t("history.undoFailed"));
          },
        },
      });
    },
    [t],
  );

  const onFlagged = useCallback(
    (row: Row | null, entry: DictionaryEntry | null) => {
      if (row) useHistoryStore.getState().put([row]);
      if (!entry) return;
      toast.success(
        entry.replacement
          ? t("history.flag.savedRule", {
              from: entry.phrase,
              to: entry.replacement,
            })
          : t("history.flag.savedWord", { word: entry.phrase }),
        {
          action: {
            label: t("history.flag.openDictionary"),
            onClick: () => navigate("dictionary"),
          },
        },
      );
    },
    [t],
  );

  const transcribeFile = async () => {
    try {
      await pickAndTranscribeFiles();
    } catch (e) {
      console.warn("fork: picking a file failed", e);
      toast.error(t("history.transcribeFailed"));
    }
  };

  let body: React.ReactNode;
  if (status === "loading" && rows.length === 0) {
    body = (
      <p
        data-testid="wb-history-loading"
        className="px-3 py-8 text-center text-[13.5px] text-(--wb-ink-2)"
      >
        {t("history.loading")}
      </p>
    );
  } else if (status === "unavailable" && rows.length === 0) {
    body = (
      <div data-testid="wb-history-unavailable" role="status">
        <EmptyState
          art={<ForkArt name="empty-unavailable" width={96} />}
          title={t("history.unavailable")}
          body=""
          compact
        />
      </div>
    );
  } else if (rows.length === 0) {
    body = query ? (
      <EmptyState
        testId="wb-history-no-results"
        art={<ForkArt name="empty-search" width={96} />}
        icon={SearchX}
        title={t("history.noResults.title")}
        body={t("history.noResults.body", { query })}
        compact
      />
    ) : (
      <EmptyState
        testId="wb-empty-dictation"
        art={<ForkArt name="empty-history" width={160} />}
        icon={AudioLines}
        title={t("hub.pages.dictation.empty.title")}
        body={t("history.empty.body")}
      />
    );
  } else {
    body = (
      <AudioPlayerGroup>
        <div
          data-testid="wb-history-items"
          style={{ height: virtualizer.getTotalSize(), position: "relative" }}
        >
          {virtualItems.map((v) => {
            const item = items[v.index];
            if (!item) return null;
            // Each day's rows form one bordered container (the reference's
            // list card): the first row carries the top edge and corners,
            // the last the bottom ones, every row a divider above it.
            const first = items[v.index - 1]?.kind !== "row";
            const last = items[v.index + 1]?.kind !== "row";
            const edge =
              item.kind === "row"
                ? `overflow-hidden border-x border-t border-(--wb-line) bg-(--wb-surface) ${first ? "rounded-t-(--wb-radius-md)" : ""} ${last ? "rounded-b-(--wb-radius-md) border-b" : ""}`
                : "";
            return (
              <div
                key={v.key}
                data-index={v.index}
                ref={virtualizer.measureElement}
                className={edge}
                style={{
                  position: "absolute",
                  top: 0,
                  insetInlineStart: 0,
                  width: "100%",
                  transform: `translateY(${v.start - virtualizer.options.scrollMargin}px)`,
                }}
              >
                {item.kind === "day" ? (
                  <DayHeading dayKey={item.key.slice(4)} now={now} />
                ) : (
                  <HistoryRow
                    row={item.row}
                    job={jobs[item.row.id]}
                    level={level}
                    audioDays={audioDays}
                    loadAudio={loadAudio}
                    onAudioMissing={onAudioMissing}
                    onCopy={onCopy}
                    onFlag={setFlagging}
                    onToggleVariant={(r) => void onToggleVariant(r)}
                    onRetry={(r, l) => void onRetry(r, l)}
                    onSaveAudio={(r) => void onSaveAudio(r)}
                    onDelete={(r) => void onDelete(r)}
                  />
                )}
              </div>
            );
          })}
        </div>
        {loadingMore && (
          <p className="py-3 text-center text-[12.5px] text-(--wb-ink-3)">
            {t("history.loadingMore")}
          </p>
        )}
      </AudioPlayerGroup>
    );
  }

  return (
    <section
      aria-labelledby="wb-history-heading"
      data-testid="wb-history"
      data-lang={i18n.language}
      className="flex min-w-0 flex-col gap-1"
    >
      <div className="flex min-h-9 flex-wrap items-center justify-between gap-2">
        <h2 id="wb-history-heading" className="type-card-title text-(--wb-ink)">
          {t("history.title")}
        </h2>
        <div className="flex min-w-0 items-center gap-1">
          <Search
            query={query}
            onQuery={(q) => useHistoryStore.getState().setQuery(q)}
          />
          <button
            type="button"
            data-testid="wb-history-transcribe-file"
            aria-label={t("history.transcribeFile")}
            title={t("history.transcribeFile")}
            onClick={() => void transcribeFile()}
            className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink-3) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
          >
            <FilePlus size={18} aria-hidden="true" />
          </button>
        </div>
      </div>
      {query && rows.length > 0 && (
        <p
          role="status"
          data-testid="wb-history-results"
          className="px-1 text-[12.5px] text-(--wb-ink-2)"
        >
          {next !== null
            ? t("history.resultsMore", { count: rows.length })
            : t("history.results", { count: rows.length })}
        </p>
      )}
      <div ref={listRef}>{body}</div>
      <FlagDialog
        row={flagging}
        onOpenChange={(open) => !open && setFlagging(null)}
        onFlagged={onFlagged}
      />
    </section>
  );
}

function DayHeading({ dayKey: key, now }: { dayKey: string; now: number }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const label = dayLabel(key, now, i18n.language);
  const text =
    label.kind === "today"
      ? t("history.today")
      : label.kind === "yesterday"
        ? t("history.yesterday")
        : label.text;
  return (
    <h3
      data-testid="wb-history-day"
      data-day={key}
      className="type-micro px-1 pt-5 pb-3 text-(--wb-ink-3)"
    >
      {text}
    </h3>
  );
}
