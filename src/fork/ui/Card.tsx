// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Cards and lists (Design Phase 0). The reference is almost flat: cards are
 * separated by a 1 px warm hairline and a 1-3 % tint step, not shadows.
 *
 * - `Card`: `plain` (white, hairline), `tint` (stat cards, setting rows),
 *   `raised` (onboarding test cards: white with a soft shadow), `selected`
 *   adds the 1.5 pt purple border of a chosen option.
 * - `ListContainer` + `ListRow`: one bordered white container with 1 px row
 *   dividers (History, Dictionary, Snippets, Recents). A row's `actions`
 *   fade in on hover and on keyboard focus inside the row.
 * - `SectionLabel`: the uppercase micro label above a group (TODAY).
 */
import { forwardRef, type HTMLAttributes, type ReactNode } from "react";

export type CardTone = "plain" | "tint" | "raised";

const CARD_TONES: Record<CardTone, string> = {
  plain: "border border-(--wb-line) bg-(--wb-surface)",
  tint: "border border-(--wb-line) bg-(--wb-surface-tint)",
  raised: "bg-(--wb-surface) shadow-(--wb-elev-2)",
};

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  tone?: CardTone;
  selected?: boolean;
  /** Padding: `md` 24 pt (default), `sm` 16 pt, `none`. */
  padding?: "none" | "sm" | "md";
}

export const Card = forwardRef<HTMLDivElement, CardProps>(function Card(
  { tone = "plain", selected = false, padding = "md", className = "", ...rest },
  ref,
) {
  const pad = padding === "md" ? "p-6" : padding === "sm" ? "p-4" : "";
  return (
    <div
      ref={ref}
      data-selected={selected || undefined}
      className={`rounded-(--wb-radius-md) text-(--wb-ink) ${selected ? "border border-transparent bg-(--wb-accent-soft) outline-[1.5px] outline-(--wb-accent-2) outline-solid -outline-offset-1" : CARD_TONES[tone]} ${pad} ${className}`}
      {...rest}
    />
  );
});

export interface ListContainerProps extends HTMLAttributes<HTMLElement> {
  /** `ul` (default) for lists of rows; `div` when rows are not list items. */
  as?: "ul" | "div";
}

export function ListContainer({
  as = "ul",
  className = "",
  children,
  ...rest
}: ListContainerProps) {
  const Tag = as;
  return (
    <Tag
      className={`divide-y divide-(--wb-line) overflow-hidden rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) ${className}`}
      {...rest}
    >
      {children}
    </Tag>
  );
}

export interface ListRowProps extends HTMLAttributes<HTMLElement> {
  as?: "li" | "div";
  /** Leading column (a time, an icon); aligned to the first text line. */
  leading?: ReactNode;
  /** Trailing actions; revealed on hover/focus unless `actionsVisible`. */
  actions?: ReactNode;
  actionsVisible?: boolean;
  /** Hover tint (rows that open something). */
  interactive?: boolean;
}

export function ListRow({
  as = "li",
  leading,
  actions,
  actionsVisible = false,
  interactive = true,
  className = "",
  children,
  ...rest
}: ListRowProps) {
  const Tag = as;
  return (
    <Tag
      className={`group/row flex min-h-(--wb-row-h) items-start gap-4 px-4 py-3.5 text-[15px] leading-5 transition-colors duration-(--wb-dur-fast) ${interactive ? "hover:bg-(--wb-fill-hover)" : ""} ${className}`}
      {...rest}
    >
      {leading !== undefined && (
        <div className="w-20 shrink-0 text-[14px] text-(--wb-ink-3) tabular-nums">
          {leading}
        </div>
      )}
      <div className="min-w-0 flex-1">{children}</div>
      {actions && (
        <div
          className={`-my-1 flex shrink-0 items-center gap-0.5 transition-opacity duration-(--wb-dur-fast) ${actionsVisible ? "" : "opacity-0 group-focus-within/row:opacity-100 group-hover/row:opacity-100"}`}
        >
          {actions}
        </div>
      )}
    </Tag>
  );
}

export function SectionLabel({
  children,
  className = "",
  as = "h2",
  id,
}: {
  children: ReactNode;
  className?: string;
  as?: "h2" | "h3" | "p";
  id?: string;
}) {
  const Tag = as;
  return (
    <Tag id={id} className={`type-micro text-(--wb-ink-3) ${className}`}>
      {children}
    </Tag>
  );
}
