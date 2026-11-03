// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Add snippet" and "Edit snippet" dialog (S-17, C-70): what you say (the
 * trigger, ≤ 60 characters) and the text written instead (plain text with
 * line breaks, ≤ 4,000 characters; DEC-12). The primary button stays
 * disabled until both are filled and within the limits, as
 * `fork::snippets::clean` checks. ⌘Return in the text saves; Return makes
 * a new line. The backend's answer is shown in place: already saved, or
 * the trigger is taken by another text (with "Replace it", which edits that
 * snippet instead).
 */
import {
  useEffect,
  useId,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "../ui/icons";
import { Dialog } from "../ui/Dialog";
import {
  commands,
  type SnippetEntry,
  type SnippetInput,
  type SnippetProblem,
  type SnippetSaveOutcome,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, TextArea, TextField } from "../ui";
import { useSnippetsStore } from "./useSnippets";

export const MAX_TRIGGER_CHARS = 60;
export const MAX_BODY_CHARS = 4000;

const triggerLength = (s: string) => [...s.trim()].length;
/** What the backend keeps: trailing whitespace and leading blank lines go. */
const bodyLength = (s: string) =>
  [
    ...s
      .replace(/\r\n?/g, "\n")
      .replace(/^(?:[^\S\n]*\n)+/, "")
      .trimEnd(),
  ].length;

export interface SnippetDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Edit this snippet instead of adding one. */
  snippet?: SnippetEntry | null;
  onSaved?: (snippet: SnippetEntry, added: boolean) => void;
}

type Feedback =
  | { kind: "problem"; problem: SnippetProblem }
  | { kind: "duplicate"; existing: SnippetEntry }
  | { kind: "conflict"; existing: SnippetEntry }
  | { kind: "missing" }
  | { kind: "failed" };

