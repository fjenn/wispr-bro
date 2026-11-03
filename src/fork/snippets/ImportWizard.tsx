// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Snippets › Import (S-17, C-71): S-16's shared `ImportWizard` with the
 * snippets' two columns (what you say, the text), the review of new rows,
 * duplicates, conflicts (keep mine / use the file's, per row) and rows that
 * cannot be imported, and the counts at the end. The file is read by
 * `fork_dictionary_read_file` (same `.csv` and 3 MB rules). Format:
 * `docs/fork/user/import-format.md` › Snippets, shown in place.
 */
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  type SnippetImportReview,
  type SnippetImportRow,
  type SnippetReviewRow,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";
import {
  ImportWizard,
  type ImportField,
  type ImportReadResult,
  type MappedRow,
} from "../ui/ImportWizard";
import { MAX_BODY_CHARS, MAX_TRIGGER_CHARS } from "./SnippetDialog";
import { useSnippetsStore } from "./useSnippets";

/** Header names the column guess recognizes (lowercase). */
const TRIGGER_HEADERS = [
  "trigger",
  "triggers",
  "cue",
  "shortcut",
  "abbreviation",
  "keyword",
  "name",
  "phrase",
  "spoken",
  "say",
  "when i say",
];
const BODY_HEADERS = [
  "text",
  "body",
  "snippet",
  "expansion",
  "content",
  "replacement",
  "write",
  "value",
];

const toRows = (rows: MappedRow[]): SnippetImportRow[] =>
  rows.map((r) => ({
    row: r.row,
    trigger: r.values.trigger ?? "",
    body: r.values.body ?? "",
  }));

/** Row numbers whose file version replaces the saved snippet. */
type Choices = number[];

/** The first line of a text, for a one-line preview. */
function Preview({ row }: { row: { trigger: string; body: string } }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const lines = row.body.split("\n");
  return (
    <span className="break-words">
      {t("snippets.entry.pair", {
        trigger: row.trigger,
        text: lines[0] + (lines.length > 1 ? " …" : ""),
      })}
    </span>
  );
}

function FormatHelp() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div className="flex flex-col gap-2" data-testid="wb-snippets-format">
      <p>{t("snippets.import.format.intro")}</p>
      <pre className="overflow-x-auto rounded-(--wb-radius-control) bg-(--wb-selected) px-3 py-2 font-mono text-[12.5px]">
        {
          'trigger,text\nmy sign-off,"Best wishes,\nFinn"\nmy hours,Mondays to Thursdays 9-17'
        }
      </pre>
      <ul className="list-disc ps-5">
        <li>{t("snippets.import.format.columns")}</li>
        <li>{t("snippets.import.format.lines")}</li>
        <li>
          {t("snippets.import.format.limits", {
            count: MAX_TRIGGER_CHARS,
            max: MAX_TRIGGER_CHARS,
            maxBody: MAX_BODY_CHARS,
          })}
        </li>
        <li>{t("snippets.import.format.encoding")}</li>
      </ul>
    </div>
  );
}

