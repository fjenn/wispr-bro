// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The review of a sync file, the same on the Mac and the iPhone: what
 * merging it changes, one switch per part, then the result.
 *
 *   Merge from Finn's MacBook Pro                      ✕
 *   Made 17 Nov 2026, 14:02 with Wispr Bro 1.0.0
 *   ┌───────────────────────────────────────────────┐
 *   │ Dictionary   12 new, 2 changed, 1 removed   ◉ │
 *   │ Snippets     3 new                          ◉ │
 *   │ Notes        2 new, 1 changed               ◉ │
 *   │ Settings     Languages, Cleanup             ◉ │
 *   │ Stats        12,840 words on 41 days        ◉ │
 *   └───────────────────────────────────────────────┘
 *   History and recordings are not synced.
 *                                   Cancel   Merge
 *
 * Also the place a file that cannot be read says why.
 */
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import type { MergeReport, SyncChoices } from "../bindings";
import { formatCount } from "../home/StatsRail";
import { FORK_NAMESPACE } from "../i18n";
import { isIos } from "../ios/platform";
import { Button, Dialog, Switch } from "../ui";
import {
  BookA,
  ChartColumn,
  CheckCircle2,
  NotebookPen,
  SlidersHorizontal,
  TextQuote,
  TriangleAlert,
  type Glyph,
} from "../ui/icons";
import {
  ALL_PARTS,
  changesAnything,
  formatWhen,
  type ReadyReview,
  type Sync,
} from "./useSync";

type Part = keyof SyncChoices;
const PARTS: readonly Part[] = [
  "dictionary",
  "snippets",
  "notes",
  "settings",
  "stats",
];
const ICONS: Record<Part, Glyph> = {
  dictionary: BookA,
  snippets: TextQuote,
  notes: NotebookPen,
  settings: SlidersHorizontal,
  stats: ChartColumn,
};

/** "12 new, 2 changed, 1 removed", or `null` when nothing moves. */
function countsLine(
  t: TFunction<"fork">,
  c: MergeReport["dictionary"],
): string | null {
  const parts = [
    c.added > 0 && t("sync.review.added", { count: c.added }),
    c.changed > 0 && t("sync.review.changed", { count: c.changed }),
    c.removed > 0 && t("sync.review.removed", { count: c.removed }),
  ].filter(Boolean);
  return parts.length ? parts.join(", ") : null;
}

/** The other device as the reader would call it. */
export function deviceLabel(
  t: TFunction<"fork">,
  device: { name?: string; device_name?: string; platform: string },
  /** `row`: the name on its own; `sentence`: after "from". */
  form: "row" | "sentence" = "sentence",
): string {
  const name = device.name ?? device.device_name ?? "";
  if (name.trim()) return name;
  const kind = device.platform === "ios" ? "iphone" : "mac";
  return t(`sync.devices.${kind}${form === "row" ? "Name" : ""}`);
}

function partHas(review: ReadyReview, part: Part): boolean {
  return {
    dictionary: review.has_dictionary,
    snippets: review.has_snippets,
    settings: review.has_settings,
    stats: review.has_stats,
    notes: review.has_notes,
  }[part];
}

function partLine(
  t: TFunction<"fork">,
  language: string,
  review: ReadyReview,
  part: Part,
): { text: string; changes: boolean } {
  const report = review.report;
  const same = { text: t("sync.review.same"), changes: false };
  if (!partHas(review, part)) {
    return { text: t("sync.review.notInFile"), changes: false };
  }
  if (part === "settings") {
    if (!report.settings.length) return same;
    return {
      text: report.settings
        .map((key) => t(`sync.settingNames.${key}`, { defaultValue: key }))
        .join(", "),
      changes: true,
    };
  }
  if (part === "stats") {
    const text = t("sync.review.stats", {
      count: report.stats_days,
      words: formatCount(report.stats_words, language),
    });
    return report.stats_changed
      ? { text, changes: true }
      : { text: `${text} · ${t("sync.review.same")}`, changes: false };
  }
  const line = countsLine(t, report[part]);
  return line ? { text: line, changes: true } : same;
}

