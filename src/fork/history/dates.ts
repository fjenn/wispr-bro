// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * History's date groups (S-15b, C-35): rows fall into the local calendar
 * day of their record time, in the time zone the Mac is in now (a row
 * recorded at 23:59 belongs to that day, one at 00:01 to the next), and
 * the labels follow the app language.
 */
import { useEffect, useState } from "react";

/** The local day of `ms` as `YYYY-MM-DD` (sorts like the dates). */
export function dayKey(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** The local day before `key`. */
function previousDay(key: string): string {
  const [y, m, d] = key.split("-").map(Number);
  // Noon avoids a daylight-saving change moving the date.
  return dayKey(new Date(y, m - 1, d - 1, 12).getTime());
}

export type DayLabel =
  | { kind: "today" }
  | { kind: "yesterday" }
  | { kind: "date"; text: string };

/** How a day group is headed, seen from `now`. */
export function dayLabel(key: string, now: number, language: string): DayLabel {
  const today = dayKey(now);
  if (key === today) return { kind: "today" };
  if (key === previousDay(today)) return { kind: "yesterday" };
  const [y, m, d] = key.split("-").map(Number);
  const date = new Date(y, m - 1, d, 12);
  const sameYear = y === new Date(now).getFullYear();
  try {
    return {
      kind: "date",
      text: new Intl.DateTimeFormat(language, {
        weekday: "long",
        month: "long",
        day: "numeric",
        ...(sameYear ? {} : { year: "numeric" }),
      }).format(date),
    };
  } catch {
    return { kind: "date", text: key };
  }
}

/** A row's time of day ("9:41 AM", "09:41"). */
export function timeLabel(ms: number, language: string): string {
  try {
    return new Intl.DateTimeFormat(language, {
      hour: "numeric",
      minute: "2-digit",
    }).format(new Date(ms));
  } catch {
    return new Date(ms).toLocaleTimeString();
  }
}

/** Milliseconds from `now` to the next local midnight. */
export function untilMidnight(now: number): number {
  const d = new Date(now);
  const next = new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1);
  return Math.max(1_000, next.getTime() - now + 50);
}

/**
 * "Now" for the group labels, moved on just after each local midnight, so
 * "Today" becomes "Yesterday" without a reload.
 */
export function useToday(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setTimeout(() => setNow(Date.now()), untilMidnight(now));
    return () => clearTimeout(timer);
  }, [now]);
  return now;
}

export interface Grouped<T> {
  key: string;
  rows: T[];
}

/**
 * Rows in day groups, newest day first; rows keep their order inside a
 * group (Handy's, newest first). A row whose clock went backwards still
 * lands in its own day's group.
 */
export function groupByDay<T extends { created_at: number }>(
  rows: T[],
): Grouped<T>[] {
  const groups = new Map<string, T[]>();
  for (const row of rows) {
    const key = dayKey(row.created_at);
    const group = groups.get(key);
    if (group) group.push(row);
    else groups.set(key, [row]);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => (a < b ? 1 : a > b ? -1 : 0))
    .map(([key, grouped]) => ({ key, rows: grouped }));
}
