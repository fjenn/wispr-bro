// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Small status badges (Design Phase 0): counts beside a tab, "Recommended",
 * "Best match", a warning. For the "New!" / "Beta" tags beside a page name,
 * use `Tag`, which also remembers when a "new" tag was seen.
 */
import type { ReactNode } from "react";

export type BadgeTone =
  | "neutral"
  | "ink"
  | "accent"
  | "success"
  | "warning"
  | "danger";

const TONES: Record<BadgeTone, string> = {
  neutral: "bg-(--wb-secondary) text-(--wb-ink-2)",
  ink: "bg-(--wb-tag-beta-bg) text-(--wb-tag-beta-ink)",
  accent: "bg-(--wb-accent-soft) text-(--wb-accent)",
  success:
    "bg-(--wb-surface-tint) text-(--wb-ink) ring-1 ring-(--wb-success) ring-inset",
  warning: "bg-(--wb-warning-bg) text-(--wb-warning-ink)",
  danger: "bg-(--wb-danger-soft) text-(--wb-danger)",
};

export function Badge({
  children,
  tone = "neutral",
  className = "",
  testId,
}: {
  children: ReactNode;
  tone?: BadgeTone;
  className?: string;
  testId?: string;
}) {
  return (
    <span
      data-testid={testId}
      className={`inline-flex h-5 shrink-0 items-center gap-1 rounded-full px-2 text-[12px] leading-none font-medium tabular-nums ${TONES[tone]} ${className}`}
    >
      {children}
    </span>
  );
}
