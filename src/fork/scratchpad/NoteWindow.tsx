// SPDX-License-Identifier: GPL-3.0-or-later
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "motion/react";
import {
  ALargeSmall,
  Check,
  Copy,
  Loader2,
  Maximize2,
  Minimize2,
  PanelLeft,
  Plus,
  Search,
  Sparkles,
  SquarePen,
  X,
} from "../ui/icons";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { commands, events, type Note } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import {
  Button,
  FOCUS_RING,
  IconButton,
  SearchField,
  useMotionPresets,
} from "../ui";
import { useForkSettings } from "../settings/useForkSettings";
import { NoteTabs } from "./NoteTabs";

const AUTOSAVE_MS = 500;
/** Same list as `SCRATCHPAD_TEXT_SIZES` in `src-tauri/src/fork/settings.rs`. */
const TEXT_SIZES = [16, 20, 24] as const;
type TextSize = (typeof TEXT_SIZES)[number];

/** The stored size, or the default for a value the list does not have. */
function storedTextSize(value: number | undefined): TextSize {
  return TEXT_SIZES.find((size) => size === value) ?? TEXT_SIZES[0];
}

type SaveState = "saved" | "saving" | "error";

function requestedNote(): number | null {
  const value = Number(new URLSearchParams(window.location.search).get("note"));
  return Number.isSafeInteger(value) && value > 0 ? value : null;
}

