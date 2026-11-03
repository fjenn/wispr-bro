// SPDX-License-Identifier: GPL-3.0-or-later
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Keyboard,
  Pencil,
  Pin,
  PinOff,
  Plus,
  Search,
  Trash2,
} from "../ui/icons";
import { commands, events, type Note } from "../bindings";
import { PageHeader } from "../hub/PageHeader";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { useForkSettings } from "../settings/useForkSettings";
import { Button, FOCUS_RING } from "../ui";
import { SCRATCHPAD_ACTION, ShortcutDialog } from "./ShortcutDialog";

export function ScratchpadPage() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const [notes, setNotes] = useState<Note[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [shortcutOpen, setShortcutOpen] = useState(false);

  const load = useCallback(async (search: string) => {
    const result = await commands.forkScratchpadList(search || null);
    if (result.status === "ok") {
      setNotes(result.data);
      setError(false);
    } else setError(true);
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
      });
    return () => {
      active = false;
      off?.();
    };
  }, [load, query]);

  const remove = async (note: Note) => {
    if (
      !window.confirm(
        t("scratchpad.deleteConfirm", {
          name: note.title || t("scratchpad.untitled"),
        }),
      )
    )
      return;
    const result = await commands.forkScratchpadDelete(note.id);
    if (result.status === "ok")
      setNotes((current) => current.filter((item) => item.id !== note.id));
    else setError(true);
  };
  const pin = async (note: Note) => {
    const result = await commands.forkScratchpadSetPinned(
      note.id,
      !note.pinned,
    );
    if (result.status === "ok") await load(query);
    else setError(true);
  };
  const binding = settings?.hotkeys[SCRATCHPAD_ACTION]?.[0];

  return (
    <div data-testid="wb-scratchpad-page" className="flex flex-col gap-6">
      <PageHeader title={t("hub.pages.scratchpad.title")}>
        <Button
          variant="primary"
          onClick={() => void commands.forkScratchpadNew()}
          data-testid="wb-scratchpad-new"
        >
          <Plus size={16} aria-hidden="true" />
          {t("scratchpad.newNote")}
        </Button>
      </PageHeader>
      <section
        aria-label={t("scratchpad.preferences")}
        className="grid gap-3 sm:grid-cols-2"
      >
        <label className="flex cursor-pointer items-start gap-3 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-4">
          <input
            type="checkbox"
            role="switch"
            checked={settings?.scratchpad_on_bar ?? false}
            disabled={!settings || !!pending.scratchpad_on_bar}
            onChange={(event) =>
              void setSetting("scratchpad_on_bar", event.currentTarget.checked)
            }
            className="mt-0.5 size-4 accent-(--wb-ink)"
            data-testid="wb-scratchpad-on-bar"
          />
          <span>
            <span className="block text-sm font-semibold">
              {t("scratchpad.addToBar")}
            </span>
            <span className="mt-1 block text-xs text-(--wb-ink-2)">
              {t("scratchpad.addToBarHelp")}
            </span>
          </span>
        </label>
        <div className="flex items-center justify-between gap-3 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-4">
          <div>
            <p className="text-sm font-semibold">
              {t("scratchpad.shortcut.label")}
            </p>
            <p
              className="mt-1 text-xs text-(--wb-ink-2)"
              data-testid="wb-scratchpad-shortcut-value"
            >
              {binding
                ? chordLabel(binding.chord)
                : t("settingsModal.hotkeys.notSet")}
            </p>
          </div>
          <Button
            onClick={() => setShortcutOpen(true)}
            data-testid="wb-scratchpad-shortcut"
          >
            <Keyboard size={16} aria-hidden="true" />
            {t("scratchpad.shortcut.change")}
          </Button>
        </div>
      </section>
      <section
        aria-labelledby="wb-scratchpad-recents"
        className="flex flex-col gap-3"
      >
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2
            id="wb-scratchpad-recents"
            className="font-(--wb-font-display) text-xl"
          >
            {t("scratchpad.recents")}
          </h2>
          <label className="relative block min-w-52 flex-1 sm:max-w-xs">
            <span className="sr-only">{t("scratchpad.searchLabel")}</span>
            <Search
              size={16}
              aria-hidden="true"
              className="pointer-events-none absolute start-3 top-2.5 text-(--wb-ink-3)"
            />
            <input
              type="search"
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder={t("scratchpad.searchPlaceholder")}
              className={`w-full rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) py-2 pe-3 ps-9 text-sm ${FOCUS_RING}`}
              data-testid="wb-scratchpad-search"
            />
          </label>
        </div>
        {error && (
          <p role="alert" className="text-sm text-red-700 dark:text-red-300">
            {t("scratchpad.error")}
          </p>
        )}
        {loading && (
          <p role="status" className="text-sm text-(--wb-ink-2)">
            {t("scratchpad.loading")}
          </p>
        )}
        {!loading && notes.length === 0 && (
          <div className="rounded-(--wb-radius-md) border border-dashed border-(--wb-line-strong) p-8 text-center">
            <p className="text-sm font-medium">
              {query ? t("scratchpad.noResults") : t("scratchpad.empty")}
            </p>
            <p className="mt-1 text-xs text-(--wb-ink-2)">
              {t("scratchpad.emptyHelp")}
            </p>
          </div>
        )}
        <ul
          className="flex flex-col gap-2"
          aria-label={t("scratchpad.recents")}
        >
          {notes.map((note) => (
            <li
              key={note.id}
              className="rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-4"
              data-testid={`wb-note-${note.id}`}
            >
              <div className="flex items-start gap-3">
                <button
                  type="button"
                  onClick={() => void commands.forkScratchpadShowNote(note.id)}
                  className={`min-w-0 flex-1 cursor-pointer text-start ${FOCUS_RING}`}
                  data-testid={`wb-note-edit-${note.id}`}
                >
                  <span className="flex items-center gap-2 text-sm font-semibold">
                    {note.pinned && (
                      <Pin
                        size={14}
                        aria-label={t("scratchpad.pinned")}
                        className="shrink-0 text-(--wb-accent)"
                      />
                    )}
                    <span className="truncate">
                      {note.title || t("scratchpad.untitled")}
                    </span>
                  </span>
                  <span className="mt-1 line-clamp-2 block text-xs text-(--wb-ink-2)">
                    {note.body || t("scratchpad.emptyNote")}
                  </span>
                  <span className="mt-2 block text-[11px] text-(--wb-ink-3)">
                    {new Intl.DateTimeFormat(i18n.language, {
                      dateStyle: "medium",
                    }).format(note.updated_at)}
                  </span>
                </button>
                <div className="flex shrink-0 items-center gap-1">
                  <button
                    type="button"
                    onClick={() =>
                      void commands.forkScratchpadShowNote(note.id)
                    }
                    aria-label={t("scratchpad.edit", {
                      name: note.title || t("scratchpad.untitled"),
                    })}
                    title={t("scratchpad.editAction")}
                    className={`grid size-8 cursor-pointer place-items-center rounded-full text-(--wb-ink-2) hover:bg-(--wb-fill-hover) ${FOCUS_RING}`}
                  >
                    <Pencil size={16} aria-hidden="true" />
                  </button>
                  <button
                    type="button"
                    onClick={() => void pin(note)}
                    aria-label={t(
                      note.pinned ? "scratchpad.unpin" : "scratchpad.pin",
                    )}
                    title={t(
                      note.pinned ? "scratchpad.unpin" : "scratchpad.pin",
                    )}
                    aria-pressed={note.pinned}
                    className={`grid size-8 cursor-pointer place-items-center rounded-full text-(--wb-ink-2) hover:bg-(--wb-fill-hover) ${FOCUS_RING}`}
                    data-testid={`wb-note-pin-${note.id}`}
                  >
                    {note.pinned ? (
                      <PinOff size={16} aria-hidden="true" />
                    ) : (
                      <Pin size={16} aria-hidden="true" />
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={() => void remove(note)}
                    aria-label={t("scratchpad.delete")}
                    title={t("scratchpad.delete")}
                    className={`grid size-8 cursor-pointer place-items-center rounded-full text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-red-700 ${FOCUS_RING}`}
                    data-testid={`wb-note-delete-${note.id}`}
                  >
                    <Trash2 size={16} aria-hidden="true" />
                  </button>
                </div>
              </div>
            </li>
          ))}
        </ul>
      </section>
      {shortcutOpen && (
        <ShortcutDialog onClose={() => setShortcutOpen(false)} />
      )}
    </div>
  );
}
