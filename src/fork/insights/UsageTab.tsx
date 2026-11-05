// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Insights › Usage (S-25, C-51, C-52, C-53, C-54, C-55): the time saved
 * over typing as the page's main figure (with the typing test to take or
 * retake), then speaking speed against typing speed, fixes made and total
 * words, where the words go and the streak heatmap. Laid out after the
 * reference: a hero card, three stat cards (rolling number over a micro
 * label, a hairline, the detail), then two
 * section cards with a large heading and a "LABEL | value" aside. Every number comes from `fork_insights_usage`
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
import { Button, StatNumber } from "../ui";
import { Keyboard } from "../ui/icons";
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
  titleTestId,
}: {
  testId: string;
  title: string;
  /** A test id on the title (when the title is itself a figure). */
  titleTestId?: string;
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
          <h2 id={id} className={SECTION_HEADING} data-testid={titleTestId}>
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
  "text-[32px] leading-[38px] font-medium tracking-[-0.015em] text-(--wb-ink) tabular-nums";

/** A hairline, then the card's detail lines. */
const DETAIL = "border-t border-(--wb-line) pt-4";

/** A section card's large heading (the reference's "N day streak"). */
export const SECTION_HEADING =
  "text-[28px] leading-[34px] font-medium tracking-[-0.01em] text-(--wb-ink)";

/**
 * A stat card's big number: it rolls to its value (`StatNumber`); screen
 * readers and tests read the plain, formatted text beside it.
 */
function Figure({
  value,
  text,
  testId,
}: {
  value: number;
  text: string;
  testId?: string;
}) {
  return (
    <p className={BIG}>
      <span aria-hidden="true">
        <StatNumber value={value} />
      </span>
      <span className="sr-only" data-testid={testId}>
        {text}
      </span>
    </p>
  );
}

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

type SavedUnit = "minute" | "hour" | "day";

/**
 * The time saved in friendly units: minutes under 2 hours, hours under
 * 2 days, then days and hours. Rounded to whole units; never negative.
 */
export function savedParts(ms: number): { value: number; unit: SavedUnit }[] {
  const minutes = Math.max(0, Math.round(ms / 60_000));
  if (minutes < 120) return [{ value: minutes, unit: "minute" }];
  const hours = Math.round(minutes / 60);
  if (hours < 48) return [{ value: hours, unit: "hour" }];
  const days = Math.floor(hours / 24);
  const rest = hours % 24;
  return [
    { value: days, unit: "day" },
    ...(rest > 0 ? [{ value: rest, unit: "hour" as const }] : []),
  ];
}

/**
 * `value unit` in the UI language, split around the number so the number
 * can roll (`StatNumber`) while the unit words (from `Intl`, so nothing to
 * translate, plurals included) sit before or after it as the language
 * puts them.
 */
function unitText(
  value: number,
  unit: SavedUnit,
  language: string,
): { before: string; after: string; text: string } {
  try {
    const parts = new Intl.NumberFormat(language, {
      style: "unit",
      unit,
      unitDisplay: "long",
    }).formatToParts(value);
    const numeric = new Set(["integer", "group", "decimal", "fraction"]);
    const first = parts.findIndex((p) => numeric.has(p.type));
    let last = first;
    while (last + 1 < parts.length && numeric.has(parts[last + 1].type))
      last += 1;
    const join = (ps: Intl.NumberFormatPart[]) =>
      ps
        .map((p) => p.value)
        .join("")
        .trim();
    return {
      before: first < 0 ? "" : join(parts.slice(0, first)),
      after: first < 0 ? "" : join(parts.slice(last + 1)),
      text: parts.map((p) => p.value).join(""),
    };
  } catch {
    return { before: "", after: unit, text: `${value} ${unit}` };
  }
}

/** The whole time saved as one localized phrase ("3 days, 4 hours"). */
export function formatSaved(ms: number, language: string): string {
  const texts = savedParts(ms).map(
    (p) => unitText(p.value, p.unit, language).text,
  );
  try {
    return new Intl.ListFormat(language, {
      type: "unit",
      style: "long",
    }).format(texts);
  } catch {
    return texts.join(" ");
  }
}

/**
 * The page's main figure (owner's call): every dictated word typed at the
 * typing speed (the test, or the average typist) against spoken at the
 * user's own speaking speed (or the 200 wpm estimate until there is one),
 * as `fork::stats::insights::time_saved_ms` computes it.
 */
function SavedHero({
  usage,
  onTypingTest,
}: {
  usage: UsageInsights;
  onTypingTest: () => void;
}) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const language = i18n.language;
  const { time_saved_ms, speaking_wpm, speaking_source, typing_wpm } =
    usage.speed;
  const measured = usage.speed.typing_source === "measured";
  const estimate = speaking_source === "estimate";
  const basis = `insights.saved.basis${estimate ? "Estimate" : ""}${measured ? "Measured" : "Average"}`;
  const id = "wb-insights-saved-title";
  return (
    <section
      aria-labelledby={id}
      data-testid="wb-insights-saved"
      data-ms={time_saved_ms}
      className={`${CARD} flex min-w-0 flex-col gap-3 p-8`}
    >
      <div className="flex flex-col gap-2">
        <h2 id={id} className="type-micro order-2 text-(--wb-ink-2)">
          {t("insights.saved.title")}
        </h2>
        <p
          className="type-hero order-1 flex flex-wrap items-baseline gap-x-5 gap-y-1 text-[64px] leading-[68px] text-(--wb-ink) tabular-nums @min-[600px]:text-[96px] @min-[600px]:leading-[96px]"
          data-testid="wb-insights-saved-figure"
        >
          <span aria-hidden="true" className="contents">
            {savedParts(time_saved_ms).map(({ value, unit }) => {
              const u = unitText(value, unit, language);
              const word = "type-sheet text-(--wb-ink-2)";
              return (
                <span
                  key={unit}
                  data-unit={unit}
                  className="inline-flex items-baseline gap-2"
                >
                  {u.before && <span className={word}>{u.before}</span>}
                  <StatNumber value={value} />
                  {u.after && <span className={word}>{u.after}</span>}
                </span>
              );
            })}
          </span>
          <span className="sr-only" data-testid="wb-insights-saved-value">
            {formatSaved(time_saved_ms, language)}
          </span>
        </p>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
        <p
          data-testid="wb-insights-saved-basis"
          data-speaking={speaking_source}
          className="text-[13px] leading-[18px] text-(--wb-ink-2)"
        >
          {t(basis, {
            speaking: formatCount(Math.round(speaking_wpm), language),
            typing: formatCount(typing_wpm, language),
          })}
        </p>
        <div data-export-skip="">
          <Button
            size="sm"
            variant="outline"
            onClick={onTypingTest}
            data-testid="wb-insights-typing-test"
            leading={<Keyboard size={16} aria-hidden="true" />}
          >
            {t(measured ? "insights.typing.retake" : "insights.typing.take")}
          </Button>
        </div>
      </div>
    </section>
  );
}

