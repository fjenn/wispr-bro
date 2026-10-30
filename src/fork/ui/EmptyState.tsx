// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Empty state with one clear action (S-13, C-24, C-206): an icon in a soft
 * disc, a serif heading, one or two lines of text and a single primary
 * button. Pages pass their own words; the action is optional only for places
 * where there is nothing to do (an empty notifications list).
 */
import type { LucideIcon } from "lucide-react";
import { Button } from "./Button";

export interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  body: string;
  action?: { label: string; onClick: () => void };
  /** Smaller type and spacing, for popovers. */
  compact?: boolean;
  testId?: string;
}

export function EmptyState({
  icon: Icon,
  title,
  body,
  action,
  compact = false,
  testId,
}: EmptyStateProps) {
  return (
    <div
      data-testid={testId}
      className={`flex flex-col items-center text-center ${compact ? "gap-2 px-4 py-6" : "gap-3 px-6 py-14"}`}
    >
      <div
        aria-hidden="true"
        className={`grid place-items-center rounded-full bg-(--wb-selected) text-(--wb-ink-2) ${compact ? "size-10" : "size-14"}`}
      >
        <Icon size={compact ? 20 : 26} strokeWidth={1.6} />
      </div>
      <h2
        className={`font-(family-name:--wb-font-display) font-medium text-(--wb-ink) ${compact ? "text-[17px]" : "text-[24px] leading-tight"}`}
      >
        {title}
      </h2>
      <p
        className={`max-w-sm text-(--wb-ink-2) ${compact ? "text-[12.5px]" : "text-[14px] leading-5"}`}
      >
        {body}
      </p>
      {action && (
        <Button variant="primary" className="mt-2" onClick={action.onClick}>
          {action.label}
        </Button>
      )}
    </div>
  );
}
