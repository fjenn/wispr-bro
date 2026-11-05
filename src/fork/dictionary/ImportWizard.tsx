// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Dictionary › Import (S-16, C-64): the shared `ImportWizard` with the
 * dictionary's two columns (word or "from", optional replacement), the
 * review of new rows, duplicates, conflicts (keep mine / use the file's,
 * per row) and rows that cannot be imported, and the counts at the end.
 * The format is `docs/fork/user/import-format.md`; the same rules are shown in
 * place under "Accepted format" (no web page is opened).
 */
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  type ImportReview,
  type ImportRow,
  type ReviewRow,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, Checkbox, FOCUS_RING } from "../ui";
import {
  ImportWizard,
  type ImportField,
  type ImportReadResult,
  type MappedRow,
} from "../ui/ImportWizard";
import { useDictionaryStore } from "./useDictionary";

/** Header names the column guess recognizes (lowercase). */
const PHRASE_HEADERS = [
  "phrase",
  "word",
  "words",
  "term",
  "name",
  "from",
  "misspelling",
  "spoken",
  "original",
  "vocabulary",
];
const REPLACEMENT_HEADERS = [
  "replacement",
  "replace with",
  "to",
  "correct",
  "correct spelling",
  "correction",
  "written",
  "with",
];

const toImportRows = (rows: MappedRow[]): ImportRow[] =>
  rows.map((r) => ({
    row: r.row,
    phrase: r.values.phrase ?? "",
    replacement: r.values.replacement?.trim() ? r.values.replacement : null,
  }));

/** Row numbers whose file version replaces the dictionary's. */
type Choices = number[];

function FormatHelp() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div className="flex flex-col gap-2" data-testid="wb-dictionary-format">
      <p>{t("dictionary.import.format.intro")}</p>
      <pre className="overflow-x-auto rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface) px-3 py-2 font-(family-name:--wb-font-mono) text-[13px] text-(--wb-ink)">
        {"phrase,replacement\nSiobhán,\nKubernetes,\ngonna,going to"}
      </pre>
      <ul className="list-disc ps-5">
        <li>{t("dictionary.import.format.columns")}</li>
        <li>{t("dictionary.import.format.header")}</li>
        <li>{t("dictionary.import.format.limits")}</li>
        <li>{t("dictionary.import.format.encoding")}</li>
      </ul>
    </div>
  );
}

function RuleText({
  phrase,
  replacement,
}: {
  phrase: string;
  replacement: string | null;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <span className="break-words">
      {replacement
        ? t("dictionary.entry.rule", { from: phrase, to: replacement })
        : phrase}
    </span>
  );
}

