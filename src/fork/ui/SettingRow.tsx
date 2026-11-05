// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A setting as a row card (Design Phase 0b; REFERENCE-SPEC §1 "Settings",
 * §4 "Settings row card"): a tinted card (warm tint, hairline, 8 pt radius,
 * 20 pt padding, 64 pt min height) with a title and one short description on
 * the start side and the control on the end side, vertically centred.
 *
 * The control is a slot. Pass a node, or a function that receives the ids
 * of the title and description so the control is named by them:
 *
 *   <SettingRow title={t(…)} description={t(…)}
 *     control={(a) => <Switch checked={on} onChange={set}
 *       ariaLabelledBy={a.labelledBy} ariaDescribedBy={a.describedBy} />} />
 *
 * `SettingGroup` stacks rows 12 pt apart under an optional micro label.
 * Copy rule (REFERENCE-SPEC §9): a description is at most ~12 words.
 */
import { useId, type ReactNode } from "react";
import { SectionLabel } from "./Card";

export interface SettingControlIds {
  labelledBy: string;
  describedBy: string | undefined;
}

export interface SettingRowProps {
  title: ReactNode;
  description?: ReactNode;
  control?: ReactNode | ((ids: SettingControlIds) => ReactNode);
  /** A glyph before the title (from `./icons`). */
  icon?: ReactNode;
  /** Extra content under the row's text (a warning line, a sub-control). */
  children?: ReactNode;
  /** `tint` (default, Settings) or `plain` (white, on a tinted page). */
  tone?: "tint" | "plain";
  disabled?: boolean;
  testId?: string;
  className?: string;
}

export function SettingRow({
  title,
  description,
  control,
  icon,
  children,
  tone = "tint",
  disabled = false,
  testId,
  className = "",
}: SettingRowProps) {
  const titleId = useId();
  const descId = useId();
  const ids: SettingControlIds = {
    labelledBy: titleId,
    describedBy: description ? descId : undefined,
  };
  return (
    <div
      data-testid={testId}
      data-disabled={disabled || undefined}
      className={`flex min-h-16 flex-col gap-3 rounded-(--wb-radius-sm) border border-(--wb-line) px-5 py-4 text-(--wb-ink) ${tone === "tint" ? "bg-(--wb-surface-tint)" : "bg-(--wb-surface)"} ${className}`}
    >
      <div className="flex items-center gap-4">
        {icon && (
          <span aria-hidden="true" className="shrink-0 text-(--wb-ink-2)">
            {icon}
          </span>
        )}
        <div
          className={`flex min-w-0 flex-1 flex-col gap-0.5 ${disabled ? "opacity-75" : ""}`}
        >
          <span id={titleId} className="text-[15px] leading-5 font-medium">
            {title}
          </span>
          {description && (
            <span
              id={descId}
              className="text-[14px] leading-5 text-(--wb-ink-2)"
            >
              {description}
            </span>
          )}
        </div>
        {control !== undefined && (
          <div className="flex shrink-0 items-center gap-2">
            {typeof control === "function" ? control(ids) : control}
          </div>
        )}
      </div>
      {children}
    </div>
  );
}

export function SettingGroup({
  label,
  children,
  className = "",
}: {
  label?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  const labelId = useId();
  return (
    <section
      aria-labelledby={label ? labelId : undefined}
      className={`flex flex-col gap-3 ${className}`}
    >
      {label && (
        <SectionLabel as="h3" id={labelId} className="mb-0.5">
          {label}
        </SectionLabel>
      )}
      {children}
    </section>
  );
}
