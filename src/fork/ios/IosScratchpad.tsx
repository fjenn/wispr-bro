// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Scratchpad tab: the Mac's notes (`../scratchpad/ScratchpadPage`, the
 * same store and commands) as a phone list. Pinned notes first, a search
 * field, a row menu to pin or delete, New note. A tap opens the note's
 * editor (`./IosNote`, `#/hub/scratchpad/<id>`); the Mac's floating note
 * window, its shortcut and "Add to the bar" do not exist here.
 */
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ForkArt } from "@/assets/fork/art";
import { commands, events, type Note } from "../bindings";
import { navigate } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import {
  Button,
  Dialog,
  EmptyState,
  IconButton,
  Menu,
  SearchField,
} from "../ui";
import { MoreHorizontal, Pin, PinOff, Plus, Trash2 } from "../ui/icons";
import { IosTitle } from "./parts";

/** How old an empty note is before the list removes it. */
const EMPTY_NOTE_MS = 60_000;

/** Today: the time; this year: day and month; else the full date. */
function useNoteDate(language: string): (ms: number) => string {
  return (ms) => {
    const date = new Date(ms);
    const now = new Date();
    if (date.toDateString() === now.toDateString()) {
      return new Intl.DateTimeFormat(language, { timeStyle: "short" }).format(
        date,
      );
    }
    return new Intl.DateTimeFormat(language, {
      day: "numeric",
      month: "short",
      year: date.getFullYear() === now.getFullYear() ? undefined : "numeric",
    }).format(date);
  };
}

