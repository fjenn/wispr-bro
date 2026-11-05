// SPDX-License-Identifier: GPL-3.0-or-later
/** Date formatting shared by the Insights page and its lazy chunks. */

/** A `YYYY-MM-DD` day as a long local date. */
export function formatDay(day: string, language: string): string {
  const [y, m, d] = day.split("-").map(Number);
  try {
    return new Intl.DateTimeFormat(language, { dateStyle: "medium" }).format(
      new Date(y, m - 1, d),
    );
  } catch {
    return day;
  }
}
