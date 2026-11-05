// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The stats card on the home's right rail (S-15a, C-30, C-31, C-32): total
 * words, words per minute and the day streak, read from S-09's
 * `daily_stats` through `useStatsSummary` (live after every take).
 *
 * A description list, so a screen reader reads each number with its label;
 * the rail around it (`Home`) is the labelled landmark.
 * Words per minute needs 3 s of timed speech (S-09); until then, and while
 * the stats cannot be read, a dash is shown with the reason as text.
 */
import { useTranslation } from "react-i18next";
import type { StatsSummary } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import type { StatsStatus } from "./useStatsSummary";

export interface StatsRailProps {
  summary: StatsSummary | null;
  status: StatsStatus;
}

/** A number in the UI language's grouping ("12,345", "12.345", "12 345"). */
export function formatCount(value: number, language: string): string {
  try {
    return new Intl.NumberFormat(language, { maximumFractionDigits: 0 }).format(
      value,
    );
  } catch {
    return String(Math.round(value));
  }
}

function Stat({
  testId,
  label,
  value,
  missing,
}: {
  testId: string;
  label: string;
  value: string | null;
  /** Read instead of the dash when `value` is null. */
  missing: string;
}) {
  // The number reads first on screen ("12,840 Total words"), the label
  // first for screen readers (term, then definition).
  return (
    <div
      data-testid={testId}
      className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-0.5"
    >
      <dt className="order-2 min-w-0 truncate text-[15px] leading-5 text-(--wb-ink-2)">
        {label}
      </dt>
      <dd
        data-testid={`${testId}-value`}
        className="type-serif-title order-1 shrink-0 text-(--wb-ink) tabular-nums"
      >
        {value ?? (
          <>
            <span aria-hidden="true">–</span>
            <span className="sr-only">{missing}</span>
          </>
        )}
      </dd>
    </div>
  );
}

export function StatsRail({ summary, status }: StatsRailProps) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const language = i18n.language;
  const unavailable = t("home.stats.unavailable");
  const wpm = summary?.words_per_minute;

  return (
    <div
      data-testid="wb-stats-rail"
      data-status={status}
      aria-busy={status === "loading"}
      className="px-6 pt-6 pb-5"
    >
      <dl className="grid grid-cols-1 gap-3 @min-[520px]:grid-cols-3 @min-[680px]:grid-cols-1">
        <Stat
          testId="wb-stat-words"
          label={t("home.stats.totalWords")}
          value={summary ? formatCount(summary.total_words, language) : null}
          missing={unavailable}
        />
        <Stat
          testId="wb-stat-wpm"
          label={t("home.stats.wordsPerMinute")}
          value={
            summary && wpm != null && Number.isFinite(wpm)
              ? formatCount(wpm, language)
              : null
          }
          missing={summary ? t("home.stats.notEnoughSpeech") : unavailable}
        />
        <Stat
          testId="wb-stat-streak"
          label={t("home.stats.streak")}
          value={
            summary ? formatCount(summary.current_streak_days, language) : null
          }
          missing={unavailable}
        />
      </dl>
    </div>
  );
}
