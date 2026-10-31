// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The streak heatmap (S-25, C-55) on react-activity-calendar 3.2.1 (MIT,
 * R4 "Charts"; loaded lazily with the Usage tab). One cell per local day
 * for the last year, from `UsageInsights.heatmap` (S-09's `daily_stats`),
 * banded 0 / 1-249 / 250-499 / 500-749 / 750+ words by the backend.
 *
 * - Colours are the `--wb-heat-*` tokens (`insights.css`), one hue light to
 *   dark, with dark mode's own steps.
 * - Cell size follows the card's width (8-14 px), so the year fits without
 *   a scroll bar; a card too narrow for 53 weeks at 8 px shows the most
 *   recent weeks that fit.
 * - Month and weekday names come from `Intl` in the app language; the week
 *   starts on the locale's first day.
 * - The calendar is a picture for screen readers (`role="img"` with a
 *   summary); the numbers are also in the streak figures and the legend
 *   beside it, which are text. Each cell has a `<title>` for the pointer.
 * - Time runs left to right in every language (`dir="ltr"`), like a
 *   chart axis.
 */
import { cloneElement, useEffect, useMemo, useRef, useState } from "react";
import { ActivityCalendar, type DayIndex } from "react-activity-calendar";
import { useTranslation } from "react-i18next";
import type { HeatmapDay } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { formatDay } from "./format";
import { HEAT_COLORS } from "./heatColors";

const BLOCK_MARGIN = 3;
const WEEKS = 53;
/** Smallest cell, in px. */
const MIN_BLOCK = 8;
/** Room for the weekday labels on the left. */
const LABEL_WIDTH = 40;

/** First day of the week in `language` (0 = Sunday). */
export function weekStart(language: string): DayIndex {
  try {
    const locale = new Intl.Locale(language) as Intl.Locale & {
      getWeekInfo?: () => { firstDay: number };
      weekInfo?: { firstDay: number };
    };
    const info = locale.getWeekInfo?.() ?? locale.weekInfo;
    if (info) return (info.firstDay % 7) as DayIndex;
  } catch {
    // fall through
  }
  return 1;
}

function monthNames(language: string): string[] {
  const fmt = new Intl.DateTimeFormat(language, { month: "short" });
  return Array.from({ length: 12 }, (_, m) =>
    fmt.format(new Date(2026, m, 15)),
  );
}

function weekdayNames(language: string): string[] {
  const fmt = new Intl.DateTimeFormat(language, { weekday: "short" });
  // 2026-11-01 is a Sunday.
  return Array.from({ length: 7 }, (_, d) =>
    fmt.format(new Date(2026, 10, 1 + d)),
  );
}

function useIsDark(): boolean {
  const read = () => {
    const theme = document.documentElement.dataset.theme;
    if (theme === "dark") return true;
    if (theme === "light") return false;
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  };
  const [dark, setDark] = useState(read);
  useEffect(() => {
    const update = () => setDark(read());
    const mo = new MutationObserver(update);
    mo.observe(document.documentElement, { attributeFilter: ["data-theme"] });
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", update);
    return () => {
      mo.disconnect();
      mq.removeEventListener("change", update);
    };
  }, []);
  return dark;
}

export interface HeatmapProps {
  days: HeatmapDay[];
  /** Read by screen readers instead of the cells. */
  summary: string;
}

export default function Heatmap({ days, summary }: HeatmapProps) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const language = i18n.language;
  const dark = useIsDark();
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) =>
      setWidth(Math.floor(entry.contentRect.width)),
    );
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // A narrow card shows the most recent weeks that fit at the smallest
  // readable cell size instead of scrolling; a wide one shows the year.
  const room = Math.max(0, width - LABEL_WIDTH);
  const weeks = Math.max(
    4,
    Math.min(WEEKS, Math.floor(room / (MIN_BLOCK + BLOCK_MARGIN)) - 1),
  );
  const blockSize = Math.max(
    MIN_BLOCK,
    Math.min(14, Math.floor(room / (weeks + 1)) - BLOCK_MARGIN),
  );
  const labels = useMemo(
    () => ({ months: monthNames(language), weekdays: weekdayNames(language) }),
    [language],
  );
  const data = useMemo(
    () =>
      days
        .slice(-Math.min(days.length, weeks * 7))
        .map((d) => ({ date: d.date, count: d.words, level: d.level })),
    [days, weeks],
  );

  return (
    <div
      ref={box}
      dir="ltr"
      role="img"
      aria-label={summary}
      data-testid="wb-insights-heatmap"
      className="min-w-0 text-(--wb-ink-2)"
    >
      {data.length > 0 && width > 0 && (
        <ActivityCalendar
          data={data}
          blockSize={blockSize}
          blockMargin={BLOCK_MARGIN}
          blockRadius={2}
          fontSize={11}
          colorScheme={dark ? "dark" : "light"}
          theme={{ light: HEAT_COLORS, dark: HEAT_COLORS }}
          labels={labels}
          showWeekdayLabels={["mon", "wed", "fri"]}
          showColorLegend={false}
          showTotalCount={false}
          weekStart={weekStart(language)}
          renderBlock={(block, activity) =>
            cloneElement(
              block,
              { "data-level": activity.level } as Record<string, unknown>,
              <title>
                {t("insights.heatmap.day", {
                  count: activity.count,
                  date: formatDay(activity.date, language),
                })}
              </title>,
            )
          }
        />
      )}
    </div>
  );
}