/** One labelled bar of the speaking-against-typing comparison. */
function SpeedBar({
  id,
  label,
  wpm,
  max,
  strong,
}: {
  id: string;
  label: string;
  wpm: number;
  max: number;
  strong: boolean;
}) {
  const { i18n } = useTranslation(FORK_NAMESPACE);
  const width = max > 0 ? Math.max(6, (wpm / max) * 100) : 0;
  return (
    <div className="flex flex-col gap-1" data-testid={`wb-insights-bar-${id}`}>
      <div className="flex items-baseline justify-between gap-3">
        <span className="type-micro text-(--wb-ink-2)">{label}</span>
        <span className="text-[13px] leading-[18px] font-medium text-(--wb-ink) tabular-nums">
          {formatCount(wpm, i18n.language)}
        </span>
      </div>
      <div
        aria-hidden="true"
        className="h-2 overflow-hidden rounded-full bg-(--wb-data-empty)"
      >
        <div
          className={`h-full rounded-full ${strong ? "bg-(--wb-data-1)" : "bg-(--wb-data-3)"}`}
          style={{ width: `${width}%` }}
        />
      </div>
    </div>
  );
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
  const measured = typing_source === "measured";
  const rounded = wpm != null ? Math.round(wpm) : null;
  const max = Math.max(rounded ?? 0, typing_wpm);
  return (
    <Card
      testId="wb-insights-speed"
      title={t("insights.speed.title")}
      figure={
        rounded != null ? (
          <Figure
            value={rounded}
            text={formatCount(rounded, language)}
            testId="wb-insights-speed-value"
          />
        ) : (
          <p aria-hidden="true" className={`${BIG} text-(--wb-ink-3)`}>
            –
          </p>
        )
      }
    >
      <div className={`${DETAIL} flex flex-1 flex-col gap-3`}>
        {rounded != null ? (
          <>
            <div className="flex flex-col gap-2.5">
              <SpeedBar
                id="speaking"
                label={t("insights.speed.speaking")}
                wpm={rounded}
                max={max}
                strong
              />
              <SpeedBar
                id="typing"
                label={t(
                  measured
                    ? "insights.speed.typing"
                    : "insights.speed.typingAverage",
                )}
                wpm={typing_wpm}
                max={max}
                strong={false}
              />
            </div>
            {times_typing != null && (
              <p
                data-testid="wb-insights-speed-compare"
                data-source={typing_source}
                className="text-[13px] leading-[18px] text-(--wb-ink-2)"
              >
                {t(`insights.speed.${typing_source}`, {
                  ratio: oneDecimal(times_typing, language),
                  typing: formatCount(typing_wpm, language),
                })}
              </p>
            )}
          </>
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
        <Figure
          value={usage.total_words}
          text={formatCount(usage.total_words, i18n.language)}
          testId="wb-insights-words-value"
        />
      }
    >
      <div
        className={`${DETAIL} flex flex-col gap-1.5 text-[15px] leading-[22px] text-(--wb-ink)`}
      >
        <p>
          {t("insights.words.dictations", { count: usage.total_dictations })}
        </p>
      </div>
    </Card>
  );
}

function FixesCard({ usage }: { usage: UsageInsights }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const rows = [
    ["dictionary", usage.fixes_dictionary],
    ["ai", usage.fixes_llm],
  ] as const;
  const total = usage.fixes_dictionary + usage.fixes_llm;
  return (
    <Card
      testId="wb-insights-fixes"
      title={t("insights.fixes.title")}
      figure={<Figure value={total} text={formatCount(total, i18n.language)} />}
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

/** "LABEL | value", the reference's micro aside next to a section heading. */
function Aside({
  label,
  value,
  testId,
}: {
  label: string;
  value: string;
  testId?: string;
}) {
  return (
    <p className="type-micro flex items-center gap-2 text-(--wb-ink-2)">
      <span>{label}</span>
      <span aria-hidden="true" className="h-3 w-px bg-(--wb-line-strong)" />
      <span className="text-(--wb-ink)" data-testid={testId}>
        {value}
      </span>
    </p>
  );
}

function ChartFallback({ height }: { height: number }) {
  return <div aria-hidden="true" style={{ height }} />;
}

function CategoriesCard({ usage }: { usage: UsageInsights }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const any = usage.categories.some((c) => c.words > 0);
  const used = usage.categories.filter((c) => c.words > 0).length;
  return (
    <Card
      testId="wb-insights-categories"
      title={t("insights.categories.title")}
      aside={
        <Aside
          label={t("insights.categories.used")}
          value={formatCount(used, i18n.language)}
          testId="wb-insights-categories-used"
        />
      }
    >
      {any ? (
        <Suspense fallback={<ChartFallback height={272} />}>
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

/** The heatmap days that belong to the current streak (outlined). */
export function streakDates(usage: UsageInsights): Set<string> {
  const days = usage.heatmap;
  const out = new Set<string>();
  let i = days.length - 1;
  // A streak still counts while today has no words yet.
  if (i >= 0 && days[i].words === 0) i -= 1;
  for (let n = 0; n < usage.current_streak_days && i >= 0; n++, i--) {
    out.add(days[i].date);
  }
  return out;
}

function StreakCard({ usage }: { usage: UsageInsights }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const days = (n: number) => t("insights.heatmap.days", { count: n });
  const active = t("insights.heatmap.active", {
    count: usage.heatmap_active_days,
  });
  const summary = `${active}. ${t("insights.heatmap.current")}: ${days(usage.current_streak_days)}. ${t("insights.heatmap.longest")}: ${days(usage.longest_streak_days)}.`;
  const current = streakDates(usage);
  return (
    <Card
      testId="wb-insights-streak"
      title={t("insights.heatmap.streak", {
        count: usage.current_streak_days,
      })}
      titleTestId="wb-insights-streak-current"
      aside={
        <Aside
          label={t("insights.heatmap.longest")}
          value={days(usage.longest_streak_days)}
          testId="wb-insights-streak-longest"
        />
      }
    >
      <Suspense fallback={<ChartFallback height={130} />}>
        <Heatmap days={usage.heatmap} summary={summary} streak={current} />
      </Suspense>
      <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 text-[13px] text-(--wb-ink-2)">
        <div className="flex items-center gap-2">
          <span aria-hidden="true">{t("insights.heatmap.less")}</span>
          <ul
            aria-label={t("insights.heatmap.title")}
            data-testid="wb-insights-legend"
            className="flex items-center gap-1"
          >
            {HEAT_COLORS.map((color, level) => (
              <li
                key={level}
                className="flex items-center"
                title={t(`insights.heatmap.band${level}`)}
              >
                <span
                  aria-hidden="true"
                  className="inline-block size-3 rounded-[3px]"
                  style={{ background: color }}
                />
                <span className="sr-only">
                  {t(`insights.heatmap.band${level}`)}
                </span>
              </li>
            ))}
          </ul>
          <span aria-hidden="true">{t("insights.heatmap.more")}</span>
        </div>
        <div className="flex flex-wrap items-center gap-x-5 gap-y-1">
          <p data-testid="wb-insights-active">{active}</p>
          {current.size > 0 && (
            <p className="flex items-center gap-1.5">
              <span
                aria-hidden="true"
                className="inline-block size-3 rounded-[3px] border-[1.5px] border-(--wb-ink)"
              />
              {t("insights.heatmap.current")}
            </p>
          )}
        </div>
      </div>
    </Card>
  );
}

export function UsageTab({
  usage,
  onTypingTest,
}: {
  usage: UsageInsights;
  /** Opens the typing test (Retake / Take the typing test). */
  onTypingTest: () => void;
}) {
  return (
    <div
      data-testid="wb-insights-usage"
      className="@container flex flex-col gap-6"
    >
      <SavedHero usage={usage} onTypingTest={onTypingTest} />
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
