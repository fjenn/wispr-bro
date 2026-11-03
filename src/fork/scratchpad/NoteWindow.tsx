// SPDX-License-Identifier: GPL-3.0-or-later
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Check,
  Copy,
  Expand,
  Minimize,
  PanelLeft,
  Plus,
  Search,
  Type,
  WandSparkles,
  X,
} from "../ui/icons";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { commands, events, type Note } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { FOCUS_RING } from "../ui";
import { NoteTabs } from "./NoteTabs";

const AUTOSAVE_MS = 500;
const TEXT_SIZES = [16, 20, 24] as const;

type SaveState = "saved" | "saving" | "error";

function requestedNote(): number | null {
  const value = Number(new URLSearchParams(window.location.search).get("note"));
  return Number.isSafeInteger(value) && value > 0 ? value : null;
}

export function NoteWindow() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const [notes, setNotes] = useState<Note[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(requestedNote);
  const [body, setBody] = useState("");
  const [savedBody, setSavedBody] = useState("");
  const [saveState, setSaveState] = useState<SaveState>("saved");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [sidebar, setSidebar] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [polishing, setPolishing] = useState(false);
  const [copied, setCopied] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [textSize, setTextSize] = useState<(typeof TEXT_SIZES)[number]>(16);
  const bodyRef = useRef(body);
  const savedBodyRef = useRef(savedBody);
  const selectedRef = useRef(selectedId);
  const saveSequence = useRef(0);
  const searchRef = useRef<HTMLInputElement>(null);
  bodyRef.current = body;
  savedBodyRef.current = savedBody;
  selectedRef.current = selectedId;

  const selected = notes.find((note) => note.id === selectedId) ?? null;

  const loadNotes = useCallback(async (search = "") => {
    const result = await commands.forkScratchpadList(search || null);
    if (result.status === "ok") {
      setNotes((current) => {
        const selectedNote = current.find(
          (note) => note.id === selectedRef.current,
        );
        const next = [...result.data];
        if (selectedNote && !next.some((note) => note.id === selectedNote.id))
          next.unshift(selectedNote);
        return next;
      });
      setError(false);
    } else setError(true);
  }, []);

  const openNote = useCallback(async (id: number) => {
    const previousId = selectedRef.current;
    if (
      previousId &&
      previousId !== id &&
      bodyRef.current !== savedBodyRef.current
    ) {
      const pending = await commands.forkScratchpadSave(
        previousId,
        bodyRef.current,
      );
      if (pending.status === "error") {
        setError(true);
        return;
      }
    }
    const result = await commands.forkScratchpadGet(id);
    if (result.status === "error" || !result.data) {
      setError(true);
      return;
    }
    const note = result.data;
    saveSequence.current += 1;
    setSelectedId(id);
    setNotes((current) => [note, ...current.filter((item) => item.id !== id)]);
    setBody(note.body);
    setSavedBody(note.body);
    setSaveState("saved");
    setLoading(false);
    setError(false);
  }, []);

  useEffect(() => {
    const id = requestedNote();
    if (id) void openNote(id);
    else setLoading(false);
    void loadNotes();
  }, [openNote, loadNotes]);

  useEffect(() => {
    let active = true;
    let offOpen: (() => void) | undefined;
    let offChanged: (() => void) | undefined;
    void events.forkScratchpadOpen
      .listen((event) => void openNote(event.payload.note_id))
      .then((off) => {
        if (active) offOpen = off;
        else off();
      });
    void events.forkScratchpadChanged
      .listen(() => void loadNotes(query))
      .then((off) => {
        if (active) offChanged = off;
        else off();
      });
    return () => {
      active = false;
      offOpen?.();
      offChanged?.();
    };
  }, [openNote, loadNotes, query]);

  const save = useCallback(async (value: string) => {
    const id = selectedRef.current;
    if (!id || value === savedBodyRef.current) return true;
    const sequence = ++saveSequence.current;
    setSaveState("saving");
    const result = await commands.forkScratchpadSave(id, value);
    if (result.status === "error") {
      if (sequence === saveSequence.current) setSaveState("error");
      return false;
    }
    setSavedBody(value);
    savedBodyRef.current = value;
    setNotes((current) =>
      current.map((note) => (note.id === id ? result.data : note)),
    );
    if (sequence === saveSequence.current) setSaveState("saved");
    return true;
  }, []);

  useEffect(() => {
    if (!selectedId || body === savedBody) return;
    setSaveState("saving");
    const timer = window.setTimeout(() => void save(body), AUTOSAVE_MS);
    return () => window.clearTimeout(timer);
  }, [body, savedBody, selectedId, save]);

  useEffect(() => {
    const flush = () => {
      if (bodyRef.current !== savedBodyRef.current) void save(bodyRef.current);
    };
    window.addEventListener("beforeunload", flush);
    return () => {
      window.removeEventListener("beforeunload", flush);
      flush();
    };
  }, [save]);

  const newNote = async () => {
    if (bodyRef.current !== savedBodyRef.current) await save(bodyRef.current);
    const result = await commands.forkScratchpadNew();
    if (result.status === "ok") await openNote(result.data.id);
    else setError(true);
  };
  const close = async () => {
    if (bodyRef.current !== savedBodyRef.current) await save(bodyRef.current);
    await commands.forkScratchpadClose();
  };
  const polish = async () => {
    if (!body.trim() || polishing) return;
    if (body !== savedBody) await save(body);
    setPolishing(true);
    setError(false);
    const result = await commands.forkScratchpadPolish(body);
    if (result.status === "ok") {
      setBody(result.data);
      await save(result.data);
    } else setError(true);
    setPolishing(false);
  };
  const copy = async () => {
    const result = await commands.forkScratchpadCopy(body);
    if (result.status === "ok") {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } else setError(true);
  };
  const cycleTextSize = () => {
    const index = TEXT_SIZES.indexOf(textSize);
    setTextSize(TEXT_SIZES[(index + 1) % TEXT_SIZES.length]);
  };

  const railButton = (
    label: string,
    testId: string,
    onClick: () => void,
    icon: React.ReactNode,
    pressed?: boolean,
  ) => (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      className={`grid size-9 cursor-pointer place-items-center rounded-lg text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
      data-testid={testId}
    >
      {icon}
    </button>
  );

  return (
    <div
      dir={getLanguageDirection(i18n.language)}
      className="flex h-screen bg-(--wb-canvas) text-(--wb-ink)"
      data-testid="wb-note-window"
    >
      <nav
        aria-label={t("scratchpad.tools")}
        className="flex w-12 shrink-0 flex-col items-center gap-1 border-e border-(--wb-line) bg-(--wb-card) py-3"
      >
        {railButton(
          t("scratchpad.sidebar"),
          "wb-note-sidebar",
          () => setSidebar((value) => !value),
          <PanelLeft size={18} aria-hidden="true" />,
          sidebar,
        )}
        {railButton(
          t("scratchpad.newNote"),
          "wb-note-new-rail",
          () => void newNote(),
          <Plus size={18} aria-hidden="true" />,
        )}
        {railButton(
          t("scratchpad.searchLabel"),
          "wb-note-search-toggle",
          () => {
            setSearchOpen((value) => !value);
            window.setTimeout(() => searchRef.current?.focus(), 0);
          },
          <Search size={18} aria-hidden="true" />,
          searchOpen,
        )}
        <div className="my-2 h-px w-6 bg-(--wb-line)" />
        {railButton(
          t("scratchpad.polish"),
          "wb-note-polish",
          () => void polish(),
          <WandSparkles size={18} aria-hidden="true" />,
        )}
        {railButton(
          t("scratchpad.textSize", { size: textSize }),
          "wb-note-text-size",
          cycleTextSize,
          <Type size={18} aria-hidden="true" />,
        )}
      </nav>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center gap-1 border-b border-(--wb-line) px-2 py-1.5">
          <NoteTabs
            notes={notes}
            selected={selectedId ?? -1}
            onSelect={(id) => void openNote(id)}
          />
          <button
            type="button"
            onClick={() => void newNote()}
            aria-label={t("scratchpad.newNote")}
            title={t("scratchpad.newNote")}
            className={`grid size-8 shrink-0 cursor-pointer place-items-center rounded-md text-(--wb-ink-2) hover:bg-(--wb-hover) ${FOCUS_RING}`}
            data-testid="wb-note-new-tab"
          >
            <Plus size={16} aria-hidden="true" />
          </button>
          <button
            type="button"
            onClick={() =>
              void commands.forkScratchpadExpand().then((result) => {
                if (result.status === "ok") setExpanded(result.data);
              })
            }
            aria-label={t(expanded ? "scratchpad.shrink" : "scratchpad.expand")}
            title={t(expanded ? "scratchpad.shrink" : "scratchpad.expand")}
            className={`grid size-8 shrink-0 cursor-pointer place-items-center rounded-md text-(--wb-ink-2) hover:bg-(--wb-hover) ${FOCUS_RING}`}
            data-testid="wb-note-expand"
          >
            {expanded ? (
              <Minimize size={16} aria-hidden="true" />
            ) : (
              <Expand size={16} aria-hidden="true" />
            )}
          </button>
          <button
            type="button"
            onClick={() => void close()}
            aria-label={t("scratchpad.close")}
            title={t("scratchpad.close")}
            className={`grid size-8 shrink-0 cursor-pointer place-items-center rounded-md text-(--wb-ink-2) hover:bg-(--wb-hover) ${FOCUS_RING}`}
            data-testid="wb-note-close"
          >
            <X size={16} aria-hidden="true" />
          </button>
        </header>
        <div className="flex min-h-0 flex-1">
          {sidebar && (
            <aside
              aria-label={t("scratchpad.recents")}
              className="w-48 shrink-0 overflow-y-auto border-e border-(--wb-line) bg-(--wb-card) p-2"
              data-testid="wb-note-sidebar-panel"
            >
              {notes
                .filter(
                  (note) =>
                    !query ||
                    note.body
                      .toLocaleLowerCase()
                      .includes(query.toLocaleLowerCase()),
                )
                .map((note) => (
                  <button
                    key={note.id}
                    type="button"
                    onClick={() => void openNote(note.id)}
                    aria-current={note.id === selectedId ? "true" : undefined}
                    className={`block w-full cursor-pointer truncate rounded-md px-2 py-2 text-start text-xs hover:bg-(--wb-hover) ${note.id === selectedId ? "bg-(--wb-hover) font-semibold" : ""} ${FOCUS_RING}`}
                  >
                    {note.title || t("scratchpad.untitled")}
                  </button>
                ))}
            </aside>
          )}
          <main
            className="flex min-w-0 flex-1 flex-col"
            id="wb-note-editor-panel"
            role="tabpanel"
          >
            {searchOpen && (
              <div className="border-b border-(--wb-line) p-2">
                <label className="sr-only" htmlFor="wb-note-search">
                  {t("scratchpad.searchLabel")}
                </label>
                <input
                  ref={searchRef}
                  id="wb-note-search"
                  type="search"
                  value={query}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setQuery(value);
                    void loadNotes(value);
                  }}
                  placeholder={t("scratchpad.searchPlaceholder")}
                  className={`w-full rounded-md border border-(--wb-line-strong) bg-(--wb-card) px-3 py-2 text-sm ${FOCUS_RING}`}
                  data-testid="wb-note-search"
                />
              </div>
            )}
            <div className="flex items-center justify-between px-5 pt-3 text-xs text-(--wb-ink)">
              <span role="status" data-testid="wb-note-save-state">
                {saveState === "saved"
                  ? t("scratchpad.saved")
                  : saveState === "saving"
                    ? t("scratchpad.saving")
                    : t("scratchpad.saveError")}
              </span>
              {polishing && (
                <span role="status">{t("scratchpad.polishing")}</span>
              )}
            </div>
            {loading || !selected ? (
              <div className="grid flex-1 place-items-center p-8 text-center text-sm text-(--wb-ink-2)">
                {loading ? t("scratchpad.loading") : t("scratchpad.empty")}
              </div>
            ) : (
              <textarea
                aria-label={t("scratchpad.noteBody")}
                value={body}
                onChange={(event) => setBody(event.currentTarget.value)}
                placeholder={t("scratchpad.notePlaceholder")}
                autoFocus
                spellCheck
                style={{ fontSize: textSize }}
                className={`min-h-0 w-full flex-1 resize-none bg-transparent px-5 py-3 leading-relaxed text-(--wb-ink) outline-none placeholder:text-(--wb-ink-2) ${FOCUS_RING}`}
                data-testid="wb-note-body"
              />
            )}
            {error && (
              <p
                role="alert"
                className="px-5 pb-2 text-xs text-red-700 dark:text-red-300"
              >
                {t("scratchpad.error")}
              </p>
            )}
            <footer className="flex items-center justify-between border-t border-(--wb-line) px-4 py-2">
              <span className="text-xs text-(--wb-ink)">
                {t("scratchpad.autosaveHelp")}
              </span>
              <button
                type="button"
                onClick={() => void copy()}
                disabled={!selected}
                className={`inline-flex cursor-pointer items-center gap-2 rounded-md px-3 py-1.5 text-xs font-medium text-(--wb-ink-2) hover:bg-(--wb-hover) disabled:opacity-50 ${FOCUS_RING}`}
                data-testid="wb-note-copy"
              >
                {copied ? (
                  <Check size={16} aria-hidden="true" />
                ) : (
                  <Copy size={16} aria-hidden="true" />
                )}
                {copied ? t("scratchpad.copied") : t("scratchpad.copy")}
              </button>
            </footer>
          </main>
        </div>
      </div>
    </div>
  );
}