function Review({
  review,
  choices,
  setChoices,
}: {
  review: SnippetImportReview;
  choices: Choices;
  setChoices: (c: Choices) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const overLimit = Math.max(0, review.new_snippets.length - review.room);
  const conflictRows = review.conflicts.map((c) => c.incoming.row);
  const toggle = (row: number, on: boolean) =>
    setChoices(on ? [...choices, row] : choices.filter((r) => r !== row));
  const stat = (id: string, count: number) => (
    <li
      data-testid={`wb-import-count-${id}`}
      data-count={count}
      className="rounded-(--wb-radius-control) border border-(--wb-line) bg-(--wb-card) px-3 py-2"
    >
      <span className="block text-[20px] font-semibold tabular-nums">
        {count}
      </span>
      <span className="text-[12.5px] text-(--wb-ink-2)">
        {t(`snippets.import.review.${id}`, { count })}
      </span>
    </li>
  );
  const sample = (rows: SnippetReviewRow[]) => rows.slice(0, 50);
  return (
    <div className="flex flex-col gap-4" data-testid="wb-snippets-review">
      <ul className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {stat("new", review.new_snippets.length)}
        {stat("duplicates", review.duplicates.length)}
        {stat("conflicts", review.conflicts.length)}
        {stat("invalid", review.invalid.length)}
      </ul>
      {overLimit > 0 && (
        <p role="status" className="text-[13px]">
          {t("snippets.import.review.overLimit", {
            count: overLimit,
            room: review.room,
          })}
        </p>
      )}
      {review.ignored_rows > 0 && (
        <p role="status" className="text-[13px]">
          {t("snippets.import.review.ignoredRows", {
            count: review.ignored_rows,
          })}
        </p>
      )}
      {review.new_snippets.length > 0 && (
        <details className="rounded-(--wb-radius-card) border border-(--wb-line) px-3 py-2">
          <summary
            className={`cursor-pointer text-[13px] font-medium ${FOCUS_RING}`}
          >
            {t("snippets.import.review.showNew")}
          </summary>
          <ul className="mt-2 flex flex-col gap-1 text-[13px]">
            {sample(review.new_snippets).map((r) => (
              <li key={r.row}>
                <Preview row={r} />
              </li>
            ))}
            {review.new_snippets.length > 50 && (
              <li className="text-(--wb-ink-2)">
                {t("snippets.import.review.more", {
                  count: review.new_snippets.length - 50,
                })}
              </li>
            )}
          </ul>
        </details>
      )}
      {review.conflicts.length > 0 && (
        <fieldset
          className="flex flex-col gap-2 rounded-(--wb-radius-card) border border-(--wb-line) px-3 py-2"
          data-testid="wb-snippets-conflicts"
        >
          <legend className="px-1 text-[13px] font-medium">
            {t("snippets.import.review.conflictsTitle")}
          </legend>
          <p className="text-[12.5px] text-(--wb-ink-2)">
            {t("snippets.import.review.conflictsHelp")}
          </p>
          <div className="flex gap-2">
            <Button onClick={() => setChoices(conflictRows)}>
              {t("snippets.import.review.useAllFile")}
            </Button>
            <Button onClick={() => setChoices([])}>
              {t("snippets.import.review.keepAllMine")}
            </Button>
          </div>
          <ul className="flex flex-col gap-1.5">
            {review.conflicts.map((c) => {
              const on = choices.includes(c.incoming.row);
              return (
                <li key={c.incoming.row}>
                  <label className="flex items-start gap-2 text-[13px]">
                    <input
                      type="checkbox"
                      checked={on}
                      onChange={(e) => toggle(c.incoming.row, e.target.checked)}
                      className={`mt-0.5 size-4 accent-(--wb-accent-fill) ${FOCUS_RING}`}
                      data-testid={`wb-conflict-${c.incoming.row}`}
                    />
                    <span className="flex flex-col">
                      <span>
                        {t("snippets.import.review.useFile")}{" "}
                        <strong>
                          <Preview row={c.incoming} />
                        </strong>
                      </span>
                      <span className="text-(--wb-ink-2)">
                        {t("snippets.import.review.yours")}{" "}
                        <Preview row={c.existing} />
                      </span>
                    </span>
                  </label>
                </li>
              );
            })}
          </ul>
        </fieldset>
      )}
      {review.invalid.length > 0 && (
        <details className="rounded-(--wb-radius-card) border border-(--wb-line) px-3 py-2">
          <summary
            className={`cursor-pointer text-[13px] font-medium ${FOCUS_RING}`}
          >
            {t("snippets.import.review.showInvalid")}
          </summary>
          <ul
            className="mt-2 flex flex-col gap-1 text-[13px]"
            data-testid="wb-snippets-invalid"
          >
            {review.invalid.slice(0, 100).map((r) => (
              <li key={r.row}>
                {t("snippets.import.review.invalidRow", {
                  row: r.row,
                  trigger: r.trigger || "—",
                  reason: t(`snippets.problem.${r.problem}`, {
                    max: MAX_TRIGGER_CHARS,
                    maxBody: MAX_BODY_CHARS,
                    count:
                      r.problem === "body_too_long"
                        ? MAX_BODY_CHARS
                        : MAX_TRIGGER_CHARS,
                  }),
                })}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

export function SnippetsImportWizard({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const fields: ImportField[] = useMemo(
    () => [
      {
        id: "trigger",
        label: t("snippets.import.fieldTrigger"),
        required: true,
        headerNames: TRIGGER_HEADERS,
      },
      {
        id: "body",
        label: t("snippets.import.fieldBody"),
        required: true,
        headerNames: BODY_HEADERS,
      },
    ],
    [t],
  );

  const readFile = async (path: string): Promise<ImportReadResult> => {
    const result = await commands.forkDictionaryReadFile(path);
    switch (result.status) {
      case "read":
        return { ok: true, name: result.name, text: result.text };
      case "not_csv":
        return { ok: false, error: t("ui.importWizard.errors.notCsv") };
      case "too_large":
        return {
          ok: false,
          error: t("ui.importWizard.errors.tooLarge", {
            size: new Intl.NumberFormat(i18n.language, {
              maximumFractionDigits: 1,
            }).format(result.bytes / (1024 * 1024)),
          }),
        };
      case "unreadable":
        return { ok: false, error: t("ui.importWizard.errors.unreadable") };
    }
  };

  return (
    <ImportWizard<SnippetImportReview, Choices>
      open={open}
      onOpenChange={onOpenChange}
      testId="wb-import-wizard"
      title={t("snippets.import.title")}
      description={t("snippets.import.description")}
      fields={fields}
      formatHelp={<FormatHelp />}
      readFile={readFile}
      review={async (rows) => {
        const result = await commands.forkSnippetsImportReview(toRows(rows));
        if (result.status !== "ok")
          throw new Error(JSON.stringify(result.error));
        return result.data;
      }}
      initialChoices={() => []}
      renderReview={(review, choices, setChoices) => (
        <Review review={review} choices={choices} setChoices={setChoices} />
      )}
      canCommit={(review, choices) =>
        (review.new_snippets.length > 0 && review.room > 0) ||
        choices.length > 0
      }
      commit={async (rows, _review, choices) => {
        const result = await commands.forkSnippetsImport(toRows(rows), choices);
        if (result.status !== "ok")
          throw new Error(JSON.stringify(result.error));
        void useSnippetsStore.getState().reload();
        const o = result.data;
        return (
          <div className="flex flex-col gap-2 text-[13.5px]">
            <p
              className="font-(family-name:--wb-font-display) text-[22px]"
              data-testid="wb-import-added"
              data-count={o.added + o.replaced}
            >
              {t("snippets.import.done.title", {
                count: o.added + o.replaced,
              })}
            </p>
            <ul className="flex flex-col gap-0.5 text-(--wb-ink-2)">
              <li>{t("snippets.import.done.added", { count: o.added })}</li>
              {o.replaced > 0 && (
                <li>
                  {t("snippets.import.done.replaced", { count: o.replaced })}
                </li>
              )}
              {o.skipped > 0 && (
                <li>
                  {t("snippets.import.done.skipped", { count: o.skipped })}
                </li>
              )}
              {o.kept > 0 && (
                <li>{t("snippets.import.done.kept", { count: o.kept })}</li>
              )}
              {o.invalid > 0 && (
                <li>
                  {t("snippets.import.done.invalid", { count: o.invalid })}
                </li>
              )}
              {o.over_limit > 0 && (
                <li>
                  {t("snippets.import.done.overLimit", {
                    count: o.over_limit,
                  })}
                </li>
              )}
            </ul>
          </div>
        );
      }}
    />
  );
}
