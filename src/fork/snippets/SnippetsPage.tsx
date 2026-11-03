// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Snippets page (S-17, C-69..C-73): the Hub's Snippets slot.
 *
 *   Snippets                          [Import] [Export] [Add snippet]
 *   ┌ banner: say a few words, get a whole text · 3 examples ─── × ┐
 *   3 snippets · used 12 times
 *   [search…]                              [sort ▾] [refresh]
 *   my address    Alex Morgan ⏎ 12 Harbour …   Used 4 times   ✎ 🗑
 *
 * Say the trigger alone or inside a sentence; the saved text is written
 * instead (stage 11, after the LLM). Counts follow every change and every
 * take live (`useSnippetsLive`). Deleting offers Undo.
 */
import { useDeferredValue, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import {
  Download,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  TextQuote,
  Trash2,
  Upload,
} from "../ui/icons";
import { commands, type SnippetEntry } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { PageHeader } from "../hub/PageHeader";
import { navigate, useHubNav } from "../hub/nav";
import { INPUT_CLASS } from "../dictionary/AddWordDialog";
import { Banner, Button, EmptyState, FOCUS_RING } from "../ui";
import { SnippetDialog } from "./SnippetDialog";
import { SnippetsImportWizard } from "./ImportWizard";
import {
  SORT_ORDERS,
  searchKey,
  sortSnippets,
  useSnippetsLive,
  useSnippetsStore,
  type SortOrder,
} from "./useSnippets";

const BANNER_ID = "snippets.examples";

function SnippetRow({
  snippet,
  onEdit,
  onDelete,
}: {
  snippet: SnippetEntry;
  onEdit: (s: SnippetEntry) => void;
  onDelete: (s: SnippetEntry) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const lines = snippet.body.split("\n");
  return (
    <li
      data-testid="wb-snippet-entry"
      data-id={snippet.id}
      className="group flex items-start gap-3 border-b border-(--wb-line) px-1 py-3 last:border-b-0"
    >
      <div className="flex min-w-0 flex-1 flex-col gap-1 sm:flex-row sm:gap-4">
        <span
          className="shrink-0 text-[14px] font-medium break-words text-(--wb-ink) sm:w-48"
          data-testid="wb-snippet-trigger-text"
        >
          {snippet.trigger}
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="sr-only">{t("snippets.entry.becomes")}</span>
          <span
            className="line-clamp-3 text-[13.5px] whitespace-pre-line break-words text-(--wb-ink-2)"
            data-testid="wb-snippet-body-text"
          >
            {snippet.body}
          </span>
          <span className="flex flex-wrap gap-2 text-[12px] text-(--wb-ink-2)">
            {lines.length > 1 && (
              <span>{t("snippets.entry.lines", { count: lines.length })}</span>
            )}
            {snippet.source === "import" && (
              <span>{t("snippets.entry.imported")}</span>
            )}
            {snippet.hit_count > 0 && (
              <span data-testid="wb-snippet-hits">
                {t("snippets.entry.used", { count: snippet.hit_count })}
              </span>
            )}
          </span>
        </span>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <button
          type="button"
          onClick={() => onEdit(snippet)}
          aria-label={t("snippets.entry.edit", { trigger: snippet.trigger })}
          data-testid="wb-snippet-edit"
          className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
        >
          <Pencil size={16} aria-hidden="true" />
        </button>
        <button
          type="button"
          onClick={() => onDelete(snippet)}
          aria-label={t("snippets.entry.delete", { trigger: snippet.trigger })}
          data-testid="wb-snippet-delete"
          className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
        >
          <Trash2 size={16} aria-hidden="true" />
        </button>
      </div>
    </li>
  );
}

export function SnippetsPage() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  useSnippetsLive();
  const snippets = useSnippetsStore((s) => s.snippets);
  const counts = useSnippetsStore((s) => s.counts);
  const status = useSnippetsStore((s) => s.status);
  const section = useHubNav((s) => s.section);
  const navSeq = useHubNav((s) => s.seq);
  const [query, setQuery] = useState("");
  const deferredQuery = useDeferredValue(query);
  const [order, setOrder] = useState<SortOrder>("newest");
  const [dialog, setDialog] = useState<{
    snippet: SnippetEntry | null;
  } | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const appName = t("rust.appName");

  useEffect(() => {
    if (section === "add") setDialog({ snippet: null });
  }, [section, navSeq]);

  const visible = useMemo(() => {
    const key = searchKey(deferredQuery.trim());
    const filtered = snippets.filter(
      (s) =>
        key === "" ||
        searchKey(s.trigger).includes(key) ||
        searchKey(s.body).includes(key),
    );
    return sortSnippets(filtered, order, i18n.language);
  }, [snippets, deferredQuery, order, i18n.language]);

  const refresh = async () => {
    setRefreshing(true);
    await useSnippetsStore.getState().reload();
    setRefreshing(false);
  };

  const remove = async (snippet: SnippetEntry) => {
    const result = await commands.forkSnippetsDelete([snippet.id]);
    if (result.status !== "ok") {
      toast.error(t("snippets.deleteFailed"));
      return;
    }
    void useSnippetsStore.getState().reload();
    const gone = result.data;
    toast(t("snippets.deleted", { trigger: snippet.trigger }), {
      action: {
        label: t("snippets.undo"),
        onClick: () => {
          void commands.forkSnippetsRestore(gone).then(() => {
            void useSnippetsStore.getState().reload();
          });
        },
      },
    });
  };

  const exportCsv = async () => {
    try {
      const path = await saveFileDialog({
        defaultPath: t("snippets.export.fileName", { appName }),
        filters: [
          { name: t("ui.importWizard.csvFilter"), extensions: ["csv"] },
        ],
      });
      if (!path) return;
      const result = await commands.forkSnippetsExport(path);
      if (result.status === "ok") {
        toast.success(t("snippets.export.done", { count: result.data }));
      } else {
        toast.error(t("snippets.export.failed"));
      }
    } catch (e) {
      console.warn("fork: snippet export failed", e);
      toast.error(t("snippets.export.failed"));
    }
  };

  const empty = () => {
    if (status === "loading") return null;
    if (status === "unavailable" && snippets.length === 0) {
      return (
        <EmptyState
          testId="wb-snippets-unavailable"
          icon={TextQuote}
          title={t("snippets.unavailable.title")}
          body={t("snippets.unavailable.body")}
          action={{
            label: t("snippets.refresh"),
            onClick: () => void refresh(),
          }}
        />
      );
    }
    if (query.trim() !== "") {
      return (
        <EmptyState
          testId="wb-snippets-no-match"
          icon={Search}
          title={t("snippets.noMatch.title")}
          body={t("snippets.noMatch.body", { query: query.trim() })}
          action={{
            label: t("snippets.noMatch.action"),
            onClick: () => setQuery(""),
          }}
        />
      );
    }
    return (
      <EmptyState
        testId="wb-snippets-empty"
        icon={TextQuote}
        title={t("snippets.empty.title")}
        body={t("snippets.empty.body", { appName })}
        action={{
          label: t("snippets.add"),
          onClick: () => setDialog({ snippet: null }),
        }}
      />
    );
  };

  return (
    <div data-testid="wb-snippets" className="flex flex-col gap-6">
      <PageHeader title={t("hub.pages.snippets.title")}>
        <Button
          onClick={() => setImportOpen(true)}
          data-testid="wb-snippets-import"
        >
          <Upload size={16} aria-hidden="true" />
          {t("snippets.import.open")}
        </Button>
        <Button
          onClick={() => void exportCsv()}
          disabled={counts.snippets === 0}
          data-testid="wb-snippets-export"
        >
          <Download size={16} aria-hidden="true" />
          {t("snippets.export.open")}
        </Button>
        <Button
          variant="primary"
          onClick={() => setDialog({ snippet: null })}
          data-testid="wb-snippets-add"
        >
          <Plus size={16} aria-hidden="true" />
          {t("snippets.add")}
        </Button>
      </PageHeader>

      <Banner
        id={BANNER_ID}
        title={t("snippets.banner.title")}
        body={t("snippets.banner.body", { appName })}
        examplesLabel={t("snippets.banner.examplesLabel")}
        examples={[
          t("snippets.banner.example1"),
          t("snippets.banner.example2"),
          t("snippets.banner.example3"),
        ]}
      />

      <section
        aria-labelledby="wb-snippets-heading"
        className="flex flex-col gap-3"
      >
        <h2
          id="wb-snippets-heading"
          className="text-[14px] font-medium text-(--wb-ink-2)"
          data-testid="wb-snippets-counts"
          data-count={counts.snippets}
          data-uses={counts.uses}
        >
          {t("snippets.counts.snippets", { count: counts.snippets })}
          {counts.uses > 0 &&
            ` · ${t("snippets.counts.uses", { count: counts.uses })}`}
        </h2>
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative min-w-48 flex-1">
            <Search
              size={16}
              aria-hidden="true"
              className="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-(--wb-ink-2)"
            />
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              aria-label={t("snippets.search")}
              placeholder={t("snippets.search")}
              className={`${INPUT_CLASS} ps-9`}
              data-testid="wb-snippets-search"
            />
          </div>
          <label className="flex items-center gap-2 text-[13px] text-(--wb-ink-2)">
            {t("snippets.sort.label")}
            <select
              value={order}
              onChange={(e) => setOrder(e.target.value as SortOrder)}
              className={`rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) px-2.5 py-2 text-[13px] text-(--wb-ink) ${FOCUS_RING}`}
              data-testid="wb-snippets-sort"
            >
              {SORT_ORDERS.map((o) => (
                <option key={o} value={o}>
                  {t(`snippets.sort.${o}`)}
                </option>
              ))}
            </select>
          </label>
          <Button
            variant="ghost"
            onClick={() => void refresh()}
            aria-label={t("snippets.refresh")}
            title={t("snippets.refresh")}
            data-testid="wb-snippets-refresh"
            className="px-2.5"
          >
            <RefreshCw
              size={16}
              aria-hidden="true"
              className={refreshing ? "motion-safe:animate-spin" : ""}
            />
          </Button>
        </div>
        {visible.length > 0 ? (
          <>
            <p
              className="sr-only"
              role="status"
              data-testid="wb-snippets-shown"
            >
              {t("snippets.shown", { count: visible.length })}
            </p>
            <ul
              aria-labelledby="wb-snippets-heading"
              data-testid="wb-snippets-list"
              className="rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) px-3"
            >
              {visible.map((s) => (
                <SnippetRow
                  key={s.id}
                  snippet={s}
                  onEdit={(snippet) => setDialog({ snippet })}
                  onDelete={(snippet) => void remove(snippet)}
                />
              ))}
            </ul>
          </>
        ) : (
          empty()
        )}
      </section>

      <SnippetDialog
        open={dialog !== null}
        snippet={dialog?.snippet ?? null}
        onOpenChange={(open) => {
          if (!open) {
            setDialog(null);
            if (section === "add") navigate("snippets");
          }
        }}
        onSaved={(snippet, added) => {
          if (added) {
            toast.success(t("snippets.added", { trigger: snippet.trigger }));
          }
        }}
      />
      <SnippetsImportWizard open={importOpen} onOpenChange={setImportOpen} />
    </div>
  );
}
