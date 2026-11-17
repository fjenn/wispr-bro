// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A Scratchpad note on iPhone: the Mac's note (`../scratchpad/NoteWindow`,
 * the same commands, 500 ms autosave, Polish, text size, Copy, Record) as
 * a full-screen editor that covers the tab bar.
 *
 *   ‹ Scratchpad                 Aa  ⋯  Done
 *   ┌──────────────────────────────────────┐
 *   │ the note (its first line is its      │
 *   │ title); scrolls by itself            │
 *   ├──────────────────────────────────────┤
 *   │ 128 words          ✦  Record   Copy  │  above the keyboard
 *   └──────────────────────────────────────┘
 *
 * Dictating is the Wispr Bro keyboard, as in any field, or Record: a take
 * the app starts itself (`ios_take_*`), whose text lands at the caret.
 * A note left empty is removed when the editor closes.
 */
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { copyToClipboard } from "@/components/settings/history/clipboard";
import {
  commands,
  events,
  type ForkLlmStatus,
  type LlmState,
  type Note,
} from "../bindings";
import { navigate } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { Button, Dialog, IconButton, Menu } from "../ui";
import {
  ALargeSmall,
  Check,
  Copy,
  Loader2,
  Mic,
  MoreHorizontal,
  Pin,
  PinOff,
  Share,
  Sparkles,
  Square,
  Trash2,
  X,
} from "../ui/icons";
import { iosCommands, type IosTakePhase } from "./commands";
import { countWords, insertAtCaret, type Caret } from "./noteText";

const AUTOSAVE_MS = 500;
/** Same list as `SCRATCHPAD_TEXT_SIZES` in `src-tauri/src/fork/settings.rs`. */
const TEXT_SIZES = [16, 20, 24] as const;
/** On the phone the three sizes read one point larger than on the Mac. */
const PHONE_SIZE: Record<number, number> = { 16: 17, 20: 20, 24: 24 };
const TAKE_POLL_MS = 90;
const LEVEL_BARS = 5;
/** The id of the slot in the top bar (`IosApp`) this page puts its actions in. */
export const BAR_END_ID = "wb-ios-bar-end";

type SaveState = "saved" | "saving" | "error";
type PolishPhase = "idle" | "running";
type Notice =
  | { kind: "polished"; previous: string; polished: string }
  | { kind: "text"; text: string; settings?: boolean };

/**
 * Why Polish cannot run now, as a string key: the cleanup model is known
 * not to be here yet. Null when it can be tried.
 */
function polishBlock(
  state: LlmState | null,
  cleanupOff: boolean,
): { key: string; settings: boolean } | null {
  switch (state) {
    case "downloading":
    case "verifying":
      return { key: "ios.scratchpad.polish.downloading", settings: false };
    case "download_failed":
      return { key: "ios.scratchpad.polish.failed", settings: true };
    case "waiting_for_speech_model":
      return cleanupOff
        ? { key: "ios.scratchpad.polish.off", settings: true }
        : { key: "ios.scratchpad.polish.downloading", settings: false };
    default:
      // Ready, or no local model for this iPhone: the app answers (another
      // engine may do it), and says so when nothing can.
      return null;
  }
}