export function SnippetDialog({
  open,
  onOpenChange,
  snippet,
  onSaved,
}: SnippetDialogProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const editing = Boolean(snippet);
  const [trigger, setTrigger] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const firstRef = useRef<HTMLInputElement>(null);
  const triggerId = useId();
  const bodyId = useId();
  const hintId = useId();
  const feedbackId = useId();

  useEffect(() => {
    if (!open) return;
    setTrigger(snippet?.trigger ?? "");
    setBody(snippet?.body ?? "");
    setFeedback(null);
    setBusy(false);
  }, [open, snippet]);

  const tLength = triggerLength(trigger);
  const bLength = bodyLength(body);
  const triggerTooLong = tLength > MAX_TRIGGER_CHARS;
  const bodyTooLong = bLength > MAX_BODY_CHARS;
  const valid =
    tLength > 0 && bLength > 0 && !triggerTooLong && !bodyTooLong && !busy;

  const input = (): SnippetInput => ({
    trigger: trigger.normalize("NFC"),
    body: body.normalize("NFC"),
  });

  const handle = (result: SnippetSaveOutcome) => {
    switch (result.status) {
      case "saved":
        void useSnippetsStore.getState().reload();
        onSaved?.(result.snippet, !editing);
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
      const id = targetId ?? snippet?.id;
      const result =
        id !== undefined
          ? await commands.forkSnippetsUpdate(id, input())
          : await commands.forkSnippetsAdd(input());
      if (result.status === "ok") handle(result.data);
      else setFeedback({ kind: "failed" });
    } catch (e) {
      console.warn("fork: saving the snippet failed", e);
      setFeedback({ kind: "failed" });
    } finally {
      setBusy(false);
    }
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (valid) void save();
  };

  const onBodyKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      if (valid) void save();
    }
  };

  const feedbackText = (f: Feedback): string => {
    switch (f.kind) {
      case "problem":
        return t(`snippets.problem.${f.problem}`, {
          max: MAX_TRIGGER_CHARS,
          maxBody: MAX_BODY_CHARS,
          // The limit the message names picks the plural form.
          count:
            f.problem === "body_too_long" ? MAX_BODY_CHARS : MAX_TRIGGER_CHARS,
        });
      case "duplicate":
        return t("snippets.dialog.duplicate", { trigger: f.existing.trigger });
      case "conflict":
        return t("snippets.dialog.conflict", { trigger: f.existing.trigger });
      case "missing":
        return t("snippets.dialog.missing");
      case "failed":
        return t("snippets.dialog.failed");
    }
  };

  const counter = (n: number, max: number, testId: string) => (
    <span
      data-testid={testId}
      className={`text-[12px] tabular-nums ${n > max ? "font-semibold text-(--wb-danger)" : "text-(--wb-ink-3)"}`}
    >
      {t("snippets.dialog.counter", { count: n, max })}
    </span>
  );

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !busy && onOpenChange(o)}
      title={t(
        editing ? "snippets.dialog.editTitle" : "snippets.dialog.addTitle",
      )}
      description={t("snippets.dialog.description")}
      closeLabel={t("snippets.dialog.close")}
      initialFocusRef={firstRef as React.RefObject<HTMLElement>}
      className="max-w-xl!"
      contentFades={false}
      footer={
        <>
          <Button onClick={() => onOpenChange(false)} disabled={busy}>
            {t("snippets.dialog.cancel")}
          </Button>
          <Button
            variant="primary"
            type="submit"
            form="wb-snippet-form"
            disabled={!valid}
            data-testid="wb-snippet-save"
          >
            {busy && <Loader2 size={14} className="animate-spin" aria-hidden />}
            {t(editing ? "snippets.dialog.save" : "snippets.dialog.add")}
          </Button>
        </>
      }
    >
      <form
        id="wb-snippet-form"
        data-testid="wb-snippet-dialog"
        onSubmit={onSubmit}
        className="flex flex-col gap-4"
        aria-describedby={feedback ? feedbackId : undefined}
      >
        <div className="flex flex-col gap-1.5">
          <div className="flex items-baseline justify-between">
            <label htmlFor={triggerId} className="text-[14px] font-medium">
              {t("snippets.dialog.trigger")}
            </label>
            {counter(tLength, MAX_TRIGGER_CHARS, "wb-snippet-trigger-counter")}
          </div>
          <TextField
            id={triggerId}
            ref={firstRef}
            value={trigger}
            onChange={(e) => {
              setTrigger(e.target.value);
              setFeedback(null);
            }}
            placeholder={t("snippets.dialog.triggerPlaceholder")}
            invalid={triggerTooLong}
            aria-describedby={hintId}
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            data-testid="wb-snippet-trigger"
          />
        </div>
        <div className="flex flex-col gap-1.5">
          <div className="flex items-baseline justify-between">
            <label htmlFor={bodyId} className="text-[14px] font-medium">
              {t("snippets.dialog.body")}
            </label>
            {counter(bLength, MAX_BODY_CHARS, "wb-snippet-body-counter")}
          </div>
          <TextArea
            id={bodyId}
            value={body}
            onChange={(e) => {
              setBody(e.target.value);
              setFeedback(null);
            }}
            onKeyDown={onBodyKey}
            placeholder={t("snippets.dialog.bodyPlaceholder")}
            invalid={bodyTooLong}
            rows={7}
            spellCheck
            className="min-h-40 resize-y"
            data-testid="wb-snippet-body"
          />
        </div>
        <p id={hintId} className="text-[13px] leading-[18px] text-(--wb-ink-2)">
          {t("snippets.dialog.hint")}
        </p>
        {feedback && (
          <div
            id={feedbackId}
            role="alert"
            data-testid="wb-snippet-feedback"
            data-kind={feedback.kind}
            className={`flex flex-wrap items-center justify-between gap-x-3 gap-y-2 rounded-(--wb-radius-sm) px-4 py-3 text-[14px] leading-5 text-(--wb-ink) ${feedback.kind === "failed" || feedback.kind === "problem" ? "bg-(--wb-danger-soft)" : "border border-(--wb-line) bg-(--wb-surface-tint)"}`}
          >
            <span>{feedbackText(feedback)}</span>
            {feedback.kind === "conflict" && !editing && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => void save(feedback.existing.id)}
                disabled={busy}
                data-testid="wb-snippet-replace-existing"
              >
                {t("snippets.dialog.replaceExisting")}
              </Button>
            )}
          </div>
        )}
      </form>
    </Dialog>
  );
}
