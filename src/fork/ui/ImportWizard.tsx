// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The four-step CSV import wizard (S-16, C-64; S-17 reuses it for
 * snippets):
 *
 *   1. Choose: browse (the system open panel) or drop a `.csv` on the step
 *      (a page drop zone, so the file is imported and never transcribed,
 *      C-205); the size cap and the file reading are the caller's
 *      (`readFile`). "Accepted format" opens the caller's help in place.
 *   2. Columns: the caller's fields mapped to the file's columns (first
 *      guess from the header row), with a preview.
 *   3. Review: the caller renders what the import would do and keeps its
 *      own choices (`initialChoices`, `renderReview`).
 *   4. Done: the caller's summary of what `commit` did.
 *
 * Built on Handy's `Dialog` (focus trap, Esc, labelled), with the Hub
 * tokens. Every step heading is announced (`aria-live`).
 */
import { useEffect, useId, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { FileUp, Loader2 } from "lucide-react";
import { Dialog } from "@/components/ui/Dialog";
import { FORK_NAMESPACE } from "../i18n";
import { useDropZone } from "../hub/dropRouter";
import { Button, FOCUS_RING } from "./Button";
import {
  columnLetter,
  guessHeader,
  guessMapping,
  mapRows,
  parseTable,
  type ColumnMapping,
  type ImportField,
  type MappedRow,
  type ParsedTable,
} from "./importTable";

export type { ImportField, MappedRow } from "./importTable";

/** What `readFile` found. `error` is a sentence for the user. */
export type ImportReadResult =
  | { ok: true; name: string; text: string }
  | { ok: false; error: string };

export interface ImportWizardProps<Review, Choices> {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  /** One line under the title on step 1. */
  description: string;
  fields: ImportField[];
  /** The "Accepted format" help shown in place on step 1. */
  formatHelp: ReactNode;
  readFile: (path: string) => Promise<ImportReadResult>;
  /** Step 3's data, from the mapped rows. Throw to show an error. */
  review: (rows: MappedRow[]) => Promise<Review>;
  initialChoices: (review: Review) => Choices;
  renderReview: (
    review: Review,
    choices: Choices,
    setChoices: (choices: Choices) => void,
  ) => ReactNode;
  /** Whether step 3 has anything to import (the Import button). */
  canCommit: (review: Review, choices: Choices) => boolean;
  /** Imports; returns step 4's summary. Throw to show an error. */
  commit: (
    rows: MappedRow[],
    review: Review,
    choices: Choices,
  ) => Promise<ReactNode>;
  testId?: string;
}

type Step = "choose" | "columns" | "review" | "done";
const STEPS: Step[] = ["choose", "columns", "review", "done"];
const PREVIEW_ROWS = 5;

const isCsvPath = (p: string) => /\.csv$/i.test(p);

export function ImportWizard<Review, Choices>(
  props: ImportWizardProps<Review, Choices>,
) {
  const { open, onOpenChange } = props;
  // Remount per opening: every import starts at step 1 with no state.
  const [session, setSession] = useState(0);
  useEffect(() => {
    if (open) setSession((s) => s + 1);
  }, [open]);
  if (!open) return null;
  return <WizardBody key={session} {...props} onOpenChange={onOpenChange} />;
}

function WizardBody<Review, Choices>({
  onOpenChange,
  title,
  description,
  fields,
  formatHelp,
  readFile,
  review: makeReview,
  initialChoices,
  renderReview,
  canCommit,
  commit,
  testId = "wb-import-wizard",
}: ImportWizardProps<Review, Choices>) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [step, setStep] = useState<Step>("choose");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string | null>(null);
  const [table, setTable] = useState<ParsedTable | null>(null);
  const [hasHeader, setHasHeader] = useState(false);
  const [mapping, setMapping] = useState<ColumnMapping>({});
  const [review, setReview] = useState<Review | null>(null);
  const [choices, setChoices] = useState<Choices | null>(null);
  const [summary, setSummary] = useState<ReactNode>(null);
  const [helpOpen, setHelpOpen] = useState(false);
  const helpId = useId();
  const headingId = useId();
  const dropRef = useRef<HTMLDivElement>(null);

  const rows = useMemo(
    () => (table ? mapRows(table, fields, mapping, hasHeader) : []),
    [table, fields, mapping, hasHeader],
  );
  const mappingComplete = fields.every(
    (f) => !f.required || (mapping[f.id] ?? null) !== null,
  );

  const load = async (path: string) => {
    setError(null);
    setBusy(true);
    try {
      const result = await readFile(path);
      if (!result.ok) {
        setError(result.error);
        return;
      }
      const parsed = parseTable(result.text);
      if (parsed.failed) {
        setError(t("ui.importWizard.errors.unreadable"));
        return;
      }
      if (parsed.records.length === 0) {
        setError(t("ui.importWizard.errors.empty"));
        return;
      }
      const header = guessHeader(parsed, fields);
      if (header && parsed.records.length < 2) {
        setError(t("ui.importWizard.errors.empty"));
        return;
      }
      setFileName(result.name);
      setTable(parsed);
      setHasHeader(header);
      setMapping(guessMapping(parsed, fields, header));
      setStep("columns");
    } catch (e) {
      console.warn("fork: reading the import file failed", e);
      setError(t("ui.importWizard.errors.unreadable"));
    } finally {
      setBusy(false);
    }
  };

  const dropActive = useDropZone(dropRef, {
    disabled: step !== "choose" || busy,
    onDrop: async (paths) => {
      const path = paths.find(isCsvPath) ?? paths[0];
      if (path) await load(path);
    },
  });

  const browse = async () => {
    try {
      const picked = await openFileDialog({
        multiple: false,
        directory: false,
        filters: [
          { name: t("ui.importWizard.csvFilter"), extensions: ["csv"] },
        ],
      });
      if (typeof picked === "string") await load(picked);
    } catch (e) {
      console.warn("fork: the open panel failed", e);
      setError(t("ui.importWizard.errors.unreadable"));
    }
  };

  const toReview = async () => {
    setError(null);
    setBusy(true);
    try {
      const r = await makeReview(rows);
      setReview(r);
      setChoices(initialChoices(r));
      setStep("review");
    } catch (e) {
      console.warn("fork: import review failed", e);
      setError(t("ui.importWizard.errors.failed"));
    } finally {
      setBusy(false);
    }
  };

  const toDone = async () => {
    if (review === null || choices === null) return;
    setError(null);
    setBusy(true);
    try {
      setSummary(await commit(rows, review, choices));
      setStep("done");
    } catch (e) {
      console.warn("fork: import failed", e);
      setError(t("ui.importWizard.errors.failed"));
    } finally {
      setBusy(false);
    }
  };

  const index = STEPS.indexOf(step);
  const stepTitle = t(`ui.importWizard.steps.${step}`);

  const footer = (
    <>
      {step !== "done" && (
        <Button onClick={() => onOpenChange(false)}>
          {t("ui.importWizard.cancel")}
        </Button>
      )}
      {(step === "columns" || step === "review") && (
        <Button
          disabled={busy}
          onClick={() => {
            setError(null);
            setStep(step === "review" ? "columns" : "choose");
          }}
        >
          {t("ui.importWizard.back")}
        </Button>
      )}
      {step === "columns" && (
        <Button
          variant="primary"
          disabled={busy || !mappingComplete || rows.length === 0}
          onClick={() => void toReview()}
          data-testid={`${testId}-next`}
        >
          {busy && <Loader2 size={14} className="animate-spin" aria-hidden />}
          {t("ui.importWizard.next")}
        </Button>
      )}
      {step === "review" && review !== null && choices !== null && (
        <Button
          variant="primary"
          disabled={busy || !canCommit(review, choices)}
          onClick={() => void toDone()}
          data-testid={`${testId}-import`}
        >
          {busy && <Loader2 size={14} className="animate-spin" aria-hidden />}
          {t("ui.importWizard.import")}
        </Button>
      )}
      {step === "done" && (
        <Button
          variant="primary"
          onClick={() => onOpenChange(false)}
          data-testid={`${testId}-done`}
        >
          {t("ui.importWizard.done")}
        </Button>
      )}
    </>
  );

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      title={title}
      closeLabel={t("ui.importWizard.close")}
      footer={footer}
      className="max-w-2xl! bg-(--wb-canvas)! text-(--wb-ink)"
      contentFades={false}
    >
      <div
        data-testid={testId}
        data-step={step}
        className="flex flex-col gap-4"
      >
        <ol
          aria-label={t("ui.importWizard.stepsLabel")}
          className="flex flex-wrap gap-x-4 gap-y-1 text-[12.5px] text-(--wb-ink-2)"
        >
          {STEPS.map((s, i) => (
            <li
              key={s}
              aria-current={s === step ? "step" : undefined}
              className={`flex items-center gap-1.5 ${s === step ? "font-semibold text-(--wb-ink)" : ""}`}
            >
              <span
                aria-hidden="true"
                className={`grid size-5 place-items-center rounded-full text-[11px] ${i <= index ? "bg-(--wb-accent-fill) text-(--wb-on-accent)" : "border border-(--wb-line-strong)"}`}
              >
                {i + 1}
              </span>
              {t(`ui.importWizard.steps.${s}`)}
            </li>
          ))}
        </ol>
        <h3
          id={headingId}
          aria-live="polite"
          className="sr-only"
          data-testid={`${testId}-step-title`}
        >
          {t("ui.importWizard.stepOf", {
            step: index + 1,
            total: STEPS.length,
            title: stepTitle,
          })}
        </h3>

        {step === "choose" && (
          <section aria-labelledby={headingId} className="flex flex-col gap-3">
            <p className="text-[13.5px] text-(--wb-ink-2)">{description}</p>
            <div
              ref={dropRef}
              data-testid={`${testId}-drop`}
              data-drop-active={dropActive || undefined}
              className={`flex flex-col items-center gap-3 rounded-(--wb-radius-card) border-2 border-dashed px-6 py-8 text-center ${dropActive ? "border-(--wb-accent) bg-(--wb-selected)" : "border-(--wb-line-strong)"}`}
            >
              <FileUp
                size={28}
                aria-hidden="true"
                className="text-(--wb-ink-2)"
              />
              <p className="text-[13.5px]">{t("ui.importWizard.dropHint")}</p>
              <Button
                variant="primary"
                disabled={busy}
                onClick={() => void browse()}
                data-testid={`${testId}-browse`}
              >
                {busy && (
                  <Loader2 size={14} className="animate-spin" aria-hidden />
                )}
                {t("ui.importWizard.browse")}
              </Button>
              <p className="text-[12.5px] text-(--wb-ink-2)">
                {t("ui.importWizard.limit")}
              </p>
            </div>
            <div>
              <button
                type="button"
                aria-expanded={helpOpen}
                aria-controls={helpId}
                onClick={() => setHelpOpen((o) => !o)}
                className={`cursor-pointer rounded text-[13px] font-medium text-(--wb-accent) underline underline-offset-2 ${FOCUS_RING}`}
                data-testid={`${testId}-format`}
              >
                {t("ui.importWizard.formatLink")}
              </button>
              {helpOpen && (
                <div
                  id={helpId}
                  className="mt-2 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-4 text-[13px] leading-5"
                >
                  {formatHelp}
                </div>
              )}
            </div>
          </section>
        )}

        {step === "columns" && table && (
          <section aria-labelledby={headingId} className="flex flex-col gap-3">
            <p className="text-[13.5px] text-(--wb-ink-2)">
              {t("ui.importWizard.fileSummary", {
                name: fileName ?? "",
                count: rows.length,
              })}
            </p>
            <label className="flex items-center gap-2 text-[13.5px]">
              <input
                type="checkbox"
                checked={hasHeader}
                onChange={(e) => {
                  const header = e.target.checked;
                  setHasHeader(header);
                  setMapping(guessMapping(table, fields, header));
                }}
                className={`size-4 accent-(--wb-accent-fill) ${FOCUS_RING}`}
                data-testid={`${testId}-header`}
              />
              {t("ui.importWizard.hasHeader")}
            </label>
            <div className="grid gap-3 sm:grid-cols-2">
              {fields.map((field) => (
                <label key={field.id} className="flex flex-col gap-1">
                  <span className="text-[13px] font-medium">
                    {field.label}
                    {!field.required && (
                      <span className="font-normal text-(--wb-ink-2)">
                        {" "}
                        {t("ui.importWizard.optional")}
                      </span>
                    )}
                  </span>
                  <select
                    value={mapping[field.id] ?? ""}
                    onChange={(e) =>
                      setMapping({
                        ...mapping,
                        [field.id]:
                          e.target.value === "" ? null : Number(e.target.value),
                      })
                    }
                    className={`rounded-(--wb-radius-control) border border-(--wb-line-strong) bg-(--wb-card) px-2.5 py-2 text-[13px] text-(--wb-ink) ${FOCUS_RING}`}
                    data-testid={`${testId}-column-${field.id}`}
                  >
                    <option value="">{t("ui.importWizard.noColumn")}</option>
                    {Array.from({ length: table.columns }, (_, i) => {
                      const head = hasHeader
                        ? table.records[0]?.[i]?.trim()
                        : "";
                      return (
                        <option key={i} value={i}>
                          {head
                            ? t("ui.importWizard.columnNamed", {
                                letter: columnLetter(i),
                                name: head,
                              })
                            : t("ui.importWizard.column", {
                                letter: columnLetter(i),
                              })}
                        </option>
                      );
                    })}
                  </select>
                </label>
              ))}
            </div>
            {!mappingComplete && (
              <p role="status" className="text-[13px] text-(--wb-ink-2)">
                {t("ui.importWizard.mapRequired")}
              </p>
            )}
            <div className="overflow-x-auto rounded-(--wb-radius-card) border border-(--wb-line)">
              <table
                className="w-full text-start text-[13px]"
                data-testid={`${testId}-preview`}
              >
                <caption className="sr-only">
                  {t("ui.importWizard.previewCaption", {
                    count: Math.min(PREVIEW_ROWS, rows.length),
                  })}
                </caption>
                <thead className="bg-(--wb-card) text-(--wb-ink-2)">
                  <tr>
                    <th
                      scope="col"
                      className="px-3 py-2 text-start font-medium"
                    >
                      {t("ui.importWizard.rowHeader")}
                    </th>
                    {fields.map((f) => (
                      <th
                        key={f.id}
                        scope="col"
                        className="px-3 py-2 text-start font-medium"
                      >
                        {f.label}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {rows.slice(0, PREVIEW_ROWS).map((r) => (
                    <tr key={r.row} className="border-t border-(--wb-line)">
                      <td className="px-3 py-1.5 text-(--wb-ink-2)">{r.row}</td>
                      {fields.map((f) => (
                        <td
                          key={f.id}
                          className="max-w-60 truncate px-3 py-1.5"
                        >
                          {r.values[f.id]}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
        )}

        {step === "review" && review !== null && choices !== null && (
          <section aria-labelledby={headingId} className="flex flex-col gap-3">
            {renderReview(review, choices, setChoices)}
          </section>
        )}

        {step === "done" && (
          <section
            aria-labelledby={headingId}
            className="flex flex-col gap-3"
            data-testid={`${testId}-summary`}
          >
            {summary}
          </section>
        )}

        {error && (
          <p
            role="alert"
            data-testid={`${testId}-error`}
            className="rounded-(--wb-radius-control) border border-(--wb-line-strong) bg-(--wb-card) px-3 py-2 text-[13px]"
          >
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}
