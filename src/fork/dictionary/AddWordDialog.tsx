// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Add word" and "Edit" dialog (S-16, C-62): one field for a word or name,
 * or, with "Replace a word" on, a from → to pair. The primary button stays
 * disabled until the fields are filled and within the limits (60
 * characters for the word, 200 for the replacement, as `fork::dictionary`
 * checks). The backend's answer is shown in place: already there, or a
 * replacement for the same word exists (with "Replace it", which edits that
 * entry instead).
 */
import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "../ui/icons";
import { Dialog } from "../ui/Dialog";
import { Switch } from "../ui/Switch";
import {
  commands,
  type DictionaryEntry,
  type EntryInput,
  type EntryProblem,
  type SaveOutcome,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";
import { useDictionaryStore } from "./useDictionary";

export const MAX_PHRASE_CHARS = 60;
export const MAX_REPLACEMENT_CHARS = 200;

const length = (s: string) => [...s.trim()].length;

export interface AddWordDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Edit this entry instead of adding one. */
  entry?: DictionaryEntry | null;
  /** Called with the saved entry. */
  onSaved?: (entry: DictionaryEntry, added: boolean) => void;
}

type Feedback =
  | { kind: "problem"; problem: EntryProblem }
  | { kind: "duplicate"; existing: DictionaryEntry }
  | { kind: "conflict"; existing: DictionaryEntry }
  | { kind: "missing" }
  | { kind: "failed" };

export const INPUT_CLASS = `w-full rounded-(--wb-radius-control) border border-(--wb-line-strong) bg-(--wb-card) px-3 py-2 text-[14px] text-(--wb-ink) placeholder:text-(--wb-ink-3) ${FOCUS_RING}`;

