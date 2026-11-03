// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings rows (Design W3, SYSTEM-PLAN §7; REFERENCE-SPEC §1 "Settings").
 * Every setting is a tinted row card from the shared `SettingRow`: a title,
 * one short description (≤ 12 words) and the control on the end side.
 *
 * - `ToggleRow`: an on/off setting with the shared `Switch`.
 * - `SelectRow`: a choice with the shared Radix `Select`.
 * - `HandyRow`: hosts one of Handy's own setting components (Handy's files
 *   are not edited, R5 §4.2) inside the same row card; `settings.css`
 *   retunes Handy's controls through its colour tokens.
 * - `PanelCard`: the same card around a fork panel of several lines.
 * - `ForkToggle`: a compact switch with its label, for pages outside
 *   Settings (Dictionary, onboarding's summary).
 */
import { useId, type ReactNode } from "react";
import { SettingRow, Select, Switch } from "../ui";

export function ToggleRow({
  label,
  hint,
  checked,
  disabled,
  onChange,
  testId,
  children,
}: {
  label: string;
  hint?: ReactNode;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
  testId: string;
  children?: ReactNode;
}) {
  return (
    <SettingRow
      title={label}
      description={hint}
      testId={`${testId}-row`}
      control={(ids) => (
        <Switch
          checked={checked}
          disabled={disabled}
          onChange={onChange}
          ariaLabelledBy={ids.labelledBy}
          ariaDescribedBy={ids.describedBy}
          testId={testId}
        />
      )}
    >
      {children}
    </SettingRow>
  );
}

export function SelectRow({
  label,
  hint,
  value,
  options,
  disabled,
  onChange,
  testId,
  width = "w-56",
}: {
  label: string;
  hint?: ReactNode;
  value: string;
  options: { value: string; label: string }[];
  disabled?: boolean;
  onChange: (value: string) => void;
  testId: string;
  /** Width of the select (a Tailwind width class). */
  width?: string;
}) {
  return (
    <SettingRow
      title={label}
      description={hint}
      testId={`${testId}-row`}
      control={(ids) => (
        <Select
          value={value}
          options={options}
          disabled={disabled}
          onValueChange={onChange}
          labelledBy={ids.labelledBy}
          describedBy={ids.describedBy}
          testId={testId}
          className={width}
        />
      )}
    />
  );
}

/** One of Handy's setting components, in a Settings row card. */
export function HandyRow({
  children,
  testId,
}: {
  children: ReactNode;
  testId?: string;
}) {
  return (
    <div
      data-handy-content
      className="wb-handy wb-handy-row"
      data-testid={testId}
    >
      {children}
    </div>
  );
}

/** A tinted card for a fork panel that is more than one row (models, the
 * AI engine, the microphone list). */
export function PanelCard({
  children,
  testId,
}: {
  children: ReactNode;
  testId?: string;
}) {
  return (
    <div
      className="rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-5 py-4 text-(--wb-ink) empty:hidden"
      data-testid={testId}
    >
      {children}
    </div>
  );
}

export function ForkToggle({
  label,
  hint,
  checked,
  disabled,
  onChange,
  testId,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
  testId: string;
}) {
  const id = useId();
  const hintId = useId();
  return (
    <div className="flex flex-col gap-1 py-1.5">
      <div className="flex items-center gap-3">
        <Switch
          id={id}
          checked={checked}
          disabled={disabled}
          ariaDescribedBy={hint ? hintId : undefined}
          onChange={onChange}
          testId={testId}
        />
        <label
          htmlFor={id}
          className={`cursor-pointer text-[14px] leading-5 ${disabled ? "opacity-75" : ""}`}
        >
          {label}
        </label>
      </div>
      {hint && (
        <p
          id={hintId}
          className="ms-[52px] text-[13px] leading-[18px] text-(--wb-ink-2)"
        >
          {hint}
        </p>
      )}
    </div>
  );
}
