// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A number that rolls to its new value (Design Phase 0) on
 * `@number-flow/react` (MIT): stats, hours saved, words per minute. Digits
 * roll vertically on change; under Reduce Motion the value just changes.
 * Formatting follows the UI language (`Intl.NumberFormat`); figures are
 * tabular. Put the serif display style on it with `className`
 * (`type-serif-title`, `type-display`).
 */
import NumberFlow, { type Format } from "@number-flow/react";
import { useTranslation } from "react-i18next";
import { useMotionPresets } from "./motion";

export function StatNumber({
  value,
  format,
  className = "",
  testId,
}: {
  value: number;
  format?: Format;
  className?: string;
  testId?: string;
}) {
  const { i18n } = useTranslation();
  const m = useMotionPresets();
  return (
    <NumberFlow
      data-testid={testId}
      value={value}
      locales={i18n.language}
      format={format}
      animated={!m.reduced}
      className={`tabular-nums ${className}`}
    />
  );
}
