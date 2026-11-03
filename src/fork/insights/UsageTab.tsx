// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Insights › Usage (S-25, C-51, C-52, C-53, C-54, C-55): speaking speed
 * against typing speed, fixes made, total words, where the words go and
 * the streak heatmap. Every number comes from `fork_insights_usage`
 * unchanged (S-09's stats store), so the page matches `daily_stats` by
 * construction. No percentile: there is no population data, by design.
 *
 * The chart and the heatmap are lazy chunks (Recharts and
 * react-activity-calendar); the figures around them are plain text.
 */
import { lazy, Suspense, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { UsageInsights } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { formatCount } from "../home/StatsRail";
import { HEAT_COLORS } from "./heatColors";

const CategoryChart = lazy(() => import("./CategoryChart"));
const Heatmap = lazy(() => import("./Heatmap"));

export const CARD =
  "rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface-tint) p-6";

/**
 * An Insights card (the reference's tinted card): either a stat card, whose
 * big figure reads first and its title sits under it as a micro label
 * (`figure`), or a section card with a sans heading (`heading`). The title is
 * the card's `h2` in both, first in the DOM for screen readers.
 */
export function Card({
  testId,
  title,
  figure,
  aside,
  small = false,
  children,
  className = "",
}: {
  testId: string;
  title: string;
  /** The big number, shown above the title. */
  figure?: ReactNode;
  /** Right of a section heading (e.g. the longest streak). */
  aside?: ReactNode;
  /** The title as a micro label at the top (the Voice cards). */
  small?: boolean;
  children?: ReactNode;
  className?: string;
}) {
  const id = `${testId}-title`;
  return (
    <section
      aria-labelledby={id}
      data-testid={testId}
      className={`${CARD} flex min-w-0 flex-col gap-4 ${className}`}
    >
      {figure !== undefined ? (
        <div className="flex flex-col gap-1.5">
          <h2 id={id} className="type-micro order-2 text-(--wb-ink-2)">
            {title}
          </h2>
          <div className="order-1">{figure}</div>
        </div>
      ) : small ? (
        <h2 id={id} className="type-micro text-(--wb-ink-2)">
          {title}
        </h2>
      ) : (
        <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
          <h2 id={id} className="type-title text-(--wb-ink)">
            {title}
          </h2>
          {aside}
        </div>
      )}
      {children}
    </section>
  );
}

const BIG =
  "text-[28px] leading-[34px] font-semibold tracking-[-0.01em] text-(--wb-ink) tabular-nums";

/** A hairline, then the card's detail lines. */
const DETAIL = "border-t border-(--wb-line) pt-4";

function oneDecimal(value: number, language: string): string {
  try {
    return new Intl.NumberFormat(language, {
      minimumFractionDigits: 1,
      maximumFractionDigits: 1,
    }).format(value);
  } catch {
    return value.toFixed(1);
  }
}

function SpeedCard({ usage }: { usage: UsageInsights }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const language = i18n.language;
  const {
    words_per_minute: wpm,
    times_typing,
    typing_wpm,
    typing_source,
  } = usage.speed;
  return (
    <Card
      testId="wb-insights-speed"
      title={t("insights.speed.title")}
      figure={
        wpm != null ? (
          <p data-testid="wb-insights-speed-value" className={BIG}>
            {t("insights.speed.value", { wpm: formatCount(wpm, language) })}
          </p>
        ) : (
          <p aria-hidden="true" className={`${BIG} text-(--wb-ink-3)`}>
            –
          </p>
        )
      }
    >
      <div className={DETAIL}>
        {wpm != null ? (
          times_typing != null && (
            <p
              data-testid="wb-insights-speed-compare"
              data-source={typing_source}
              className="text-[15px] leading-[22px] text-(--wb-ink)"
            >
              {t(`insights.speed.${typing_source}`, {
                ratio: oneDecimal(times_typing, language),
                typing: formatCount(typing_wpm, language),
              })}
            </p>
          )
        ) : (
          <p
            data-testid="wb-insights-speed-value"
            className="text-[15px] leading-[22px] text-(--wb-ink-2)"
          >
            {t("insights.speed.notEnough")}
          </p>
        )}
      </div>
    </Card>
  );
}

function WordsCard({ usage }: { usage: UsageInsights }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  return (
    <Card
      testId="wb-insights-words"
      title={t("insights.words.title")}
      figure={
        <p data-testid="wb-insights-words-value" className={BIG}>
          {formatCount(usage.total_words, i18n.language)}
        </p>
      }
    >
      <p className={`${DETAIL} text-[15px] leading-[22px] text-(--wb-ink)`}>
        {t("insights.words.dictations", { count: usage.total_dictations })}
      </p>
    </Card>
  );
}

function FixesCard({ usage }: { usage: UsageInsights }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const rows = [
    ["dictionary", usage.fixes_dictionary],
    ["ai", usage.fixes_llm],
  ] as const;
  return (
    <Card
      testId="wb-insights-fixes"
      title={t("insights.fixes.title")}
      figure={
        <p aria-hidden="true" className={BIG}>
          {formatCount(usage.fixes_dictionary + usage.fixes_llm, i18n.language)}
        </p>
      }
    >
      <dl className={`${DETAIL} flex flex-col gap-1.5`}>
        {rows.map(([id, value]) => (
          <div key={id} className="flex items-baseline gap-2">
            <dt className="order-2 text-[15px] leading-[22px] text-(--wb-ink-2)">
              {t(`insights.fixes.${id}`)}
            </dt>
            <dd
              data-testid={`wb-insights-fixes-${id}`}
              className="order-1 text-[15px] leading-[22px] font-medium text-(--wb-ink) tabular-nums"
            >
              {formatCount(value, i18n.language)}
            </dd>
          </div>
        ))}
      </dl>
    </Card>
  );
}

function ChartFallback({ height }: { height: number }) {
  return <div aria-hidden="true" style={{ height }} />;
}

function CategoriesCard({ usage }: { usage: UsageInsights }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const any = usage.categories.some((c) => c.words > 0);
  return (
    <Card
      testId="wb-insights-categories"
      title={t("insights.categories.title")}
    >
      {any ? (
        <Suspense fallback={<ChartFallback height={212} />}>
          <CategoryChart categories={usage.categories} />
        </Suspense>
      ) : (
        <p className="text-[15px] leading-[22px] text-(--wb-ink-2)">
          {t("insights.categories.empty")}
        </p>
      )}
    </Card>
  );
}

function StreakCard({ usage }: { usage: UsageInsights }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const days = (n: number) => t("insights.heatmap.days", { count: n });
  const active = t("insights.heatmap.active", {
    count: usage.heatmap_active_days,
  });
  const summary = `${active}. ${t("insights.heatmap.current")}: ${days(usage.current_streak_days)}. ${t("insights.heatmap.longest")}: ${days(usage.longest_streak_days)}.`;
  return (
    <Card
      testId="wb-insights-streak"
      title={t("insights.heatmap.title")}
      aside={
        <p
          className="type-micro text-(--wb-ink-2)"
          data-testid="wb-insights-active"
        >
          {active}
        </p>
      }
    >
      <dl className="flex flex-wrap gap-x-10 gap-y-2">
        {(
          [
            ["current", usage.current_streak_days],
            ["longest", usage.longest_streak_days],
          ] as const
        ).map(([id, n]) => (
          <div key={id} className="flex flex-col gap-0.5">
            <dt className="type-micro order-2 text-(--wb-ink-2)">
              {t(`insights.heatmap.${id}`)}
            </dt>
            <dd
              data-testid={`wb-insights-streak-${id}`}
              className="type-serif-title order-1 text-(--wb-ink) tabular-nums"
            >
              {days(n)}
            </dd>
          </div>
        ))}
      </dl>
      <Suspense fallback={<ChartFallback height={130} />}>
        <Heatmap days={usage.heatmap} summary={summary} />
      </Suspense>
      <ul
        aria-label={t("insights.heatmap.title")}
        data-testid="wb-insights-legend"
        className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[13px] text-(--wb-ink-2)"
      >
        {HEAT_COLORS.map((color, level) => (
          <li key={level} className="flex items-center gap-1.5">
            <span
              aria-hidden="true"
              className="inline-block size-3 rounded-[3px]"
              style={{ background: color }}
            />
            {t(`insights.heatmap.band${level}`)}
          </li>
        ))}
      </ul>
    </Card>
  );
}

export function UsageTab({ usage }: { usage: UsageInsights }) {
  return (
    <div
      data-testid="wb-insights-usage"
      className="@container flex flex-col gap-6"
    >
      <div className="grid grid-cols-1 gap-6 @min-[600px]:grid-cols-3">
        <SpeedCard usage={usage} />
        <FixesCard usage={usage} />
        <WordsCard usage={usage} />
      </div>
      <div className="grid grid-cols-1 gap-6 @min-[880px]:grid-cols-2">
        <CategoriesCard usage={usage} />
        <StreakCard usage={usage} />
      </div>
    </div>
  );
}
