// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Snippets page (S-17, C-69..C-73): the Hub's Snippets slot.
 *
 *   Snippets                                   [Import] [Add snippet]
 *   3 snippets · used 12 times                       🔍 ⇅ ⟳ ⤓
 *   ┌ banner: say a few words, get a whole text ──────────────── × ┐
 *   │ "cue" → text   (three clickable examples)                    │
 *   │ [Add snippet]                                                │
 *   my address    Alex Morgan ⏎ 12 Harbour …   Used 4 times   ✎ 🗑
 *
 * Say the trigger alone or inside a sentence; the saved text is written
 * instead (stage 11, after the LLM). Counts follow every change and every
 * take live (`useSnippetsLive`). Deleting offers Undo. A banner example
 * (flowmatch) opens Add snippet with its cue and text filled in.
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
  SortMenu,
} from "../ui";
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
import { ForkArt, photos } from "@/assets/fork/art";

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
      className="group flex min-h-(--wb-row-h) items-start gap-3 px-4 py-3.5 transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-surface-tint) focus-within:bg-(--wb-surface-tint)"
    >
      <div className="flex min-w-0 flex-1 flex-col gap-1 sm:flex-row sm:gap-6">
        <span
          className="shrink-0 text-[15px] leading-[22px] font-medium break-words text-(--wb-ink) sm:w-44"
          data-testid="wb-snippet-trigger-text"
        >
          {snippet.trigger}
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="sr-only">{t("snippets.entry.becomes")}</span>
          <span
            className="line-clamp-3 text-[15px] leading-[22px] whitespace-pre-line break-words text-(--wb-ink-2)"
            data-testid="wb-snippet-body-text"
          >
            {snippet.body}
          </span>
          <span className="flex flex-wrap gap-2 text-[13px] text-(--wb-ink-3) empty:hidden">
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
      <div className="-my-1 flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity duration-(--wb-dur-fast) group-focus-within:opacity-100 group-hover:opacity-100 [@media(hover:none)]:opacity-100">
        <button
          type="button"
          onClick={() => onEdit(snippet)}
          aria-label={t("snippets.entry.edit", { trigger: snippet.trigger })}
          data-testid="wb-snippet-edit"
          className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink) hover:bg-(--wb-fill-press) ${FOCUS_RING}`}
        >
          <Pencil size={18} aria-hidden="true" />
        </button>
        <button
          type="button"
          onClick={() => onDelete(snippet)}
          aria-label={t("snippets.entry.delete", { trigger: snippet.trigger })}
          data-testid="wb-snippet-delete"
          className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink) hover:bg-(--wb-fill-press) ${FOCUS_RING}`}
        >
          <Trash2 size={18} aria-hidden="true" />
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
    draft?: { trigger: string; body: string };
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
          // Audit OT2: a failed or skipped restore is said, not swallowed.
          const trigger = snippet.trigger;
          void commands
            .forkSnippetsRestore(gone)
            .then((restored) => {
              if (restored.status !== "ok") {
                toast.error(t("snippets.undoFailed", { trigger }));
              } else if (restored.data < gone.length) {
                toast.error(t("snippets.undoSkipped", { trigger }));
              }
            })
            .catch(() => {
              toast.error(t("snippets.undoFailed", { trigger }));
            })
            .finally(() => {
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
          art={<ForkArt name="empty-unavailable" width={160} />}
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
          art={<ForkArt name="empty-search" width={160} />}
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
        art={<ForkArt name="empty-snippets" width={160} />}
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
    <div data-testid="wb-snippets" className="flex flex-col gap-14">
      <PageHeader title={t("hub.pages.snippets.title")}>
        <Button
          variant="primary"
          size="lg"
          onClick={() => setImportOpen(true)}
          data-testid="wb-snippets-import"
        >
          {t("snippets.import.open")}
        </Button>
        <Button
          variant="primary"
          size="lg"
          onClick={() => setDialog({ snippet: null })}
          data-testid="wb-snippets-add"
        >
          {t("snippets.addNew")}
        </Button>
      </PageHeader>

      <section
        aria-labelledby="wb-snippets-heading"
        className="flex flex-col gap-[25px]"
      >
        <h2
          id="wb-snippets-heading"
          className="sr-only"
          data-testid="wb-snippets-counts"
          data-count={counts.snippets}
          data-uses={counts.uses}
        >
          {t("snippets.counts.snippets", { count: counts.snippets })}
          {counts.uses > 0 &&
            ` · ${t("snippets.counts.uses", { count: counts.uses })}`}
        </h2>
        <div className="flex items-end gap-4 border-b border-(--wb-line)">
          <span
            className="-mb-px border-b-2 border-(--wb-ink) pt-1 pb-3 text-[16px] font-medium text-(--wb-ink)"
            aria-hidden="true"
          >
            {t("snippets.tabs.all")}
          </span>
          <div className="ms-auto flex shrink-0 items-center gap-1 pb-1.5">
            <SearchToggle
              value={query}
              onChange={setQuery}
              label={t("snippets.search")}
              testId="wb-snippets-search"
            />
            <SortMenu
              value={order}
              onChange={setOrder}
              label={t("snippets.sort.label")}
              testId="wb-snippets-sort"
              options={SORT_ORDERS.map((o) => ({
                value: o,
                label: t(`snippets.sort.${o}`),
              }))}
            />
            <IconButton
              onClick={() => void refresh()}
              aria-label={t("snippets.refresh")}
              title={t("snippets.refresh")}
              data-testid="wb-snippets-refresh"
            >
              <RefreshCw
                size={18}
                aria-hidden="true"
                className={refreshing ? "motion-safe:animate-spin" : ""}
              />
            </IconButton>
            <IconButton
              onClick={() => void exportCsv()}
              disabled={counts.snippets === 0}
              aria-label={t("snippets.export.open")}
              title={t("snippets.export.open")}
              data-testid="wb-snippets-export"
            >
              <Download size={18} aria-hidden="true" />
            </IconButton>
          </div>
        </div>

        <Banner
          id={BANNER_ID}
          image={photos.snippets.src}
          title={t("snippets.banner.title")}
          body={t("snippets.banner.lead", { appName })}
          pairsLabel={t("snippets.banner.examplesLabel")}
          pairs={([1, 2, 3] as const).map((n) => {
            const trigger = t(`snippets.banner.cue${n}`);
            const body = t(`snippets.banner.text${n}`);
            return {
              id: String(n),
              cue: trigger,
              text: body,
              testId: "wb-snippets-example",
              onClick: () =>
                setDialog({ snippet: null, draft: { trigger, body } }),
            };
          })}
          action={{
            label: t("snippets.banner.add"),
            onClick: () => setDialog({ snippet: null }),
            testId: "wb-snippets-banner-add",
          }}
        />
        {visible.length > 0 ? (
          <>
            <p
              className="sr-only"
              role="status"
              data-testid="wb-snippets-shown"
            >
              {t("snippets.shown", { count: visible.length })}
            </p>
            <ListContainer
              aria-labelledby="wb-snippets-heading"
              data-testid="wb-snippets-list"
            >
              {visible.map((s) => (
                <SnippetRow
                  key={s.id}
                  snippet={s}
                  onEdit={(snippet) => setDialog({ snippet })}
                  onDelete={(snippet) => void remove(snippet)}
                />
              ))}
            </ListContainer>
          </>
        ) : (
          empty()
        )}
      </section>

      <SnippetDialog
        open={dialog !== null}
        snippet={dialog?.snippet ?? null}
        draft={dialog?.draft}
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
