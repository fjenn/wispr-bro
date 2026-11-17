// SPDX-License-Identifier: GPL-3.0-or-later
import type { TFunction } from "i18next";

/**
 * "End session after" as words. `label` is the option's name ("5
 * minutes", "Immediately", "Never"); `sentence` fits "Ends … " lines
 * ("after 5 minutes without dictation").
 */
export function sessionTimeoutLabel(
  t: TFunction<"fork">,
  seconds: number,
  form: "label" | "sentence" = "label",
): string {
  const base = `ios.session.timeout.${form}`;
  if (seconds < 0) return t(`${base}.never`);
  if (seconds === 0) return t(`${base}.immediately`);
  if (seconds % 3600 === 0) {
    return t(`${base}.hours`, { count: seconds / 3600 });
  }
  return t(`${base}.minutes`, { count: Math.max(1, Math.round(seconds / 60)) });
}