export function SyncReviewDialog({ sync }: { sync: Sync }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { state } = sync;
  const [choices, setChoices] = useState<SyncChoices>(ALL_PARTS);
  // Opened inside Settings' native modal <dialog>, it must portal into it.
  const anchor = useRef<HTMLSpanElement>(null);
  const [container, setContainer] = useState<HTMLElement | null>(null);
  useLayoutEffect(() => {
    setContainer(anchor.current?.closest("dialog") ?? null);
  }, []);

  const review =
    state.step === "review" || state.step === "merging" || state.step === "done"
      ? state.review
      : null;
  const reviewKey = state.step === "review" ? state.path : null;
  useEffect(() => {
    if (reviewKey) setChoices(ALL_PARTS);
  }, [reviewKey]);

  const open = review !== null || state.step === "problem";
  const done = state.step === "done";
  const merging = state.step === "merging";
  const csv =
    review?.source === "dictionary_csv" || review?.source === "snippets_csv";
  const from = review
    ? csv
      ? review.device_name
      : deviceLabel(t, review)
    : "";
  const shown = PARTS.filter(
    (part) => review && (!csv || partHas(review, part)),
  );
  const willChange =
    review !== null &&
    shown.some(
      (part) =>
        choices[part] && partLine(t, i18n.language, review, part).changes,
    );

  let title: string;
  let description: string | undefined;
  if (state.step === "problem") {
    title = t("sync.problems.title");
  } else if (done) {
    title = t("sync.result.title");
    description = t("sync.result.from", { device: from });
  } else {
    title = t(csv ? "sync.review.titleFile" : "sync.review.title", {
      device: from,
    });
    if (review && !csv) {
      const when = formatWhen(review.created_at, i18n.language);
      description = review.app_version
        ? t("sync.review.madeWith", { when, version: review.app_version })
        : t("sync.review.made", { when });
    }
  }

  const footer =
    state.step === "problem" || done ? (
      <Button
        variant="primary"
        onClick={sync.close}
        data-testid="wb-sync-done"
        className="max-sm:h-11 max-sm:flex-1"
      >
        {t("sync.review.done")}
      </Button>
    ) : (
      <>
        <Button
          variant="secondary"
          onClick={sync.close}
          disabled={merging}
          data-testid="wb-sync-cancel"
          className="max-sm:h-11 max-sm:flex-1"
        >
          {t("sync.review.cancel")}
        </Button>
        <Button
          variant="primary"
          onClick={() => void sync.apply(choices)}
          disabled={merging || !willChange}
          aria-busy={merging}
          data-testid="wb-sync-merge"
          className="max-sm:h-11 max-sm:flex-1"
        >
          {t(merging ? "sync.review.merging" : "sync.review.merge")}
        </Button>
      </>
    );

  return (
    <>
      <span ref={anchor} hidden />
      <Dialog
        container={container}
        open={open}
        onOpenChange={(next) => !next && !merging && sync.close()}
        title={title}
        description={description}
        closeLabel={t("sync.review.cancel")}
        dismissible={!merging}
        footer={footer}
        contentFades={false}
        className="max-w-[480px]"
        testId="wb-sync-review"
      >
        {state.step === "problem" && (
          <p
            role="alert"
            data-testid="wb-sync-problem"
            data-problem={state.problem}
            className="flex items-start gap-3 text-[15px] leading-[22px] text-(--wb-ink-2)"
          >
            <TriangleAlert
              size={20}
              aria-hidden="true"
              className="mt-px shrink-0 text-(--wb-danger)"
            />
            {t(`sync.problems.${state.problem}`)}
          </p>
        )}
        {review && (
          <div className="flex flex-col gap-4">
            {done && (
              <p
                role="status"
                data-testid="wb-sync-result"
                className="flex items-center gap-2.5 text-[15px] leading-[22px] font-medium text-(--wb-ink)"
              >
                <CheckCircle2
                  size={20}
                  aria-hidden="true"
                  className="shrink-0 text-(--wb-success)"
                />
                {changesAnything(review.report)
                  ? t("sync.result.merged")
                  : t("sync.result.nothing")}
              </p>
            )}
            <ul
              className="overflow-hidden rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface-tint)"
              data-testid="wb-sync-parts"
            >
              {shown.map((part) => {
                const Icon = ICONS[part];
                const line = partLine(t, i18n.language, review, part);
                const labelId = `wb-sync-part-${part}`;
                return (
                  <li
                    key={part}
                    data-testid={`wb-sync-part-${part}`}
                    data-changes={line.changes || undefined}
                    className="flex min-h-[60px] items-center gap-3 border-t border-(--wb-line) px-4 py-2.5 first:border-t-0"
                  >
                    <Icon
                      size={20}
                      aria-hidden="true"
                      className="shrink-0 text-(--wb-ink-2)"
                    />
                    <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                      <span
                        id={labelId}
                        className="text-[15px] leading-5 font-medium text-(--wb-ink)"
                      >
                        {t(`sync.parts.${part}`)}
                      </span>
                      <span
                        className={`text-[13px] leading-[18px] ${
                          line.changes && (done || choices[part])
                            ? "text-(--wb-ink-2)"
                            : "text-(--wb-ink-3)"
                        }`}
                      >
                        {line.text}
                      </span>
                    </span>
                    {!done && line.changes && (
                      <Switch
                        checked={choices[part]}
                        disabled={merging}
                        onChange={(on) =>
                          setChoices((c) => ({ ...c, [part]: on }))
                        }
                        ariaLabelledBy={labelId}
                        testId={`wb-sync-switch-${part}`}
                      />
                    )}
                  </li>
                );
              })}
            </ul>
            {!done && !changesAnything(review.report) && (
              <p
                data-testid="wb-sync-up-to-date"
                className="text-[14px] leading-5 text-(--wb-ink-2)"
              >
                {t("sync.review.upToDate")}
              </p>
            )}
            <p className="text-[13px] leading-[18px] text-(--wb-ink-3)">
              {review.source === "mac_export"
                ? `${t("sync.review.privacyExport")}${
                    isIos() ? ` ${t("sync.review.privacyExportCopy")}` : ""
                  }`
                : t("sync.review.privacy")}
            </p>
          </div>
        )}
      </Dialog>
    </>
  );
}
