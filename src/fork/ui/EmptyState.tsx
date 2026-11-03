// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Empty state with one clear action (S-13, C-24, C-206; Design Phase 0):
 * an illustration (the `art` slot; W7 supplies the art) or, until a page has
 * one, a quiet 32 pt glyph; one serif line, one short sentence and a single
 * black button. Pages pass their own words; the action is optional only for
 * places where there is nothing to do (an empty notifications list).
 * `compact` is for popovers: sans title, no serif.
 */
import type { ReactNode } from "react";
import { Button } from "./Button";
import type { Glyph } from "./icons";

export interface EmptyStateProps {
  /** A glyph shown when there is no `art`. */
  icon?: Glyph;
  /** Decorative illustration (hidden from screen readers). */
  art?: ReactNode;
  title: string;
  body: string;
  action?: { label: string; onClick: () => void };
  compact?: boolean;
  testId?: string;
}

export function EmptyState({
  icon: Icon,
  art,
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
      {art ? (
        <div aria-hidden="true" className="mb-2">
          {art}
        </div>
      ) : (
        Icon && (
          <div aria-hidden="true" className="mb-1 text-(--wb-ink-3)">
            <Icon size={compact ? 24 : 32} />
          </div>
        )
      )}
      <h2
        className={
          compact
            ? "type-card-title text-(--wb-ink)"
            : "type-serif-title text-(--wb-ink)"
        }
      >
        {title}
      </h2>
      <p
        className={`max-w-sm text-(--wb-ink-2) ${compact ? "text-[13px] leading-[18px]" : "text-[15px] leading-[22px]"}`}
      >
        {body}
      </p>
      {action && (
        <Button variant="primary" className="mt-3" onClick={action.onClick}>
          {action.label}
        </Button>
      )}
    </div>
  );
}
