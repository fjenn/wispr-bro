// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Dictionary page (S-16, C-58..C-65): the Hub's Dictionary slot.
 *
 *   Dictionary                                   [Import] [Add word]
 *   All (n) · Added by you (n) · Suggested (n)   🔍 ⇅ ⟳ ⤓
 *   ┌ banner: Teach Wispr Bro your words · [Add word] chips ───── × ┐
 *   Siobhán                                           ✎  🗑
 *   gonna → going to        Used 3 times              ✎  🗑
 *
 * Words go to the recognizer (Handy's custom words), replacements to the
 * text pipeline; both are added here. "Suggested" holds S-34's auto-learn
 * suggestions (Accept ✓ / Reject ✕) under the "Learn from my corrections"
 * toggle (off by default, DEC-20); a rejected one never returns. Counts
 * follow every change live (`useDictionaryLive`). Deleting offers Undo.
 * The banner's chips (flowmatch) are names and terms from recent History
 * not in the dictionary yet (`suggestWords`), or examples; a click opens
 * Add word with it filled in.
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
} from "../ui/icons";
import { commands, type DictionaryEntry } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { PageHeader } from "../ui/PageHeader";
import { navigate, useHubNav } from "../hub/nav";
import {
  Banner,
  Button,
  EmptyState,
  FOCUS_RING,
  IconButton,
  ListContainer,
  SearchToggle,
  SettingRow,
  SortMenu,
  Switch,
  TabPanel,
  Tabs,
} from "../ui";
import { useForkSettings } from "../settings/useForkSettings";
import { AddWordDialog } from "./AddWordDialog";
import { suggestWords } from "./suggestions";
import { DictionaryImportWizard } from "./ImportWizard";
import {
  SORT_ORDERS,
  searchKey,
  sortEntries,
  useDictionaryLive,
  useDictionaryStore,
  type SortOrder,
} from "./useDictionary";
import { ForkArt, photos } from "@/assets/fork/art";

type TabId = "all" | "mine" | "suggested";

const BANNER_ID = "dictionary.teach";

