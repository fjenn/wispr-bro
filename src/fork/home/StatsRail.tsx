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
import { Flame, Gauge, Type, type LucideIcon } from "lucide-react";
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
  icon: Icon,
  label,
  value,
  missing,
}: {
  testId: string;
  icon: LucideIcon;
  label: string;
  value: string | null;
  /** Read instead of the dash when `value` is null. */
  missing: string;
}) {
  return (
    <div
      data-testid={testId}
      className="flex min-w-0 items-center gap-3 @min-[520px]:flex-col @min-[520px]:items-start @min-[680px]:flex-row @min-[680px]:items-center"
    >
      <dt className="flex min-w-0 items-center gap-2 text-[13px] text-(--wb-ink-2)">
        <Icon
          size={16}
          strokeWidth={1.8}
          aria-hidden="true"
          className="shrink-0"
        />
        <span className="truncate">{label}</span>
      </dt>
      <dd
        data-testid={`${testId}-value`}
        className="ms-auto shrink-0 @min-[520px]:ms-0 @min-[680px]:ms-auto font-(family-name:--wb-font-display) text-[22px] leading-none font-medium text-(--wb-ink) tabular-nums"
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
      className="rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) px-5 py-4"
    >
      <dl className="grid grid-cols-1 gap-4 @min-[520px]:grid-cols-3 @min-[680px]:grid-cols-1">
        <Stat
          testId="wb-stat-words"
          icon={Type}
          label={t("home.stats.totalWords")}
          value={summary ? formatCount(summary.total_words, language) : null}
          missing={unavailable}
        />
        <Stat
          testId="wb-stat-wpm"
          icon={Gauge}
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
          icon={Flame}
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