function Review({
  review,
  choices,
  setChoices,
}: {
  review: ImportReview;
  choices: Choices;
  setChoices: (c: Choices) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const overLimit = Math.max(0, review.new_entries.length - review.room);
  const conflictRows = review.conflicts.map((c) => c.incoming.row);
  const toggle = (row: number, on: boolean) =>
    setChoices(on ? [...choices, row] : choices.filter((r) => r !== row));
  const stat = (id: string, count: number) => (
    <li
      data-testid={`wb-import-count-${id}`}
      data-count={count}
      className="rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-4 py-3"
    >
      <span className="block font-(family-name:--wb-font-display) text-[28px] leading-8 tabular-nums">
        {count}
      </span>
      <span className="text-[13px] text-(--wb-ink-2)">
        {t(`dictionary.import.review.${id}`, { count })}
      </span>
    </li>
  );
  const sample = (rows: ReviewRow[]) => rows.slice(0, 50);
  return (
    <div className="flex flex-col gap-4" data-testid="wb-dictionary-review">
      <ul className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {stat("new", review.new_entries.length)}
        {stat("duplicates", review.duplicates.length)}
        {stat("conflicts", review.conflicts.length)}
        {stat("invalid", review.invalid.length)}
      </ul>
      {overLimit > 0 && (
        <p role="status" className="text-[14px] text-(--wb-ink)">
          {t("dictionary.import.review.overLimit", {
            count: overLimit,
            room: review.room,
          })}
        </p>
      )}
      {review.new_entries.length > 0 && (
        <details className="rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface) px-4 py-3">
          <summary
            className={`cursor-pointer text-[14px] font-medium ${FOCUS_RING}`}
          >
            {t("dictionary.import.review.showNew")}
          </summary>
          <ul className="mt-2 flex flex-col gap-1 text-[14px] text-(--wb-ink-2)">
            {sample(review.new_entries).map((r) => (
              <li key={r.row}>
                <RuleText phrase={r.phrase} replacement={r.replacement} />
              </li>
            ))}
            {review.new_entries.length > 50 && (
              <li className="text-(--wb-ink-2)">
                {t("dictionary.import.review.more", {
                  count: review.new_entries.length - 50,
                })}
              </li>
            )}
          </ul>
        </details>
      )}
      {review.conflicts.length > 0 && (
        <fieldset
          className="flex flex-col gap-3 rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface) px-4 pt-1 pb-4"
          data-testid="wb-dictionary-conflicts"
        >
          <legend className="px-1 text-[14px] font-semibold">
            {t("dictionary.import.review.conflictsTitle")}
          </legend>
          <p className="text-[13px] text-(--wb-ink-2)">
            {t("dictionary.import.review.conflictsHelp")}
          </p>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setChoices(conflictRows)}
            >
              {t("dictionary.import.review.useAllFile")}
            </Button>
            <Button variant="outline" size="sm" onClick={() => setChoices([])}>
              {t("dictionary.import.review.keepAllMine")}
            </Button>
          </div>
          <ul className="flex flex-col gap-1.5">
            {review.conflicts.map((c) => {
              const on = choices.includes(c.incoming.row);
              return (
                <li key={c.incoming.row}>
                  <label className="flex cursor-pointer items-start gap-2.5 text-[14px]">
                    <Checkbox
                      checked={on}
                      onChange={(e) => toggle(c.incoming.row, e.target.checked)}
                      className="mt-px"
                      data-testid={`wb-conflict-${c.incoming.row}`}
                    />
                    <span className="flex flex-col">
                      <span>
                        {t("dictionary.import.review.useFile")}{" "}
                        <strong>
                          <RuleText
                            phrase={c.incoming.phrase}
                            replacement={c.incoming.replacement}
                          />
                        </strong>
                      </span>
                      <span className="text-(--wb-ink-2)">
                        {t("dictionary.import.review.yours")}{" "}
                        <RuleText
                          phrase={c.existing.phrase}
                          replacement={c.existing.replacement}
                        />
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
        <details className="rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface) px-4 py-3">
          <summary
            className={`cursor-pointer text-[14px] font-medium ${FOCUS_RING}`}
          >
            {t("dictionary.import.review.showInvalid")}
          </summary>
          <ul
            className="mt-2 flex flex-col gap-1 text-[14px] text-(--wb-ink-2)"
            data-testid="wb-dictionary-invalid"
          >
            {review.invalid.slice(0, 100).map((r) => (
              <li key={r.row}>
                {t("dictionary.import.review.invalidRow", {
                  row: r.row,
                  phrase: r.phrase || "—",
                  reason: t(`dictionary.problem.${r.problem}`, {
                    max: 60,
                    maxReplacement: 200,
                    count: r.problem === "replacement_too_long" ? 200 : 60,
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

export function DictionaryImportWizard({
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
        id: "phrase",
        label: t("dictionary.import.fieldPhrase"),
        required: true,
        headerNames: PHRASE_HEADERS,
      },
      {
        id: "replacement",
        label: t("dictionary.import.fieldReplacement"),
        required: false,
        headerNames: REPLACEMENT_HEADERS,
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
    <ImportWizard<ImportReview, Choices>
      open={open}
      onOpenChange={onOpenChange}
      testId="wb-import-wizard"
      title={t("dictionary.import.title")}
      description={t("dictionary.import.description")}
      fields={fields}
      formatHelp={<FormatHelp />}
      readFile={readFile}
      review={async (rows) => {
        const result = await commands.forkDictionaryImportReview(
          toImportRows(rows),
        );
        if (result.status !== "ok")
          throw new Error(JSON.stringify(result.error));
        return result.data;
      }}
      initialChoices={() => []}
      renderReview={(review, choices, setChoices) => (
        <Review review={review} choices={choices} setChoices={setChoices} />
      )}
      canCommit={(review, choices) =>
        (review.new_entries.length > 0 && review.room > 0) || choices.length > 0
      }
      commit={async (rows, _review, choices) => {
        const result = await commands.forkDictionaryImport(
          toImportRows(rows),
          choices,
        );
        if (result.status !== "ok")
          throw new Error(JSON.stringify(result.error));
        void useDictionaryStore.getState().reload();
        const o = result.data;
        return (
          <div className="flex flex-col gap-2 text-[15px]">
            <p
              className="type-serif-title text-(--wb-ink)"
              data-testid="wb-import-added"
              data-count={o.added + o.replaced}
            >
              {t("dictionary.import.done.title", {
                count: o.added + o.replaced,
              })}
            </p>
            <ul className="flex flex-col gap-0.5 text-(--wb-ink-2)">
              <li>{t("dictionary.import.done.added", { count: o.added })}</li>
              {o.replaced > 0 && (
                <li>
                  {t("dictionary.import.done.replaced", { count: o.replaced })}
                </li>
              )}
              {o.skipped > 0 && (
                <li>
                  {t("dictionary.import.done.skipped", { count: o.skipped })}
                </li>
              )}
              {o.kept > 0 && (
                <li>{t("dictionary.import.done.kept", { count: o.kept })}</li>
              )}
              {o.invalid > 0 && (
                <li>
                  {t("dictionary.import.done.invalid", { count: o.invalid })}
                </li>
              )}
              {o.over_limit > 0 && (
                <li>
                  {t("dictionary.import.done.overLimit", {
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