/** A row's icon button: bare ink glyph, a press fill on hover. */
const ROW_ACTION = `grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink) hover:bg-(--wb-fill-press) ${FOCUS_RING}`;

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
      className="group flex min-h-(--wb-row-h) items-center gap-3 px-4 py-2.5 transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-surface-tint) focus-within:bg-(--wb-surface-tint)"
    >
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-3 gap-y-0.5">
        <span className="flex min-w-0 items-center gap-2 text-[15px] leading-[22px] text-(--wb-ink)">
          <span className="break-words" data-testid="wb-dictionary-phrase-text">
            {entry.phrase}
          </span>
          {entry.replacement && (
            <>
              <ArrowRight
                size={16}
                aria-hidden="true"
                className="shrink-0 text-(--wb-ink-3) rtl:rotate-180"
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
          <span className="flex gap-2 text-[13px] text-(--wb-ink-3)">
            {sourceLabel && <span>{sourceLabel}</span>}
            {entry.kind === "rule" && entry.hit_count > 0 && (
              <span data-testid="wb-dictionary-hits">
                {t("dictionary.entry.used", { count: entry.hit_count })}
              </span>
            )}
          </span>
        )}
      </div>
      <div
        className={`flex shrink-0 items-center gap-0.5 transition-opacity duration-(--wb-dur-fast) ${suggested ? "" : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 [@media(hover:none)]:opacity-100"}`}
      >
        {suggested && (
          <>
            <button
              type="button"
              onClick={() => void accept()}
              aria-label={t("dictionary.entry.accept", { entry: label })}
              title={t("dictionary.entry.accept", { entry: label })}
              data-testid="wb-dictionary-accept"
              className={`${ROW_ACTION}`}
            >
              <Check size={18} aria-hidden="true" />
            </button>
            <button
              type="button"
              onClick={() => void reject()}
              aria-label={t("dictionary.entry.reject", { entry: label })}
              title={t("dictionary.entry.reject", { entry: label })}
              data-testid="wb-dictionary-reject"
              className={`${ROW_ACTION}`}
            >
              <X size={18} aria-hidden="true" />
            </button>
          </>
        )}
        {!suggested && (
          <button
            type="button"
            onClick={() => onEdit(entry)}
            aria-label={t("dictionary.entry.edit", { entry: label })}
            data-testid="wb-dictionary-edit"
            className={`${ROW_ACTION}`}
          >
            <Pencil size={18} aria-hidden="true" />
          </button>
        )}
        {!suggested && (
          <button
            type="button"
            onClick={() => onDelete(entry)}
            aria-label={t("dictionary.entry.delete", { entry: label })}
            data-testid="wb-dictionary-delete"
            className={`${ROW_ACTION}`}
          >
            <Trash2 size={18} aria-hidden="true" />
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
    draft?: string;
  } | null>(null);
  const [recentTexts, setRecentTexts] = useState<string[]>([]);
  const [importOpen, setImportOpen] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const appName = t("rust.appName");
  const { settings, pending, setSetting } = useForkSettings();
  const learning = settings?.auto_learn_enabled ?? false;

  useEffect(() => {
    if (section === "add") setDialog({ entry: null });
  }, [section, navSeq]);

  // The banner's suggestions read the latest dictations once per visit.
  useEffect(() => {
    let active = true;
    void commands
      .forkHistoryPage(null, 200)
      .then((r) => {
        if (active && r.status === "ok")
          setRecentTexts(r.data.rows.map((row) => row.text));
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);
  const suggestions = useMemo(() => {
    const found = suggestWords(
      recentTexts,
      entries.map((e) => e.phrase),
    );
    if (found.length > 0) return found;
    return [
      t("dictionary.banner.example1"),
      t("dictionary.banner.example2"),
      t("dictionary.banner.example4"),
    ];
  }, [recentTexts, entries, t]);

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
          // Audit OT2: a failed or skipped restore is said, not swallowed.
          void commands
            .forkDictionaryRestore(gone)
            .then((restored) => {
              if (restored.status !== "ok") {
                toast.error(t("dictionary.undoFailed", { entry: label }));
              } else if (restored.data < gone.length) {
                toast.error(t("dictionary.undoSkipped", { entry: label }));
              }
            })
            .catch(() => {
              toast.error(t("dictionary.undoFailed", { entry: label }));
            })
            .finally(() => {
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
    if (status === "broken" && entries.length === 0) {
      return (
        <EmptyState
          testId="wb-dictionary-broken"
          art={<ForkArt name="empty-unavailable" width={160} />}
          icon={BookA}
          title={t("dictionary.unavailable.brokenTitle")}
          body={t("dictionary.unavailable.brokenBody", { appName })}
          action={{
            label: t("dictionary.refresh"),
            onClick: () => void refresh(),
          }}
        />
      );
    }
    if (status === "unavailable" && entries.length === 0) {
      return (
        <EmptyState
          testId="wb-dictionary-unavailable"
          art={<ForkArt name="empty-unavailable" width={160} />}
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
          art={<ForkArt name="empty-search" width={160} />}
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
          art={<ForkArt name="empty-suggested" width={160} />}
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
        art={<ForkArt name="empty-dictionary" width={160} />}
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
    <div data-testid="wb-dictionary" className="flex flex-col gap-14">
      <PageHeader title={t("hub.pages.dictionary.title")}>
        <Button
          variant="primary"
          size="lg"
          onClick={() => setImportOpen(true)}
          data-testid="wb-dictionary-import"
        >
          {t("dictionary.import.open")}
        </Button>
        <Button
          variant="primary"
          size="lg"
          onClick={() => setDialog({ entry: null })}
          data-testid="wb-dictionary-add"
        >
          {t("dictionary.addNew")}
        </Button>
      </PageHeader>

      <Tabs
        value={tab}
        onValueChange={(v) => setTab(v as TabId)}
        tabs={tabs}
        label={t("dictionary.tabs.label")}
        end={
          <>
            <SearchToggle
              value={query}
              onChange={setQuery}
              label={t("dictionary.search")}
              testId="wb-dictionary-search"
            />
            <SortMenu
              value={order}
              onChange={setOrder}
              label={t("dictionary.sort.label")}
              testId="wb-dictionary-sort"
              options={SORT_ORDERS.map((o) => ({
                value: o,
                label: t(`dictionary.sort.${o}`),
              }))}
            />
            <IconButton
              onClick={() => void refresh()}
              aria-label={t("dictionary.refresh")}
              title={t("dictionary.refresh")}
              data-testid="wb-dictionary-refresh"
            >
              <RefreshCw
                size={18}
                aria-hidden="true"
                className={refreshing ? "motion-safe:animate-spin" : ""}
              />
            </IconButton>
            <IconButton
              onClick={() => void exportCsv()}
              disabled={counts.words + counts.rules === 0}
              aria-label={t("dictionary.export.open")}
              title={t("dictionary.export.open")}
              data-testid="wb-dictionary-export"
            >
              <Download size={18} aria-hidden="true" />
            </IconButton>
          </>
        }
      >
        <div className="mt-[25px]">
          <Banner
            id={BANNER_ID}
            image={photos.dictionary.src}
            imageLift={1.15}
            title={t("dictionary.banner.title", { appName })}
            body={t("dictionary.banner.lead")}
            action={{
              label: t("dictionary.banner.add"),
              onClick: () => setDialog({ entry: null }),
              testId: "wb-dictionary-banner-add",
            }}
            suggestionsLabel={t("dictionary.banner.suggestionsLabel")}
            suggestions={suggestions.map((word) => ({
              id: word,
              label: word,
              testId: "wb-dictionary-suggestion",
              onClick: () => setDialog({ entry: null, draft: word }),
            }))}
          />
        </div>
        {(["all", "mine", "suggested"] as TabId[]).map((id) => (
          <TabPanel key={id} value={id} className="mt-6">
            {id === tab && id === "suggested" && (
              <SettingRow
                className="mb-3"
                testId="wb-dictionary-learn"
                title={t("dictionary.learn.toggle")}
                description={t("dictionary.learn.hint", { appName })}
                control={(ids) => (
                  <Switch
                    checked={learning}
                    disabled={!!pending.auto_learn_enabled}
                    onChange={(checked) =>
                      void setSetting("auto_learn_enabled", checked)
                    }
                    ariaLabelledBy={ids.labelledBy}
                    ariaDescribedBy={ids.describedBy}
                    testId="wb-dictionary-learn-toggle"
                  />
                )}
              />
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
                  <ListContainer
                    aria-label={t(`dictionary.tabs.${id}`, {
                      count: tabCounts[id],
                    })}
                    data-testid="wb-dictionary-list"
                  >
                    {visible.map((e) => (
                      <EntryRow
                        key={e.id}
                        entry={e}
                        onEdit={(entry) => setDialog({ entry })}
                        onDelete={(entry) => void remove(entry)}
                      />
                    ))}
                  </ListContainer>
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
        draft={dialog?.draft}
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
