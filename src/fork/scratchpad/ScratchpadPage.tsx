// SPDX-License-Identifier: GPL-3.0-or-later
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pencil, Pin, PinOff, Trash2 } from "../ui/icons";
import { ForkArt, photos } from "@/assets/fork/art";
import { commands, events, type Note } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { useForkSettings } from "../settings/useForkSettings";
import {
  Button,
  EmptyState,
  FOCUS_RING,
  Hero,
  IconButton,
  PageHeader,
  SearchField,
  Switch,
  Tag,
} from "../ui";
import { SCRATCHPAD_ACTION, ShortcutDialog } from "./ShortcutDialog";

/** The banner's picture: a small note window, drawn (decorative). */
function NotePeek() {
  return (
    <div className="w-[248px] translate-y-9 rounded-t-(--wb-radius-md) bg-(--wb-chrome) p-1.5 pb-0 shadow-(--wb-elev-3) rtl:-scale-x-100">
      <div className="flex items-center gap-1.5 px-1.5 pb-1.5">
        <span className="size-1.5 rounded-full bg-(--wb-ink-3)/50" />
        <span className="size-1.5 rounded-full bg-(--wb-ink-3)/50" />
        <span className="size-1.5 rounded-full bg-(--wb-ink-3)/50" />
        <span className="ms-3 h-3 w-14 rounded-[4px] bg-(--wb-fill-active)" />
        <span className="h-1.5 w-8 rounded-full bg-(--wb-line-2)" />
      </div>
      <div className="flex gap-1.5">
        <div className="flex w-5 flex-col items-center gap-2 pt-1.5">
          <span className="size-2.5 rounded-[3px] border border-(--wb-ink-3)/60" />
          <span className="size-2.5 rounded-[3px] border border-(--wb-ink-3)/60" />
          <span className="size-2.5 rounded-full border border-(--wb-ink-3)/60" />
        </div>
        <div className="flex h-[92px] flex-1 flex-col gap-2 rounded-t-[8px] bg-(--wb-panel) px-3 pt-3">
          <span className="h-1.5 w-[86%] rounded-full bg-(--wb-ink-3)/45" />
          <span className="h-1.5 w-[72%] rounded-full bg-(--wb-ink-3)/45" />
          <span className="h-1.5 w-[80%] rounded-full bg-(--wb-ink-3)/45" />
          <span className="h-1.5 w-[40%] rounded-full bg-(--wb-ink-3)/45" />
        </div>
      </div>
    </div>
  );
}

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
  const dateFormat = new Intl.DateTimeFormat(i18n.language, {
    month: "short",
    day: "2-digit",
  });
  const timeFormat = new Intl.DateTimeFormat(i18n.language, {
    timeStyle: "short",
  });

  return (
    <div data-testid="wb-scratchpad-page" className="flex flex-col gap-6">
      <PageHeader
        title={t("hub.pages.scratchpad.title")}
        tag={<Tag kind="beta" />}
      >
        <section
          aria-label={t("scratchpad.preferences")}
          className="flex flex-wrap items-center justify-end gap-x-4 gap-y-2"
        >
          <span className="flex items-center gap-2.5">
            <label
              htmlFor="wb-scratchpad-on-bar"
              title={t("scratchpad.addToBarHelp")}
              className="cursor-pointer text-[15px] text-(--wb-ink)"
            >
              {t("scratchpad.addToBar")}
            </label>
            <span id="wb-scratchpad-on-bar-help" className="sr-only">
              {t("scratchpad.addToBarHelp")}
            </span>
            <Switch
              id="wb-scratchpad-on-bar"
              checked={settings?.scratchpad_on_bar ?? false}
              disabled={!settings || !!pending.scratchpad_on_bar}
              onChange={(on) => void setSetting("scratchpad_on_bar", on)}
              ariaDescribedBy="wb-scratchpad-on-bar-help"
              testId="wb-scratchpad-on-bar"
            />
          </span>
          <Button
            variant="outline"
            onClick={() => setShortcutOpen(true)}
            data-testid="wb-scratchpad-shortcut"
          >
            <span className={binding ? "text-(--wb-ink-2)" : "sr-only"}>
              {t("scratchpad.shortcut.label")}
            </span>
            <span
              className={binding ? "tracking-wide tabular-nums" : ""}
              data-testid="wb-scratchpad-shortcut-value"
            >
              {binding
                ? chordLabel(binding.chord)
                : t("scratchpad.shortcut.change")}
            </span>
          </Button>
        </section>
      </PageHeader>
      <Hero
        id="scratchpad"
        title={t("scratchpad.hero.title")}
        body={t("scratchpad.hero.body")}
        action={{
          label: t("scratchpad.newNote"),
          onClick: () => void commands.forkScratchpadNew(),
          testId: "wb-scratchpad-new",
        }}
        image={photos.scratchpad.src}
        art={<NotePeek />}
      />
      <section
        aria-labelledby="wb-scratchpad-recents"
        className="flex flex-col gap-4"
      >
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-(--wb-line) pb-3">
          <h2
            id="wb-scratchpad-recents"
            className="type-card-title text-(--wb-ink)"
          >
            {t("scratchpad.recents")}
          </h2>
          <label className="block w-60 max-w-full">
            <span className="sr-only">{t("scratchpad.searchLabel")}</span>
            <SearchField
              size="sm"
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder={t("scratchpad.searchPlaceholder")}
              data-testid="wb-scratchpad-search"
            />
          </label>
        </div>
        {error && (
          <p
            role="alert"
            className="rounded-(--wb-radius-sm) bg-(--wb-danger-soft) px-3 py-2 text-[14px] text-(--wb-ink)"
          >
            {t("scratchpad.error")}
          </p>
        )}
        {loading && (
          <p role="status" className="text-[15px] text-(--wb-ink-2)">
            {t("scratchpad.loading")}
          </p>
        )}
        {!loading && notes.length === 0 && (
          <EmptyState
            compact={Boolean(query)}
            art={
              query ? undefined : (
                <ForkArt name="empty-scratchpad" width={160} />
              )
            }
            title={query ? t("scratchpad.noResults") : t("scratchpad.empty")}
            body={t("scratchpad.emptyHelp")}
          />
        )}
        <ul
          className="flex flex-col gap-3"
          aria-label={t("scratchpad.recents")}
        >
          {notes.map((note) => {
            const name = note.title || t("scratchpad.untitled");
            const rest = note.body.split("\n").slice(1).join(" ").trim();
            return (
              <li
                key={note.id}
                className="group/note relative rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-surface-tint)"
                data-testid={`wb-note-${note.id}`}
              >
                <button
                  type="button"
                  onClick={() => void commands.forkScratchpadShowNote(note.id)}
                  className={`block w-full cursor-pointer rounded-(--wb-radius-md) px-4 pt-4 pb-3.5 text-start ${FOCUS_RING} focus-visible:-outline-offset-2`}
                  data-testid={`wb-note-edit-${note.id}`}
                >
                  <span className="flex items-center gap-2 pe-28 text-[16px] leading-[22px] font-semibold text-(--wb-ink)">
                    {note.pinned && (
                      <Pin
                        size={14}
                        aria-label={t("scratchpad.pinned")}
                        className="shrink-0 text-(--wb-ink-2)"
                      />
                    )}
                    <span className="truncate">{name}</span>
                  </span>
                  <span className="mt-1.5 line-clamp-2 block text-[15px] leading-[22px] text-(--wb-ink-2)">
                    {rest || (note.body ? "" : t("scratchpad.emptyNote"))}
                  </span>
                  <span className="mt-3 flex justify-between gap-3 text-[13px] text-(--wb-ink-3) tabular-nums">
                    <span>{dateFormat.format(note.updated_at)}</span>
                    <span>{timeFormat.format(note.updated_at)}</span>
                  </span>
                </button>
                <div className="absolute end-3 top-3 flex items-center rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface) p-0.5 opacity-0 shadow-(--wb-elev-1) transition-opacity duration-(--wb-dur-fast) group-focus-within/note:opacity-100 group-hover/note:opacity-100">
                  <IconButton
                    size="sm"
                    onClick={() =>
                      void commands.forkScratchpadShowNote(note.id)
                    }
                    aria-label={t("scratchpad.edit", { name })}
                    title={t("scratchpad.editAction")}
                  >
                    <Pencil size={16} />
                  </IconButton>
                  <IconButton
                    size="sm"
                    onClick={() => void pin(note)}
                    aria-label={t(
                      note.pinned ? "scratchpad.unpin" : "scratchpad.pin",
                    )}
                    title={t(
                      note.pinned ? "scratchpad.unpin" : "scratchpad.pin",
                    )}
                    aria-pressed={note.pinned}
                    data-testid={`wb-note-pin-${note.id}`}
                  >
                    {note.pinned ? <PinOff size={16} /> : <Pin size={16} />}
                  </IconButton>
                  <IconButton
                    size="sm"
                    onClick={() => void remove(note)}
                    aria-label={t("scratchpad.delete")}
                    title={t("scratchpad.delete")}
                    className="text-(--wb-danger)! hover:bg-(--wb-danger-soft)!"
                    data-testid={`wb-note-delete-${note.id}`}
                  >
                    <Trash2 size={16} />
                  </IconButton>
                </div>
              </li>
            );
          })}
        </ul>
      </section>
      {shortcutOpen && (
        <ShortcutDialog onClose={() => setShortcutOpen(false)} />
      )}
    </div>
  );
}