export function IosNote({ id }: { id: number }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  const [note, setNote] = useState<Note | null>(null);
  const [body, setBody] = useState("");
  const [savedBody, setSavedBody] = useState("");
  const [saveState, setSaveState] = useState<SaveState>("saved");
  const [loading, setLoading] = useState(true);
  const [focused, setFocused] = useState(false);
  const [copied, setCopied] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [polishPhase, setPolishPhase] = useState<PolishPhase>("idle");
  const [llm, setLlm] = useState<LlmState | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [take, setTake] = useState<{ id: string; phase: IosTakePhase } | null>(
    null,
  );
  const [levels, setLevels] = useState<number[]>(() =>
    Array(LEVEL_BARS).fill(0),
  );
  const [slot, setSlot] = useState<HTMLElement | null>(null);

  const editorRef = useRef<HTMLTextAreaElement>(null);
  const bodyRef = useRef(body);
  const savedRef = useRef(savedBody);
  const caretRef = useRef<Caret | null>(null);
  const pendingCaret = useRef<number | null>(null);
  const saveSequence = useRef(0);
  const deleted = useRef(false);
  const loaded = useRef(false);
  const takeRef = useRef<string | null>(null);
  bodyRef.current = body;
  savedRef.current = savedBody;
  takeRef.current = take?.id ?? null;

  const storedSize = settings?.scratchpad_text_size;
  const textSize =
    TEXT_SIZES.find((size) => size === storedSize) ?? TEXT_SIZES[0];

  useLayoutEffect(() => {
    setSlot(document.getElementById(BAR_END_ID));
  }, []);

  // The note.
  useEffect(() => {
    let active = true;
    void commands
      .forkScratchpadGet(id)
      .then((result) => {
        if (!active) return;
        if (result.status !== "ok" || !result.data) {
          navigate("scratchpad");
          return;
        }
        setNote(result.data);
        setBody(result.data.body);
        setSavedBody(result.data.body);
        savedRef.current = result.data.body;
        loaded.current = true;
        setLoading(false);
        // A new note is for writing: the keyboard comes up with it.
        if (!result.data.body) {
          requestAnimationFrame(() => editorRef.current?.focus());
        }
      })
      .catch(() => active && navigate("scratchpad"));
    return () => {
      active = false;
    };
  }, [id]);

  const save = useCallback(
    async (value: string) => {
      if (deleted.current || value === savedRef.current) return true;
      const sequence = ++saveSequence.current;
      setSaveState("saving");
      const result = await commands
        .forkScratchpadSave(id, value)
        .catch(() => null);
      if (!result || result.status === "error") {
        if (sequence === saveSequence.current) setSaveState("error");
        return false;
      }
      savedRef.current = value;
      setSavedBody(value);
      setNote(result.data);
      if (sequence === saveSequence.current) setSaveState("saved");
      return true;
    },
    [id],
  );

  useEffect(() => {
    if (loading || body === savedBody) return;
    setSaveState("saving");
    const timer = window.setTimeout(() => void save(body), AUTOSAVE_MS);
    return () => window.clearTimeout(timer);
  }, [body, savedBody, loading, save]);

  // Nothing waits for the timer when the app goes to the background or
  // the editor closes; a note left empty is removed.
  useEffect(() => {
    const flush = () => {
      if (bodyRef.current !== savedRef.current) void save(bodyRef.current);
    };
    const onHidden = () => {
      if (document.visibilityState === "hidden") flush();
    };
    document.addEventListener("visibilitychange", onHidden);
    window.addEventListener("pagehide", flush);
    return () => {
      document.removeEventListener("visibilitychange", onHidden);
      window.removeEventListener("pagehide", flush);
      if (takeRef.current) void iosCommands.takeCancel(takeRef.current);
      if (deleted.current || !loaded.current) return;
      if (!bodyRef.current.trim()) {
        deleted.current = true;
        void commands.forkScratchpadDelete(id).catch(() => undefined);
      } else flush();
    };
  }, [id, save]);

  // Polish needs the cleanup model.
  useEffect(() => {
    let active = true;
    let off: (() => void) | undefined;
    const apply = (status: ForkLlmStatus) => active && setLlm(status.state);
    void commands
      .forkLlmStatus()
      .then((result) => result.status === "ok" && apply(result.data))
      .catch(() => undefined);
    void events.forkLlmStatus
      .listen(({ payload }) => apply(payload))
      .then((unlisten) => {
        if (active) off = unlisten;
        else unlisten();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      off?.();
    };
  }, []);

  // A caret set by an inserted take, once the text is in the field.
  useLayoutEffect(() => {
    const at = pendingCaret.current;
    const field = editorRef.current;
    if (at === null || !field) return;
    pendingCaret.current = null;
    field.setSelectionRange(at, at);
    caretRef.current = { start: at, end: at };
  }, [body]);

  // Undo is offered until the note is edited again.
  useEffect(() => {
    if (notice?.kind === "polished" && body !== notice.polished)
      setNotice(null);
  }, [body, notice]);

  // The caret is known only while the note has the keyboard: a field
  // that was never touched reports its start, where nobody put it.
  const rememberCaret = () => {
    const field = editorRef.current;
    if (!field || document.activeElement !== field) return;
    caretRef.current = { start: field.selectionStart, end: field.selectionEnd };
  };
  // WebKit gives a field whose text was set from here its caret at the very
  // start when it gets the keyboard. Put it back where it was left: after
  // the last take, else at the end.
  const restoreCaret = () => {
    const apply = () => {
      const field = editorRef.current;
      if (!field || document.activeElement !== field) return;
      if (field.selectionStart !== 0 || field.selectionEnd !== 0) return;
      const at = Math.min(wanted, field.value.length);
      if (at > 0) field.setSelectionRange(at, at);
      caretRef.current = { start: at, end: at };
    };
    const field = editorRef.current;
    const wanted = caretRef.current?.end ?? field?.value.length ?? 0;
    // WebKit places its caret a moment after the field has the keyboard.
    for (const delay of [0, 120, 320]) window.setTimeout(apply, delay);
  };

  // Record: the take's phase and level, asked while it runs.
  const takeId = take?.id;
  useEffect(() => {
    if (!takeId) {
      setLevels(Array(LEVEL_BARS).fill(0));
      return;
    }
    let active = true;
    let busy = false;
    let unknown = 0;
    const timer = window.setInterval(() => {
      if (busy) return;
      busy = true;
      void iosCommands
        .takeState(takeId)
        .then((state) => {
          if (!active) return;
          if (state.phase === "done") {
            const next = insertAtCaret(
              bodyRef.current,
              state.text ?? "",
              caretRef.current,
            );
            pendingCaret.current = next.caret;
            setBody(next.body);
            setTake(null);
          } else if (state.phase === "failed") {
            setNotice({
              kind: "text",
              text: state.error || t("ios.scratchpad.recordFailed"),
            });
            setTake(null);
          } else if (state.phase === "cancelled") {
            setTake(null);
          } else if (state.phase === "none") {
            // Not this take any more (another one replaced it).
            unknown += 1;
            if (unknown > 10) setTake(null);
          } else {
            setTake((current) =>
              current && current.id === takeId && current.phase !== state.phase
                ? { id: takeId, phase: state.phase }
                : current,
            );
            if (state.phase === "listening") {
              const level = Math.min(1, Math.max(0, state.level || 0));
              setLevels((bars) => [...bars.slice(1), level]);
            }
          }
        })
        .catch(() => {
          if (!active) return;
          setNotice({ kind: "text", text: t("ios.scratchpad.recordFailed") });
          setTake(null);
        })
        .finally(() => {
          busy = false;
        });
    }, TAKE_POLL_MS);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [takeId, t]);

  const record = async () => {
    if (take) {
      if (take.phase === "listening" || take.phase === "starting") {
        await iosCommands.takeStop(take.id).catch(() => undefined);
      }
      return;
    }
    rememberCaret();
    setNotice(null);
    try {
      const started = await iosCommands.takeStart();
      setTake({ id: started, phase: "starting" });
    } catch {
      setNotice({ kind: "text", text: t("ios.scratchpad.recordFailed") });
    }
  };

  const block = polishBlock(llm, settings?.cleanup_level === "none");
  const polishing = polishPhase !== "idle";
  const polish = async () => {
    if (polishing) return;
    if (block) {
      setNotice({ kind: "text", text: t(block.key), settings: block.settings });
      return;
    }
    const source = body;
    if (!source.trim()) return;
    if (source !== savedRef.current) await save(source);
    setNotice(null);
    setPolishPhase("running");
    const result = await commands
      .forkScratchpadPolish(source)
      .catch(() => null);
    setPolishPhase("idle");
    if (!result || result.status === "error") {
      setNotice({ kind: "text", text: t("scratchpad.polishRefused") });
      return;
    }
    const { outcome, text } = result.data;
    if (outcome === "changed" && text != null) {
      if (bodyRef.current !== source) {
        setNotice({ kind: "text", text: t("scratchpad.polishStale") });
        return;
      }
      setBody(text);
      await save(text);
      setNotice({ kind: "polished", previous: source, polished: text });
    } else {
      setNotice({
        kind: "text",
        text: t(
          outcome === "no_change"
            ? "scratchpad.polishNoChange"
            : outcome === "not_ready"
              ? llm === "unsupported" || llm === "missing_runtime"
                ? "ios.scratchpad.polish.unsupported"
                : "ios.scratchpad.polish.downloading"
              : "scratchpad.polishRefused",
        ),
      });
    }
  };
  const undoPolish = async () => {
    if (notice?.kind !== "polished") return;
    const previous = notice.previous;
    setNotice(null);
    setBody(previous);
    await save(previous);
  };

  const copy = async () => {
    if (await copyToClipboard(body)) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } else setNotice({ kind: "text", text: t("ios.scratchpad.error") });
  };
  const canShare =
    typeof navigator !== "undefined" && typeof navigator.share === "function";
  const share = () => {
    // The share sheet; closing it without sharing is not an error.
    void navigator.share({ text: body }).catch(() => undefined);
  };
  const cycleTextSize = () => {
    const index = TEXT_SIZES.indexOf(textSize);
    void setSetting(
      "scratchpad_text_size",
      TEXT_SIZES[(index + 1) % TEXT_SIZES.length],
    );
  };
  const pin = async () => {
    if (!note) return;
    const result = await commands
      .forkScratchpadSetPinned(id, !note.pinned)
      .catch(() => null);
    if (result?.status === "ok") setNote(result.data);
    else setNotice({ kind: "text", text: t("ios.scratchpad.error") });
  };
  const remove = async () => {
    setConfirmDelete(false);
    const result = await commands.forkScratchpadDelete(id).catch(() => null);
    if (result?.status === "ok") {
      deleted.current = true;
      navigate("scratchpad");
    } else setNotice({ kind: "text", text: t("ios.scratchpad.error") });
  };

  const words = countWords(body);
  const hasText = body.trim() !== "";
  const listening = take?.phase === "listening";
  const writing = take?.phase === "transcribing";
  // A tap on a tool must not take the keyboard away from the note.
  const keepFocus = (event: { preventDefault: () => void }) =>
    event.preventDefault();

  const barEnd = (
    <>
      <IconButton
        onMouseDown={keepFocus}
        onClick={cycleTextSize}
        aria-label={t("scratchpad.textSize", { size: textSize })}
        data-testid="wb-note-text-size"
      >
        <ALargeSmall size={20} />
      </IconButton>
      <Menu
        label={t("ios.scratchpad.more")}
        align="end"
        testId="wb-note-menu"
        trigger={
          <IconButton
            aria-label={t("ios.scratchpad.more")}
            data-testid="wb-note-more"
          >
            <MoreHorizontal size={20} />
          </IconButton>
        }
        groups={[
          {
            items: [
              {
                id: "pin",
                label: t(note?.pinned ? "scratchpad.unpin" : "scratchpad.pin"),
                icon: note?.pinned ? PinOff : Pin,
                onSelect: () => void pin(),
              },
              ...(canShare
                ? [
                    {
                      id: "share",
                      label: t("ios.scratchpad.share"),
                      icon: Share,
                      disabled: !hasText,
                      onSelect: share,
                    },
                  ]
                : []),
              {
                id: "delete",
                label: t("scratchpad.delete"),
                icon: Trash2,
                onSelect: () => setConfirmDelete(true),
              },
            ],
          },
        ]}
      />
      {focused && (
        <button
          type="button"
          className="wb-ios-bar-done"
          onClick={() => editorRef.current?.blur()}
          data-testid="wb-note-done"
        >
          {t("ios.scratchpad.done")}
        </button>
      )}
    </>
  );

  return (
    <div
      className="wb-ios-note"
      data-testid="wb-ios-note"
      data-note={id}
      data-save={saveState}
    >
      {slot && createPortal(barEnd, slot)}
      <h1 className="sr-only" data-testid="wb-page-title">
        {note?.title || t("scratchpad.untitled")}
      </h1>
      {loading ? (
        <p
          role="status"
          className="flex-1 px-(--wb-ios-gutter) pt-3 text-[16px] text-(--wb-ink-3)"
        >
          {t("scratchpad.loading")}
        </p>
      ) : (
        <textarea
          ref={editorRef}
          dir="auto"
          aria-label={t("scratchpad.noteBody")}
          value={body}
          onChange={(event) => {
            setBody(event.currentTarget.value);
            rememberCaret();
          }}
          onSelect={rememberCaret}
          onFocus={() => {
            setFocused(true);
            restoreCaret();
          }}
          onBlur={() => {
            rememberCaret();
            setFocused(false);
          }}
          placeholder={t("scratchpad.notePlaceholder")}
          spellCheck
          autoCapitalize="sentences"
          style={{ "--wb-note-size": `${PHONE_SIZE[textSize]}px` } as never}
          className="wb-ios-note-body"
          data-wb-selectable
          data-testid="wb-note-body"
        />
      )}
      {notice && (
        <div
          role="status"
          className="wb-ios-note-notice"
          data-testid="wb-note-notice"
        >
          <span className="min-w-0 flex-1">
            {notice.kind === "polished"
              ? t("scratchpad.polished")
              : notice.text}
          </span>
          {notice.kind === "polished" ? (
            <Button
              size="sm"
              variant="outline"
              onMouseDown={keepFocus}
              onClick={() => void undoPolish()}
              data-testid="wb-note-polish-undo"
            >
              {t("scratchpad.undo")}
            </Button>
          ) : (
            <>
              {notice.settings && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => navigate("settings", "cleanup")}
                  data-testid="wb-note-notice-settings"
                >
                  {t("ios.tabs.settings")}
                </Button>
              )}
              <IconButton
                size="sm"
                onMouseDown={keepFocus}
                onClick={() => setNotice(null)}
                aria-label={t("scratchpad.dismiss")}
                data-testid="wb-note-notice-dismiss"
              >
                <X size={16} />
              </IconButton>
            </>
          )}
        </div>
      )}
      <div className="wb-ios-note-tools" data-testid="wb-note-tools">
        <span
          role="status"
          className="min-w-0 flex-1 truncate text-[13px] leading-[18px] text-(--wb-ink-3) tabular-nums"
          data-testid="wb-note-status"
        >
          {polishing
            ? t("scratchpad.polishing")
            : writing
              ? t("ios.scratchpad.writing")
              : saveState === "error"
                ? t("scratchpad.saveError")
                : t("ios.scratchpad.words", { count: words })}
        </span>
        <IconButton
          onMouseDown={keepFocus}
          onClick={() => void polish()}
          disabled={loading || !hasText || polishing}
          data-blocked={block ? true : undefined}
          aria-label={t("scratchpad.polish")}
          className={block ? "opacity-45" : ""}
          data-testid="wb-note-polish"
        >
          {polishing ? (
            <Loader2 size={20} className="animate-spin" />
          ) : (
            <Sparkles size={20} />
          )}
        </IconButton>
        <Button
          variant="outline"
          onMouseDown={keepFocus}
          onClick={() => void record()}
          disabled={loading || writing}
          aria-pressed={listening}
          aria-label={
            listening
              ? t("scratchpad.stopHint")
              : t("scratchpad.recordHintNoKey")
          }
          leading={
            listening ? (
              <Square size={14} />
            ) : writing || take?.phase === "starting" ? (
              <Loader2 size={16} className="animate-spin" />
            ) : (
              <Mic size={18} />
            )
          }
          className="wb-ios-pill wb-ios-note-record"
          data-state={take?.phase ?? "idle"}
          data-testid="wb-note-record"
        >
          {listening ? t("scratchpad.stop") : t("scratchpad.record")}
          {listening && (
            <span
              aria-hidden="true"
              className="ms-0.5 flex h-4 items-center gap-[2px]"
              data-testid="wb-note-record-level"
            >
              {levels.map((value, i) => (
                <span
                  key={i}
                  className="w-[3px] rounded-full bg-current transition-[height] duration-(--wb-dur-fast)"
                  style={{ height: `${Math.round(3 + value * 13)}px` }}
                />
              ))}
            </span>
          )}
        </Button>
        <Button
          variant="primary"
          onMouseDown={keepFocus}
          onClick={() => void copy()}
          disabled={loading || !hasText}
          leading={copied ? <Check size={18} /> : <Copy size={18} />}
          className="wb-ios-pill"
          data-testid="wb-note-copy"
        >
          {copied ? t("scratchpad.copied") : t("scratchpad.copy")}
        </Button>
      </div>
      <Dialog
        open={confirmDelete}
        onOpenChange={setConfirmDelete}
        title={t("ios.scratchpad.deleteTitle")}
        closeLabel={t("ios.close")}
        showCloseButton={false}
        testId="wb-note-delete-sheet"
        footer={
          <>
            <Button
              variant="secondary"
              onClick={() => setConfirmDelete(false)}
              data-testid="wb-note-delete-cancel"
            >
              {t("ios.scratchpad.cancel")}
            </Button>
            <Button
              variant="destructive"
              onClick={() => void remove()}
              data-testid="wb-note-delete-confirm"
            >
              {t("scratchpad.delete")}
            </Button>
          </>
        }
      >
        <p className="text-[16px] leading-[22px] text-(--wb-ink-2)">
          {t("ios.scratchpad.deleteBody", {
            name: note?.title || t("scratchpad.untitled"),
          })}
        </p>
      </Dialog>
    </div>
  );
}
