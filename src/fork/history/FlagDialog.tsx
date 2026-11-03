// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Flag a dictation (S-15b, C-40, DEC-18): "Wrong word?". The row's words
 * are buttons; pick the wrong one (pick a neighbour to widen the pick to a
 * few words), type how it should be, and choose:
 *
 * - **Replace it every time** (default): a dictionary replacement
 *   from → to, so the same mistake is fixed in every later take;
 * - **Teach the spelling**: the corrected word goes in the dictionary, so
 *   the recognizer's word fix can find it.
 *
 * Saved through the S-16 store (`fork_history_flag`, source `flag`); the
 * dictionary's answer (already there, a replacement for that word exists
 * with "Replace it", too long …) is shown in place, like the Add word
 * dialog. The row is never re-pasted or rewritten.
 */
import { Loader2 } from "../ui/icons";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "../ui/Dialog";
import {
  commands,
  type DictionaryEntry,
  type EntryProblem,
  type FlagKind,
  type FlagResult,
  type HistoryRow,
} from "../bindings";
import {
  INPUT_CLASS,
  MAX_PHRASE_CHARS,
  MAX_REPLACEMENT_CHARS,
  SAVE_FAILURE_KEY,
  saveFailure,
  type SaveFailure,
} from "../dictionary/AddWordDialog";
import { useDictionaryStore } from "../dictionary/useDictionary";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";

/** Most words shown to pick from (a long take is cut, the rest says …). */
export const MAX_WORDS = 120;
/** Most words in one pick (a replacement's "from" is short). */
export const MAX_PICK = 4;

type SegmenterCtor = new (
  language: string,
  options: { granularity: "word" },
) => {
  segment: (
    text: string,
  ) => Iterable<{ segment: string; index: number; isWordLike?: boolean }>;
};

export interface Word {
  text: string;
  index: number;
}