export function NoteWindow() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const m = useMotionPresets();
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
  // Audit OT2: a fork setting, so the size survives a parked window and a
  // restart (it was window state).
  const { settings: forkSettings, setSetting } = useForkSettings();
  const textSize = storedTextSize(forkSettings?.scratchpad_text_size);
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
    void setSetting(
      "scratchpad_text_size",
      TEXT_SIZES[(index + 1) % TEXT_SIZES.length],
    ).then((result) => {
      if (result.status === "error") setError(true);
    });
  };

  const toolButton = (
    label: string,
    testId: string,
    onClick: () => void,
    icon: ReactNode,
    pressed?: boolean,
  ) => (
    <IconButton
      onClick={onClick}
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      className={pressed ? "bg-(--wb-fill-active)! text-(--wb-ink)!" : ""}
      data-testid={testId}
    >
      {icon}
    </IconButton>
  );

  const visibleNotes = notes.filter(
    (note) =>
      !query ||
      note.body.toLocaleLowerCase().includes(query.toLocaleLowerCase()),
  );

  return (
    <div
      dir={getLanguageDirection(i18n.language)}
      className="flex h-screen flex-col bg-(--wb-chrome) text-(--wb-ink) select-none"
      data-testid="wb-note-window"
    >
      {/* The title strip: macOS draws the traffic lights over its left
          end in every language (overlay title bar,
          `fork/scratchpad/window.rs`), so the room for them is physical
          left padding, not `ps-` (Mac wave 6: in Arabic `ps-` moved it to
          the right and the expand/close buttons slid under the lights).
          Empty space drags the window. */}
      <header
        data-tauri-drag-region
        className="flex h-11 shrink-0 items-center gap-1 pr-2 pl-[90px]"
      >
        <NoteTabs
          notes={notes}
          selected={selectedId ?? -1}
          onSelect={(id) => void openNote(id)}
        />
        <IconButton
          size="sm"
          onClick={() => void newNote()}
          aria-label={t("scratchpad.newNote")}
          title={t("scratchpad.newNote")}
          data-testid="wb-note-new-tab"
        >
          <Plus size={16} />
        </IconButton>
        <div data-tauri-drag-region className="h-full min-w-4 flex-1" />
        <IconButton
          size="sm"
          onClick={() =>
            void commands.forkScratchpadExpand().then((result) => {
              if (result.status === "ok") setExpanded(result.data);
            })
          }
          aria-label={t(expanded ? "scratchpad.shrink" : "scratchpad.expand")}
          title={t(expanded ? "scratchpad.shrink" : "scratchpad.expand")}
          data-testid="wb-note-expand"
        >
          {expanded ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
        </IconButton>
        <IconButton
          size="sm"
          onClick={() => void close()}
          aria-label={t("scratchpad.close")}
          title={t("scratchpad.close")}
          data-testid="wb-note-close"
        >
          <X size={16} />
        </IconButton>
      </header>
      <div className="flex min-h-0 flex-1">
        <nav
          aria-label={t("scratchpad.tools")}
          className="flex w-[52px] shrink-0 flex-col items-center gap-1 pt-1 pb-3"
        >
          {toolButton(
            t("scratchpad.sidebar"),
            "wb-note-sidebar",
            () => setSidebar((value) => !value),
            <PanelLeft size={18} />,
            sidebar,
          )}
          {toolButton(
            t("scratchpad.newNote"),
            "wb-note-new-rail",
            () => void newNote(),
            <SquarePen size={18} />,
          )}
          {toolButton(
            t("scratchpad.searchLabel"),
            "wb-note-search-toggle",
            () => {
              setSearchOpen((value) => !value);
              window.setTimeout(() => searchRef.current?.focus(), 0);
            },
            <Search size={18} />,
            searchOpen,
          )}
          <div className="mt-auto flex flex-col items-center gap-1">
            {toolButton(
              t("scratchpad.polish"),
              "wb-note-polish",
              () => void polish(),
              polishing ? (
                <Loader2 size={18} className="animate-spin" />
              ) : (
                <Sparkles size={18} />
              ),
            )}
            {toolButton(
              t("scratchpad.textSize", { size: textSize }),
              "wb-note-text-size",
              cycleTextSize,
              <ALargeSmall size={18} />,
            )}
          </div>
        </nav>
        <AnimatePresence initial={false}>
          {sidebar && (
            <motion.aside
              {...m.reveal}
              aria-label={t("scratchpad.recents")}
              className="flex w-44 shrink-0 flex-col gap-0.5 overflow-y-auto pe-2 pb-2"
              data-testid="wb-note-sidebar-panel"
            >
              {visibleNotes.map((note) => (
                <button
                  key={note.id}
                  type="button"
                  onClick={() => void openNote(note.id)}
                  aria-current={note.id === selectedId ? "true" : undefined}
                  dir={note.title ? "auto" : undefined}
                  className={`block h-8 w-full shrink-0 cursor-pointer truncate rounded-(--wb-radius-sm) px-2.5 text-start text-[13px] transition-colors duration-(--wb-dur-fast) ${note.id === selectedId ? "bg-(--wb-fill-active) font-medium text-(--wb-ink)" : "text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink)"} ${FOCUS_RING} focus-visible:-outline-offset-2`}
                >
                  {note.title || t("scratchpad.untitled")}
                </button>
              ))}
            </motion.aside>
          )}
        </AnimatePresence>
        <main
          className="relative me-2 mb-2 flex min-w-0 flex-1 flex-col overflow-hidden rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-panel) select-text"
          id="wb-note-editor-panel"
          role="tabpanel"
        >
          {searchOpen && (
            <div className="shrink-0 px-4 pt-3">
              <label className="sr-only" htmlFor="wb-note-search">
                {t("scratchpad.searchLabel")}
              </label>
              <SearchField
                ref={searchRef}
                id="wb-note-search"
                size="sm"
                value={query}
                onChange={(event) => {
                  const value = event.currentTarget.value;
                  setQuery(value);
                  void loadNotes(value);
                }}
                placeholder={t("scratchpad.searchPlaceholder")}
                data-testid="wb-note-search"
              />
            </div>
          )}
          {loading || !selected ? (
            <div className="grid flex-1 place-items-center p-8 text-center text-[15px] text-(--wb-ink-2)">
              {loading ? t("scratchpad.loading") : t("scratchpad.empty")}
            </div>
          ) : (
            <textarea
              dir="auto"
              aria-label={t("scratchpad.noteBody")}
              value={body}
              onChange={(event) => setBody(event.currentTarget.value)}
              placeholder={t("scratchpad.notePlaceholder")}
              autoFocus
              spellCheck
              style={{ fontSize: textSize }}
              className="min-h-0 w-full flex-1 resize-none bg-transparent px-6 pt-5 pb-16 leading-[1.55] text-(--wb-ink) outline-none placeholder:text-(--wb-ink-3) focus-visible:outline-none"
              data-testid="wb-note-body"
            />
          )}
          {error && (
            <p
              role="alert"
              className="mx-4 mb-14 rounded-(--wb-radius-sm) bg-(--wb-danger-soft) px-3 py-2 text-[13px] text-(--wb-ink)"
            >
              {t("scratchpad.error")}
            </p>
          )}
          <div className="pointer-events-none absolute inset-x-0 bottom-0 flex items-center justify-between gap-3 ps-6 pe-3 pb-3">
            <span
              role="status"
              title={t("scratchpad.autosaveHelp")}
              className="pointer-events-auto text-[12px] text-(--wb-ink-3)"
              data-testid="wb-note-save-state"
            >
              {polishing
                ? t("scratchpad.polishing")
                : saveState === "saved"
                  ? t("scratchpad.saved")
                  : saveState === "saving"
                    ? t("scratchpad.saving")
                    : t("scratchpad.saveError")}
            </span>
            <Button
              variant="primary"
              onClick={() => void copy()}
              disabled={!selected}
              leading={copied ? <Check size={16} /> : <Copy size={16} />}
              className="pointer-events-auto rounded-full! px-4 shadow-(--wb-elev-1)"
              data-testid="wb-note-copy"
            >
              {copied ? t("scratchpad.copied") : t("scratchpad.copy")}
            </Button>
          </div>
        </main>
      </div>
    </div>
  );
}
