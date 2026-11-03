// SPDX-License-Identifier: GPL-3.0-or-later
import { useId, type ReactNode } from "react";
import { FOCUS_RING } from "../ui";

export function Panel({
  title,
  children,
  testId,
}: {
  title: string;
  children: ReactNode;
  testId?: string;
}) {
  const id = useId();
  return (
    <section
      aria-labelledby={id}
      className="flex flex-col gap-3"
      data-testid={testId}
    >
      <h3
        id={id}
        className="text-xs font-semibold tracking-wide text-(--wb-ink-3) uppercase"
      >
        {title}
      </h3>
      <div className="rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-4">
        {children}
      </div>
    </section>
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
      <label
        htmlFor={id}
        className="flex cursor-pointer items-center gap-2 text-sm"
      >
        <input
          id={id}
          type="checkbox"
          checked={checked}
          disabled={disabled}
          aria-describedby={hint ? hintId : undefined}
          onChange={(event) => onChange(event.currentTarget.checked)}
          data-testid={testId}
        />
        {label}
      </label>
      {hint && (
        <p id={hintId} className="ms-6 text-xs text-(--wb-ink-3)">
          {hint}
        </p>
      )}
    </div>
  );
}

export function ForkSelect({
  label,
  hint,
  value,
  options,
  disabled,
  onChange,
  testId,
}: {
  label: string;
  hint?: string;
  value: string;
  options: { value: string; label: string }[];
  disabled?: boolean;
  onChange: (value: string) => void;
  testId: string;
}) {
  const id = useId();
  const hintId = useId();
  return (
    <div className="flex flex-col gap-1 py-1.5">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      {hint && (
        <p id={hintId} className="text-xs text-(--wb-ink-3)">
          {hint}
        </p>
      )}
      <select
        id={id}
        value={value}
        disabled={disabled}
        aria-describedby={hint ? hintId : undefined}
        onChange={(event) => onChange(event.currentTarget.value)}
        className={`mt-1 max-w-md rounded-md border border-(--wb-line-strong) bg-(--wb-surface) px-2 py-1.5 text-sm ${FOCUS_RING}`}
        data-testid={testId}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}
