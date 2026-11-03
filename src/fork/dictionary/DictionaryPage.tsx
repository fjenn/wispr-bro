// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Dictionary page (S-16, C-58..C-65): the Hub's Dictionary slot.
 *
 *   Dictionary                         [Import] [Export] [Add word]
 *   ┌ banner: Teach Wispr Bro your words · example chips ──────── × ┐
 *   All (n) · Added by you (n) · Suggested (n)
 *   [search…]                              [sort ▾] [refresh]
 *   Siobhán                                           ✎  🗑
 *   gonna → going to        Used 3 times              ✎  🗑
 *
 * Words go to the recognizer (Handy's custom words), replacements to the
 * text pipeline; both are added here. "Suggested" holds S-34's auto-learn
 * suggestions (Accept ✓ / Reject ✕) under the "Learn from my corrections"
 * toggle (off by default, DEC-20); a rejected one never returns. Counts
 * follow every change live (`useDictionaryLive`). Deleting offers Undo.
 */
import { useDeferredValue, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import {
  ArrowRight,
  BookA,
  Check,
  Download,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Sparkles,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { commands, type DictionaryEntry } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { PageHeader } from "../hub/PageHeader";
import { navigate, useHubNav } from "../hub/nav";
import { Banner, Button, EmptyState, FOCUS_RING, TabPanel, Tabs } from "../ui";
import { ForkToggle } from "../settings/Controls";
import { useForkSettings } from "../settings/useForkSettings";
import { AddWordDialog, INPUT_CLASS } from "./AddWordDialog";
import { DictionaryImportWizard } from "./ImportWizard";
import {
  SORT_ORDERS,
  searchKey,
  sortEntries,
  useDictionaryLive,
  useDictionaryStore,
  type SortOrder,
} from "./useDictionary";

type TabId = "all" | "mine" | "suggested";

const BANNER_ID = "dictionary.teach";

function inTab(entry: DictionaryEntry, tab: TabId): boolean {
  switch (tab) {
    case "all":
      return entry.status === "active";
    case "mine":
      return entry.status === "active" && entry.source !== "learn";
    case "suggested":
      return entry.status === "suggested";
  }
}

function EntryRow({
  entry,
  onEdit,
  onDelete,
}: {
  entry: DictionaryEntry;
  onEdit: (e: DictionaryEntry) => void;
  onDelete: (e: DictionaryEntry) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const label = entry.replacement
    ? t("dictionary.entry.rule", { from: entry.phrase, to: entry.replacement })
    : entry.phrase;
  const sourceLabel =
    entry.source === "user" ? null : t(`dictionary.source.${entry.source}`);
  const suggested = entry.status === "suggested";
  const accept = async () => {
    const result = await commands.forkDictionaryAccept([entry.id]);
    if (result.status === "ok" && result.data.accepted > 0) {
      void useDictionaryStore.getState().reload();
      toast.success(t("dictionary.added", { entry: label }));
    } else if (result.status === "ok" && result.data.full > 0) {
      toast.error(t("dictionary.problem.full"));
    } else {
      toast.error(t("dictionary.actionFailed"));
    }
  };
  const reject = async () => {
    const result = await commands.forkDictionaryReject([entry.id]);
    if (result.status === "ok") {
      void useDictionaryStore.getState().reload();
      toast(t("dictionary.rejected", { entry: label }));
    } else {
      toast.error(t("dictionary.actionFailed"));
    }
  };
  return (
    <li
      data-testid="wb-dictionary-entry"
      data-kind={entry.kind}
      data-id={entry.id}
      className="group flex items-center gap-3 border-b border-(--wb-line) px-1 py-2.5 last:border-b-0"
    >
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-3 gap-y-0.5">
        <span className="flex min-w-0 items-center gap-2 text-[14px] text-(--wb-ink)">
          <span className="break-words" data-testid="wb-dictionary-phrase-text">
            {entry.phrase}
          </span>
          {entry.replacement && (
            <>
              <ArrowRight
                size={14}
                aria-hidden="true"
                className="shrink-0 text-(--wb-ink-2) rtl:rotate-180"
              />
              <span className="sr-only">{t("dictionary.entry.becomes")}</span>
              <span
                className="break-words"
                data-testid="wb-dictionary-replacement-text"
              >
                {entry.replacement}
              </span>
            </>
          )}
        </span>
        {(sourceLabel || entry.kind === "rule") && (
          <span className="flex gap-2 text-[12px] text-(--wb-ink-2)">
            {sourceLabel && <span>{sourceLabel}</span>}
            {entry.kind === "rule" && entry.hit_count > 0 && (
              <span data-testid="wb-dictionary-hits">
                {t("dictionary.entry.used", { count: entry.hit_count })}
              </span>
            )}
          </span>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-1">
        {suggested && (
          <>
            <button
              type="button"
              onClick={() => void accept()}
              aria-label={t("dictionary.entry.accept", { entry: label })}
              title={t("dictionary.entry.accept", { entry: label })}
              data-testid="wb-dictionary-accept"
              className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-control) text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
            >
              <Check size={16} aria-hidden="true" />
            </button>
            <button
              type="button"
              onClick={() => void reject()}
              aria-label={t("dictionary.entry.reject", { entry: label })}
              title={t("dictionary.entry.reject", { entry: label })}
              data-testid="wb-dictionary-reject"
              className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-control) text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
            >
              <X size={16} aria-hidden="true" />
            </button>
          </>
        )}
        {!suggested && (
          <button
            type="button"
            onClick={() => onEdit(entry)}
            aria-label={t("dictionary.entry.edit", { entry: label })}
            data-testid="wb-dictionary-edit"
            className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-control) text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
          >
            <Pencil size={16} aria-hidden="true" />
          </button>
        )}
        {!suggested && (
          <button
            type="button"
            onClick={() => onDelete(entry)}
            aria-label={t("dictionary.entry.delete", { entry: label })}
            data-testid="wb-dictionary-delete"
            className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-control) text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
          >
            <Trash2 size={16} aria-hidden="true" />
          </button>
        )}
      </div>
    </li>
  );
}

export function DictionaryPage() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  useDictionaryLive();
  const entries = useDictionaryStore((s) => s.entries);
  const counts = useDictionaryStore((s) => s.counts);
  const status = useDictionaryStore((s) => s.status);
  const section = useHubNav((s) => s.section);
  const navSeq = useHubNav((s) => s.seq);
  const [tab, setTab] = useState<TabId>("all");
  const [query, setQuery] = useState("");
  const deferredQuery = useDeferredValue(query);
  const [order, setOrder] = useState<SortOrder>("newest");
  const [dialog, setDialog] = useState<{
    entry: DictionaryEntry | null;
  } | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const appName = t("rust.appName");
  const { settings, pending, setSetting } = useForkSettings();
  const learning = settings?.auto_learn_enabled ?? false;

  useEffect(() => {
    if (section === "add") setDialog({ entry: null });
  }, [section, navSeq]);

  const tabCounts: Record<TabId, number> = useMemo(
    () => ({
      all: counts.words + counts.rules,
      mine: entries.filter((e) => inTab(e, "mine")).length,
      suggested: counts.suggested,
    }),
    [counts, entries],
  );

  const visible = useMemo(() => {
    const key = searchKey(deferredQuery.trim());
    const filtered = entries.filter(
      (e) =>
        inTab(e, tab) &&
        (key === "" ||
          searchKey(e.phrase).includes(key) ||
          (e.replacement !== null && searchKey(e.replacement).includes(key))),
    );
    return sortEntries(filtered, order, i18n.language);
  }, [entries, tab, deferredQuery, order, i18n.language]);

  const refresh = async () => {
    setRefreshing(true);
    await useDictionaryStore.getState().reload();
    setRefreshing(false);
  };

  const remove = async (entry: DictionaryEntry) => {
    const result = await commands.forkDictionaryDelete([entry.id]);
    if (result.status !== "ok") {
      toast.error(t("dictionary.deleteFailed"));
      return;
    }
    void useDictionaryStore.getState().reload();
    const gone = result.data;
    const label = entry.replacement
      ? t("dictionary.entry.rule", {
          from: entry.phrase,
          to: entry.replacement,
        })
      : entry.phrase;
    toast(t("dictionary.deleted", { entry: label }), {
      action: {
        label: t("dictionary.undo"),
        onClick: () => {
          void commands.forkDictionaryRestore(gone).then(() => {
            void useDictionaryStore.getState().reload();
          });
        },
      },
    });
  };

  const exportCsv = async () => {
    try {
      const path = await saveFileDialog({
        defaultPath: t("dictionary.export.fileName", { appName }),
        filters: [
          { name: t("ui.importWizard.csvFilter"), extensions: ["csv"] },
        ],
      });
      if (!path) return;
      const result = await commands.forkDictionaryExport(path);
      if (result.status === "ok") {
        toast.success(t("dictionary.export.done", { count: result.data }));
      } else {
        toast.error(t("dictionary.export.failed"));
      }
    } catch (e) {
      console.warn("fork: dictionary export failed", e);
      toast.error(t("dictionary.export.failed"));
    }
  };

  const tabs = (["all", "mine", "suggested"] as TabId[]).map((id) => ({
    value: id,
    label: t(`dictionary.tabs.${id}`, { count: tabCounts[id] }),
  }));

  const empty = () => {
    if (status === "loading") return null;
    if (status === "unavailable" && entries.length === 0) {
      return (
        <EmptyState
          testId="wb-dictionary-unavailable"
          icon={BookA}
          title={t("dictionary.unavailable.title")}
          body={t("dictionary.unavailable.body")}
          action={{
            label: t("dictionary.refresh"),
            onClick: () => void refresh(),
          }}
        />
      );
    }
    if (query.trim() !== "") {
      return (
        <EmptyState
          testId="wb-dictionary-no-match"
          icon={Search}
          title={t("dictionary.noMatch.title")}
          body={t("dictionary.noMatch.body", { query: query.trim() })}
          action={{
            label: t("dictionary.noMatch.action"),
            onClick: () => setQuery(""),
          }}
        />
      );
    }
    if (tab === "suggested") {
      return (
        <EmptyState
          testId="wb-dictionary-empty-suggested"
          icon={Sparkles}
          title={t("dictionary.emptySuggested.title")}
          body={
            learning
              ? t("dictionary.emptySuggested.bodyOn")
              : t("dictionary.emptySuggested.body", { appName })
          }
        />
      );
    }
    return (
      <EmptyState
        testId="wb-dictionary-empty"
        icon={BookA}
        title={t("dictionary.empty.title")}
        body={t("dictionary.empty.body", { appName })}
        action={{
          label: t("dictionary.add"),
          onClick: () => setDialog({ entry: null }),
        }}
      />
    );
  };

  return (
    <div data-testid="wb-dictionary" className="flex flex-col gap-6">
      <PageHeader title={t("hub.pages.dictionary.title")}>
        <Button
          onClick={() => setImportOpen(true)}
          data-testid="wb-dictionary-import"
        >
          <Upload size={15} aria-hidden="true" />
          {t("dictionary.import.open")}
        </Button>
        <Button
          onClick={() => void exportCsv()}
          disabled={counts.words + counts.rules === 0}
          data-testid="wb-dictionary-export"
        >
          <Download size={15} aria-hidden="true" />
          {t("dictionary.export.open")}
        </Button>
        <Button
          variant="primary"
          onClick={() => setDialog({ entry: null })}
          data-testid="wb-dictionary-add"
        >
          <Plus size={15} aria-hidden="true" />
          {t("dictionary.add")}
        </Button>
      </PageHeader>

      <Banner
        id={BANNER_ID}
        title={t("dictionary.banner.title", { appName })}
        body={t("dictionary.banner.body")}
        examplesLabel={t("dictionary.banner.examplesLabel")}
        examples={[
          t("dictionary.banner.example1"),
          t("dictionary.banner.example2"),
          t("dictionary.banner.example3"),
          t("dictionary.banner.example4"),
        ]}
      />

      <Tabs
        value={tab}
        onValueChange={(v) => setTab(v as TabId)}
        tabs={tabs}
        label={t("dictionary.tabs.label")}
      >
        <div className="mt-4 flex flex-wrap items-center gap-2">
          <div className="relative min-w-48 flex-1">
            <Search
              size={15}
              aria-hidden="true"
              className="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-(--wb-ink-2)"
            />
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              aria-label={t("dictionary.search")}
              placeholder={t("dictionary.search")}
              className={`${INPUT_CLASS} ps-9`}
              data-testid="wb-dictionary-search"
            />
          </div>
          <label className="flex items-center gap-2 text-[13px] text-(--wb-ink-2)">
            {t("dictionary.sort.label")}
            <select
              value={order}
              onChange={(e) => setOrder(e.target.value as SortOrder)}
              className={`rounded-(--wb-radius-control) border border-(--wb-line-strong) bg-(--wb-card) px-2.5 py-2 text-[13px] text-(--wb-ink) ${FOCUS_RING}`}
              data-testid="wb-dictionary-sort"
            >
              {SORT_ORDERS.map((o) => (
                <option key={o} value={o}>
                  {t(`dictionary.sort.${o}`)}
                </option>
              ))}
            </select>
          </label>
          <Button
            variant="ghost"
            onClick={() => void refresh()}
            aria-label={t("dictionary.refresh")}
            title={t("dictionary.refresh")}
            data-testid="wb-dictionary-refresh"
            className="px-2.5"
          >
            <RefreshCw
              size={16}
              aria-hidden="true"
              className={refreshing ? "motion-safe:animate-spin" : ""}
            />
          </Button>
        </div>
        {(["all", "mine", "suggested"] as TabId[]).map((id) => (
          <TabPanel key={id} value={id} className="mt-2">
            {id === tab && id === "suggested" && (
              <div
                className="mb-2 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) px-3 py-1.5"
                data-testid="wb-dictionary-learn"
              >
                <ForkToggle
                  label={t("dictionary.learn.toggle")}
                  hint={t("dictionary.learn.hint", { appName })}
                  checked={learning}
                  disabled={!!pending.auto_learn_enabled}
                  onChange={(checked) =>
                    void setSetting("auto_learn_enabled", checked)
                  }
                  testId="wb-dictionary-learn-toggle"
                />
              </div>
            )}
            {id === tab &&
              (visible.length > 0 ? (
                <>
                  <p
                    className="sr-only"
                    role="status"
                    data-testid="wb-dictionary-count"
                  >
                    {t("dictionary.shown", { count: visible.length })}
                  </p>
                  <ul
                    aria-label={t(`dictionary.tabs.${id}`, {
                      count: tabCounts[id],
                    })}
                    data-testid="wb-dictionary-list"
                    className="rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) px-3"
                  >
                    {visible.map((e) => (
                      <EntryRow
                        key={e.id}
                        entry={e}
                        onEdit={(entry) => setDialog({ entry })}
                        onDelete={(entry) => void remove(entry)}
                      />
                    ))}
                  </ul>
                </>
              ) : (
                empty()
              ))}
          </TabPanel>
        ))}
      </Tabs>

      <AddWordDialog
        open={dialog !== null}
        entry={dialog?.entry ?? null}
        onOpenChange={(open) => {
          if (!open) {
            setDialog(null);
            if (section === "add") navigate("dictionary");
          }
        }}
        onSaved={(entry, added) => {
          if (added) {
            toast.success(
              t("dictionary.added", {
                entry: entry.replacement
                  ? t("dictionary.entry.rule", {
                      from: entry.phrase,
                      to: entry.replacement,
                    })
                  : entry.phrase,
              }),
            );
          }
        }}
      />
      <DictionaryImportWizard open={importOpen} onOpenChange={setImportOpen} />
    </div>
  );
}
