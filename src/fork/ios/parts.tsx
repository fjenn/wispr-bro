// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Small iPhone building blocks on the shared tokens: the large page title,
 * grouped lists (the iOS Settings pattern: a tinted, rounded group of 52 pt
 * rows with hairlines between them) and their rows.
 */
import { useId, type ReactNode } from "react";
import { SectionLabel, Switch } from "../ui";
import { Check, ChevronRight, type Glyph } from "../ui/icons";

/** The page's large title (its only `h1`). */
export function IosTitle({ children }: { children: ReactNode }) {
  return (
    <h1 data-testid="wb-page-title" className="wb-ios-title text-(--wb-ink)">
      {children}
    </h1>
  );
}

export function IosGroup({
  label,
  footer,
  children,
  testId,
}: {
  label?: ReactNode;
  /** A short explanation under the group. */
  footer?: ReactNode;
  children: ReactNode;
  testId?: string;
}) {
  const labelId = useId();
  return (
    <section
      aria-labelledby={label ? labelId : undefined}
      data-testid={testId}
      className="flex flex-col gap-2"
    >
      {label && (
        <SectionLabel as="h2" id={labelId} className="px-4">
          {label}
        </SectionLabel>
      )}
      <div className="wb-ios-group">{children}</div>
      {footer && (
        <p className="px-4 text-[13px] leading-[18px] text-(--wb-ink-3)">
          {footer}
        </p>
      )}
    </section>
  );
}

function RowIcon({ icon: Icon }: { icon?: Glyph }) {
  if (!Icon) return null;
  return (
    <span
      aria-hidden="true"
      className="grid size-7 shrink-0 place-items-center text-(--wb-ink-2)"
    >
      <Icon size={20} />
    </span>
  );
}

/** A row that opens something: label, optional value, chevron. */
export function IosNavRow({
  icon,
  label,
  value,
  onClick,
  testId,
}: {
  icon?: Glyph;
  label: ReactNode;
  value?: ReactNode;
  onClick: () => void;
  testId?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={testId}
      className="wb-ios-row wb-ios-row-press"
    >
      <RowIcon icon={icon} />
      <span className="min-w-0 flex-1 truncate text-start">{label}</span>
      {value !== undefined && (
        <span className="max-w-[48%] shrink-0 truncate text-(--wb-ink-3)">
          {value}
        </span>
      )}
      <ChevronRight
        size={18}
        aria-hidden="true"
        className="-me-1 shrink-0 text-(--wb-ink-disabled) rtl:-scale-x-100"
      />
    </button>
  );
}

/** A row with a switch. */
export function IosSwitchRow({
  icon,
  label,
  hint,
  checked,
  disabled,
  onChange,
  testId,
}: {
  icon?: Glyph;
  label: ReactNode;
  hint?: ReactNode;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
  testId?: string;
}) {
  const labelId = useId();
  const hintId = useId();
  return (
    <div className="wb-ios-row" data-testid={testId && `${testId}-row`}>
      <RowIcon icon={icon} />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5 py-2.5">
        <span id={labelId}>{label}</span>
        {hint && (
          <span
            id={hintId}
            className="text-[13px] leading-[18px] text-(--wb-ink-3)"
          >
            {hint}
          </span>
        )}
      </span>
      <Switch
        checked={checked}
        disabled={disabled}
        onChange={onChange}
        ariaLabelledBy={labelId}
        ariaDescribedBy={hint ? hintId : undefined}
        testId={testId}
      />
    </div>
  );
}

/** A plain row: label and a value or control at the end. */
export function IosValueRow({
  icon,
  label,
  children,
  testId,
}: {
  icon?: Glyph;
  label: ReactNode;
  children?: ReactNode;
  testId?: string;
}) {
  return (
    <div className="wb-ios-row" data-testid={testId}>
      <RowIcon icon={icon} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <span className="flex shrink-0 items-center gap-2 text-(--wb-ink-3)">
        {children}
      </span>
    </div>
  );
}

/** One choice of a single-choice list: the chosen row carries a check. */
export function IosChoiceRow({
  label,
  hint,
  checked,
  disabled,
  onSelect,
  testId,
}: {
  label: ReactNode;
  hint?: ReactNode;
  checked: boolean;
  disabled?: boolean;
  onSelect: () => void;
  testId?: string;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={checked}
      disabled={disabled}
      onClick={onSelect}
      data-testid={testId}
      className="wb-ios-row wb-ios-row-press disabled:opacity-50"
    >
      <span className="flex min-w-0 flex-1 flex-col gap-0.5 py-2.5 text-start">
        <span>{label}</span>
        {hint && (
          <span className="text-[13px] leading-[18px] text-(--wb-ink-3)">
            {hint}
          </span>
        )}
      </span>
      <Check
        size={20}
        aria-hidden="true"
        className={`shrink-0 text-(--wb-accent) ${checked ? "" : "invisible"}`}
      />
    </button>
  );
}