export function IosScratchpad() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const [notes, setNotes] = useState<Note[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [creating, setCreating] = useState(false);
  const [doomed, setDoomed] = useState<Note | null>(null);
  const noteDate = useNoteDate(i18n.language);

  const load = useCallback(async (search: string) => {
    try {
      const result = await commands.forkScratchpadList(search || null);
      if (result.status === "ok") {
        // A note that stayed empty (the app was closed with it open) is
        // removed, as the editor does when it closes.
        const stale = result.data.filter(
          (note) =>
            !note.body.trim() && Date.now() - note.updated_at > EMPTY_NOTE_MS,
        );
        for (const note of stale) {
          void commands.forkScratchpadDelete(note.id).catch(() => undefined);
        }
        setNotes(result.data.filter((note) => !stale.includes(note)));
        setError(false);
      } else setError(true);
    } catch {
      setError(true);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => void load(query), query ? 150 : 0);
    return () => window.clearTimeout(timer);
  }, [load, query]);

  useEffect(() => {
    let active = true;
    let off: (() => void) | undefined;
    void events.forkScratchpadChanged
      .listen(() => void load(query))
      .then((unlisten) => {
        if (active) off = unlisten;
        else unlisten();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      off?.();
    };
  }, [load, query]);

  const create = async () => {
    if (creating) return;
    setCreating(true);
    try {
      const result = await commands.forkScratchpadNew();
      if (result.status === "ok") {
        navigate("scratchpad", String(result.data.id));
      } else setError(true);
    } catch {
      setError(true);
    }
    setCreating(false);
  };
  const pin = async (note: Note) => {
    const result = await commands
      .forkScratchpadSetPinned(note.id, !note.pinned)
      .catch(() => null);
    if (result?.status === "ok") await load(query);
    else setError(true);
  };
  const remove = async (note: Note) => {
    setDoomed(null);
    const result = await commands
      .forkScratchpadDelete(note.id)
      .catch(() => null);
    if (result?.status === "ok") {
      setNotes((current) => current.filter((item) => item.id !== note.id));
    } else setError(true);
  };

  const pinned = notes.filter((note) => note.pinned);
  const others = notes.filter((note) => !note.pinned);
  const empty = !loading && notes.length === 0;
  const blank = empty && !query;

  const row = (note: Note) => {
    const name = note.title || t("scratchpad.untitled");
    const rest = note.body
      .split("\n")
      .filter((line) => line.trim())
      .slice(1)
      .join(" ")
      .trim();
    return (
      <li
        key={note.id}
        className="wb-ios-row wb-ios-note-row"
        data-testid={`wb-note-${note.id}`}
      >
        <button
          type="button"
          className="wb-ios-note-open"
          onClick={() => navigate("scratchpad", String(note.id))}
          data-testid={`wb-note-edit-${note.id}`}
        >
          <span
            dir="auto"
            className={`truncate text-[16px] leading-[22px] font-semibold ${note.title ? "text-(--wb-ink)" : "text-(--wb-ink-3)"}`}
          >
            {name}
          </span>
          <span className="flex min-w-0 items-baseline gap-2 text-[14px] leading-[20px]">
            <span className="shrink-0 text-(--wb-ink-3) tabular-nums">
              {noteDate(note.updated_at)}
            </span>
            <span dir="auto" className="min-w-0 truncate text-(--wb-ink-2)">
              {rest || (note.body.trim() ? "" : t("scratchpad.emptyNote"))}
            </span>
          </span>
        </button>
        <Menu
          label={t("ios.scratchpad.noteMenu", { name })}
          align="end"
          testId={`wb-note-menu-${note.id}`}
          trigger={
            <IconButton
              aria-label={t("ios.scratchpad.noteMenu", { name })}
              className="-me-2 text-(--wb-ink-3)!"
              data-testid={`wb-note-more-${note.id}`}
            >
              <MoreHorizontal size={20} />
            </IconButton>
          }
          groups={[
            {
              items: [
                {
                  id: "pin",
                  label: t(note.pinned ? "scratchpad.unpin" : "scratchpad.pin"),
                  icon: note.pinned ? PinOff : Pin,
                  onSelect: () => void pin(note),
                },
                {
                  id: "delete",
                  label: t("scratchpad.delete"),
                  icon: Trash2,
                  onSelect: () => setDoomed(note),
                },
              ],
            },
          ]}
        />
      </li>
    );
  };

  const group = (label: string, items: Note[], testId: string) =>
    items.length > 0 && (
      <section className="flex flex-col gap-2" data-testid={testId}>
        <h2 className="type-micro px-4 text-(--wb-ink-3)">{label}</h2>
        <ul className="wb-ios-group" aria-label={label}>
          {items.map(row)}
        </ul>
      </section>
    );

  return (
    <div
      className="wb-ios-scratchpad flex flex-col gap-4"
      data-testid="wb-ios-scratchpad"
    >
      <div className="flex min-h-10 items-center justify-between gap-3">
        <IosTitle>{t("hub.pages.scratchpad.title")}</IosTitle>
        {!blank && (
          <Button
            variant="primary"
            size="sm"
            leading={<Plus size={16} />}
            onClick={() => void create()}
            disabled={creating}
            className="wb-ios-pill"
            data-testid="wb-scratchpad-new"
          >
            {t("scratchpad.newNote")}
          </Button>
        )}
      </div>
      {!blank && (
        <SearchField
          value={query}
          onChange={(event) => setQuery(event.currentTarget.value)}
          aria-label={t("scratchpad.searchLabel")}
          placeholder={t("scratchpad.searchPlaceholder")}
          autoCapitalize="off"
          autoCorrect="off"
          enterKeyHint="search"
          className="wb-ios-search"
          data-testid="wb-scratchpad-search"
        />
      )}
      {error && (
        <p
          role="alert"
          className="rounded-(--wb-radius-md) bg-(--wb-danger-soft) px-4 py-3 text-[15px] leading-[21px] text-(--wb-ink)"
        >
          {t("ios.scratchpad.error")}
        </p>
      )}
      {blank && (
        <EmptyState
          art={<ForkArt name="empty-scratchpad" width={168} />}
          title={t("scratchpad.empty")}
          body={t("ios.scratchpad.emptyHelp")}
          action={{
            label: t("scratchpad.newNote"),
            onClick: () => void create(),
          }}
          testId="wb-scratchpad-empty"
        />
      )}
      {empty && !blank && (
        <EmptyState
          compact
          title={t("scratchpad.noResults")}
          body={t("ios.scratchpad.noResultsHelp")}
          testId="wb-scratchpad-no-results"
        />
      )}
      {group(t("scratchpad.pinned"), pinned, "wb-scratchpad-pinned")}
      {group(
        pinned.length > 0 ? t("ios.scratchpad.notes") : t("scratchpad.recents"),
        others,
        "wb-scratchpad-notes",
      )}
      <Dialog
        open={doomed !== null}
        onOpenChange={(open) => !open && setDoomed(null)}
        title={t("ios.scratchpad.deleteTitle")}
        closeLabel={t("ios.close")}
        showCloseButton={false}
        testId="wb-note-delete-sheet"
        footer={
          <>
            <Button
              variant="secondary"
              onClick={() => setDoomed(null)}
              data-testid="wb-note-delete-cancel"
            >
              {t("ios.scratchpad.cancel")}
            </Button>
            <Button
              variant="destructive"
              onClick={() => doomed && void remove(doomed)}
              data-testid="wb-note-delete-confirm"
            >
              {t("scratchpad.delete")}
            </Button>
          </>
        }
      >
        <p className="text-[16px] leading-[22px] text-(--wb-ink-2)">
          {t("ios.scratchpad.deleteBody", {
            name: doomed?.title || t("scratchpad.untitled"),
          })}
        </p>
      </Dialog>
    </div>
  );
}