/** The words of `text` (Unicode word segments, so CJK and accents work). */
export function wordsOf(text: string, language: string): Word[] {
  const words: Word[] = [];
  try {
    // `Intl.Segmenter` is in every WebKit the app runs on (Safari 14.1+),
    // but not in this project's TypeScript lib.
    const Segmenter = (Intl as unknown as { Segmenter: SegmenterCtor })
      .Segmenter;
    const segmenter = new Segmenter(language, { granularity: "word" });
    for (const s of segmenter.segment(text)) {
      if (s.isWordLike) words.push({ text: s.segment, index: s.index });
    }
  } catch {
    for (const m of text.matchAll(/[\p{L}\p{N}][\p{L}\p{N}'’-]*/gu)) {
      words.push({ text: m[0], index: m.index ?? 0 });
    }
  }
  return words;
}

/** The text of words `from..=to` as written (spaces and inner marks kept). */
export function pickText(
  text: string,
  words: Word[],
  from: number,
  to: number,
) {
  const first = words[from];
  const last = words[to];
  // A pick from a row that just closed (the dialog's text is gone).
  if (!first || !last) return "";
  return text.slice(first.index, last.index + last.text.length);
}

type Feedback =
  | { kind: "problem"; problem: EntryProblem }
  | { kind: "duplicate"; existing: DictionaryEntry }
  | { kind: "conflict"; existing: DictionaryEntry }
  | { kind: "failed"; reason: SaveFailure };

export interface FlagDialogProps {
  row: HistoryRow | null;
  onOpenChange: (open: boolean) => void;
  /** Called with the flagged row and what was saved. */
  onFlagged: (row: HistoryRow | null, entry: DictionaryEntry | null) => void;
}

export function FlagDialog({ row, onOpenChange, onFlagged }: FlagDialogProps) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const open = row !== null;
  const text = row?.text ?? "";
  const words = useMemo(
    () => wordsOf(text, i18n.language).slice(0, MAX_WORDS),
    [text, i18n.language],
  );
  const [pick, setPick] = useState<[number, number] | null>(null);
  const [to, setTo] = useState("");
  const [kind, setKind] = useState<FlagKind>("rule");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const firstWordRef = useRef<HTMLButtonElement>(null);
  const toRef = useRef<HTMLInputElement>(null);
  const toId = useId();
  const feedbackId = useId();
  const wordsLabelId = useId();
  const kindName = useId();

  useEffect(() => {
    if (!open) return;
    setPick(null);
    setTo("");
    setKind("rule");
    setBusy(false);
    setFeedback(null);
  }, [open, row?.id]);

  const from = pick ? pickText(text, words, pick[0], pick[1]) : "";

  const choose = (i: number) => {
    setFeedback(null);
    let next: [number, number] | null;
    if (!pick) next = [i, i];
    else if (pick[0] === i && pick[1] === i) next = null;
    else if (i === pick[0] - 1 && pick[1] - i < MAX_PICK) next = [i, pick[1]];
    else if (i === pick[1] + 1 && i - pick[0] < MAX_PICK) next = [pick[0], i];
    else if (i >= pick[0] && i <= pick[1]) next = [i, i];
    else next = [i, i];
    setPick(next);
    const picked = next ? pickText(text, words, next[0], next[1]) : "";
    setTo(picked);
    if (next) requestAnimationFrame(() => toRef.current?.select());
  };

  const toTrim = to.trim();
  const tooLong =
    [...(kind === "rule" ? toTrim : toTrim)].length >
    (kind === "rule" ? MAX_REPLACEMENT_CHARS : MAX_PHRASE_CHARS);
  const unchanged = kind === "rule" && toTrim === from.trim();
  const valid =
    pick !== null && toTrim.length > 0 && !unchanged && !tooLong && !busy;

  const handle = (result: FlagResult) => {
    const outcome = result.outcome;
    switch (outcome.status) {
      case "saved":
        void useDictionaryStore.getState().reload();
        onFlagged(result.row, outcome.entry);
        onOpenChange(false);
        return;
      case "duplicate":
        setFeedback({ kind: "duplicate", existing: outcome.existing });
        onFlagged(result.row, null);
        return;
      case "conflict":
        setFeedback({ kind: "conflict", existing: outcome.existing });
        return;
      case "invalid":
        setFeedback({ kind: "problem", problem: outcome.problem });
        return;
      case "missing":
        setFeedback({ kind: "failed", reason: "unknown" });
        return;
    }
  };

  const save = async (replace: number | null) => {
    if (!row || !pick) return;
    setBusy(true);
    setFeedback(null);
    try {
      const result = await commands.forkHistoryFlag(
        row.id,
        from.normalize("NFC"),
        toTrim.normalize("NFC"),
        kind,
        replace,
      );
      if (result.status === "ok") handle(result.data);
      else {
        console.warn("fork: flagging the dictation failed", result.error);
        setFeedback({ kind: "failed", reason: saveFailure(result.error) });
      }
    } catch (e) {
      console.warn("fork: flagging the dictation failed", e);
      setFeedback({ kind: "failed", reason: "unknown" });
    } finally {
      setBusy(false);
    }
  };

  const feedbackText = (f: Feedback): string => {
    switch (f.kind) {
      case "problem":
        return t(`dictionary.problem.${f.problem}`, {
          max: MAX_PHRASE_CHARS,
          maxReplacement: MAX_REPLACEMENT_CHARS,
          // The limit the message names picks the plural form.
          count:
            f.problem === "replacement_too_long"
              ? MAX_REPLACEMENT_CHARS
              : MAX_PHRASE_CHARS,
        });
      case "duplicate":
        return t("history.flag.duplicate");
      case "conflict":
        return f.existing.kind === "rule"
          ? t("dictionary.dialog.conflictRule", {
              from: f.existing.phrase,
              to: f.existing.replacement ?? "",
            })
          : t("dictionary.dialog.conflictWord", { word: f.existing.phrase });
      case "failed":
        return f.reason === "unknown"
          ? t("history.flag.failed")
          : t(SAVE_FAILURE_KEY[f.reason], { appName: t("rust.appName") });
    }
  };

  const radio = (value: FlagKind, label: string, hint: string) => (
    <label
      className={`flex cursor-pointer items-start gap-2.5 rounded-(--wb-radius-sm) border px-3 py-2 ${kind === value ? "border-(--wb-accent) bg-(--wb-fill-active)" : "border-(--wb-line)"}`}
    >
      <input
        type="radio"
        name={kindName}
        value={value}
        checked={kind === value}
        onChange={() => {
          setKind(value);
          setFeedback(null);
        }}
        data-testid={`wb-flag-kind-${value}`}
        className="mt-0.5 accent-(--wb-accent)"
      />
      <span className="flex flex-col">
        <span className="text-[13.5px] font-medium">{label}</span>
        <span className="text-[12.5px] text-(--wb-ink-2)">{hint}</span>
      </span>
    </label>
  );

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !busy && onOpenChange(o)}
      title={t("history.flag.title")}
      description={t("history.flag.description")}
      closeLabel={t("history.flag.close")}
      initialFocusRef={firstWordRef as React.RefObject<HTMLElement>}
      contentFades={false}
      footer={
        <>
          <Button onClick={() => onOpenChange(false)} disabled={busy}>
            {t("history.flag.cancel")}
          </Button>
          <Button
            variant="primary"
            type="submit"
            form="wb-flag-form"
            disabled={!valid}
            data-testid="wb-flag-save"
          >
            {busy && <Loader2 size={14} className="animate-spin" aria-hidden />}
            {t("history.flag.save")}
          </Button>
        </>
      }
    >
      <form
        id="wb-flag-form"
        data-testid="wb-flag-dialog"
        onSubmit={(e) => {
          e.preventDefault();
          if (valid) void save(null);
        }}
        className="flex flex-col gap-4"
        aria-describedby={feedback ? feedbackId : undefined}
      >
        <div className="flex flex-col gap-1.5">
          <span id={wordsLabelId} className="text-[13px] font-medium">
            {t("history.flag.pick")}
          </span>
          <div
            role="group"
            aria-labelledby={wordsLabelId}
            data-testid="wb-flag-words"
            className="flex max-h-48 flex-wrap gap-1 overflow-y-auto rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface) p-2"
          >
            {words.map((w, i) => {
              const picked = pick !== null && i >= pick[0] && i <= pick[1];
              return (
                <button
                  key={`${w.index}`}
                  ref={i === 0 ? firstWordRef : undefined}
                  type="button"
                  aria-pressed={picked}
                  data-testid="wb-flag-word"
                  onClick={() => choose(i)}
                  className={`cursor-pointer rounded-[6px] px-1.5 py-0.5 text-[14px] ${picked ? "bg-(--wb-ink) text-(--wb-on-ink)" : "text-(--wb-ink) hover:bg-(--wb-fill-hover)"} ${FOCUS_RING}`}
                >
                  {w.text}
                </button>
              );
            })}
            {words.length === 0 && (
              <span className="text-[13px] text-(--wb-ink-2)">
                {t("history.flag.noWords")}
              </span>
            )}
          </div>
        </div>
        <div className="flex flex-col gap-1.5">
          <label htmlFor={toId} className="text-[13px] font-medium">
            {pick
              ? t("history.flag.correctionFor", { word: from })
              : t("history.flag.correction")}
          </label>
          <input
            id={toId}
            ref={toRef}
            value={to}
            disabled={!pick}
            onChange={(e) => {
              setTo(e.target.value);
              setFeedback(null);
            }}
            placeholder={t("history.flag.correctionPlaceholder")}
            aria-invalid={tooLong || undefined}
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            className={`${INPUT_CLASS} disabled:opacity-50`}
            data-testid="wb-flag-to"
          />
        </div>
        <fieldset className="flex flex-col gap-2">
          <legend className="sr-only">{t("history.flag.kindLabel")}</legend>
          {radio(
            "rule",
            t("history.flag.rule"),
            pick && toTrim && !unchanged
              ? t("history.flag.ruleHintFilled", { from, to: toTrim })
              : t("history.flag.ruleHint"),
          )}
          {radio("word", t("history.flag.word"), t("history.flag.wordHint"))}
        </fieldset>
        {feedback && (
          <div
            id={feedbackId}
            role="alert"
            data-testid="wb-flag-feedback"
            data-kind={feedback.kind}
            className="flex flex-wrap items-center justify-between gap-2 rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) px-3 py-2 text-[13px]"
          >
            <span>{feedbackText(feedback)}</span>
            {feedback.kind === "conflict" && (
              <Button
                onClick={() => void save(feedback.existing.id)}
                disabled={busy}
                data-testid="wb-flag-replace-existing"
              >
                {t("dictionary.dialog.replaceExisting")}
              </Button>
            )}
          </div>
        )}
      </form>
    </Dialog>
  );
}