export function AddWordDialog({
  open,
  onOpenChange,
  entry,
  onSaved,
}: AddWordDialogProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const editing = Boolean(entry);
  const [replace, setReplace] = useState(false);
  const [phrase, setPhrase] = useState("");
  const [replacement, setReplacement] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const firstRef = useRef<HTMLInputElement>(null);
  const switchLabelId = useId();
  const phraseId = useId();
  const replacementId = useId();
  const feedbackId = useId();

  useEffect(() => {
    if (!open) return;
    setReplace(entry?.kind === "rule");
    setPhrase(entry?.phrase ?? "");
    setReplacement(entry?.replacement ?? "");
    setFeedback(null);
    setBusy(false);
  }, [open, entry]);

  const phraseLength = length(phrase);
  const replacementLength = length(replacement);
  const phraseTooLong = phraseLength > MAX_PHRASE_CHARS;
  const replacementTooLong = replacementLength > MAX_REPLACEMENT_CHARS;
  const filled =
    phraseLength > 0 && (!replace || replacementLength > 0) && !busy;
  const valid = filled && !phraseTooLong && !(replace && replacementTooLong);

  const input = (): EntryInput => ({
    phrase: phrase.normalize("NFC"),
    replacement: replace ? replacement.normalize("NFC") : null,
  });

  const handle = (result: SaveOutcome) => {
    switch (result.status) {
      case "saved":
        void useDictionaryStore.getState().reload();
        onSaved?.(result.entry, !editing);
        onOpenChange(false);
        return;
      case "duplicate":
        setFeedback({ kind: "duplicate", existing: result.existing });
        return;
      case "conflict":
        setFeedback({ kind: "conflict", existing: result.existing });
        return;
      case "invalid":
        setFeedback({ kind: "problem", problem: result.problem });
        return;
      case "missing":
        setFeedback({ kind: "missing" });
        return;
    }
  };

  const save = async (targetId?: number) => {
    setBusy(true);
    setFeedback(null);
    try {
      const id = targetId ?? entry?.id;
      const result =
        id !== undefined
          ? await commands.forkDictionaryUpdate(id, input())
          : await commands.forkDictionaryAdd(input());
      if (result.status === "ok") handle(result.data);
      else setFeedback({ kind: "failed" });
    } catch (e) {
      console.warn("fork: saving the dictionary entry failed", e);
      setFeedback({ kind: "failed" });
    } finally {
      setBusy(false);
    }
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (valid) void save();
  };

  const existingText = (existing: DictionaryEntry) =>
    existing.replacement
      ? t("dictionary.entry.rule", {
          from: existing.phrase,
          to: existing.replacement,
        })
      : existing.phrase;

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
        return t("dictionary.dialog.duplicate", {
          entry: existingText(f.existing),
        });
      case "conflict":
        return f.existing.kind === "rule"
          ? t("dictionary.dialog.conflictRule", {
              from: f.existing.phrase,
              to: f.existing.replacement ?? "",
            })
          : t("dictionary.dialog.conflictWord", { word: f.existing.phrase });
      case "missing":
        return t("dictionary.dialog.missing");
      case "failed":
        return t("dictionary.dialog.failed");
    }
  };

  const counter = (n: number, max: number, testId: string) => (
    <span
      data-testid={testId}
      className={`text-[12px] ${n > max ? "font-semibold text-(--wb-ink)" : "text-(--wb-ink-2)"}`}
    >
      {t("dictionary.dialog.counter", { count: n, max })}
    </span>
  );

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !busy && onOpenChange(o)}
      title={t(
        editing ? "dictionary.dialog.editTitle" : "dictionary.dialog.addTitle",
      )}
      description={t("dictionary.dialog.description")}
      closeLabel={t("dictionary.dialog.close")}
      initialFocusRef={firstRef as React.RefObject<HTMLElement>}
      contentFades={false}
      footer={
        <>
          <Button onClick={() => onOpenChange(false)} disabled={busy}>
            {t("dictionary.dialog.cancel")}
          </Button>
          <Button
            variant="primary"
            type="submit"
            form="wb-dictionary-form"
            disabled={!valid}
            data-testid="wb-dictionary-save"
          >
            {busy && <Loader2 size={14} className="animate-spin" aria-hidden />}
            {t(editing ? "dictionary.dialog.save" : "dictionary.dialog.add")}
          </Button>
        </>
      }
    >
      <form
        id="wb-dictionary-form"
        data-testid="wb-dictionary-dialog"
        onSubmit={onSubmit}
        className="flex flex-col gap-4"
        aria-describedby={feedback ? feedbackId : undefined}
      >
        <div className="flex items-center justify-between gap-3">
          <span id={switchLabelId} className="text-[13.5px] font-medium">
            {t("dictionary.dialog.replaceToggle")}
          </span>
          <Switch
            checked={replace}
            onChange={(on) => {
              setReplace(on);
              setFeedback(null);
            }}
            ariaLabelledBy={switchLabelId}
          />
        </div>
        <div className="flex flex-col gap-1.5">
          <div className="flex items-baseline justify-between">
            <label htmlFor={phraseId} className="text-[13px] font-medium">
              {t(replace ? "dictionary.dialog.from" : "dictionary.dialog.word")}
            </label>
            {counter(phraseLength, MAX_PHRASE_CHARS, "wb-dictionary-counter")}
          </div>
          <input
            id={phraseId}
            ref={firstRef}
            value={phrase}
            onChange={(e) => {
              setPhrase(e.target.value);
              setFeedback(null);
            }}
            placeholder={t(
              replace
                ? "dictionary.dialog.fromPlaceholder"
                : "dictionary.dialog.wordPlaceholder",
            )}
            aria-invalid={phraseTooLong || undefined}
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            className={INPUT_CLASS}
            data-testid="wb-dictionary-phrase"
          />
        </div>
        {replace && (
          <div className="flex flex-col gap-1.5">
            <div className="flex items-baseline justify-between">
              <label
                htmlFor={replacementId}
                className="text-[13px] font-medium"
              >
                {t("dictionary.dialog.to")}
              </label>
              {counter(
                replacementLength,
                MAX_REPLACEMENT_CHARS,
                "wb-dictionary-counter-to",
              )}
            </div>
            <input
              id={replacementId}
              value={replacement}
              onChange={(e) => {
                setReplacement(e.target.value);
                setFeedback(null);
              }}
              placeholder={t("dictionary.dialog.toPlaceholder")}
              aria-invalid={replacementTooLong || undefined}
              autoComplete="off"
              autoCorrect="off"
              spellCheck={false}
              className={INPUT_CLASS}
              data-testid="wb-dictionary-replacement"
            />
          </div>
        )}
        <p className="text-[12.5px] text-(--wb-ink-2)">
          {t(
            replace
              ? "dictionary.dialog.ruleHint"
              : "dictionary.dialog.wordHint",
          )}
        </p>
        {feedback && (
          <div
            id={feedbackId}
            role="alert"
            data-testid="wb-dictionary-feedback"
            data-kind={feedback.kind}
            className="flex flex-wrap items-center justify-between gap-2 rounded-(--wb-radius-control) border border-(--wb-line-strong) bg-(--wb-card) px-3 py-2 text-[13px]"
          >
            <span>{feedbackText(feedback)}</span>
            {feedback.kind === "conflict" && !editing && (
              <Button
                onClick={() => void save(feedback.existing.id)}
                disabled={busy}
                data-testid="wb-dictionary-replace-existing"
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
