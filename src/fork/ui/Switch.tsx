// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * On/off switch (Design Phase 0): 40 × 24 pt, black track when on, a white
 * knob that slides with the base ease-out (220 ms; instant under Reduce
 * Motion). A native checkbox with
 * `role="switch"` underneath, so Space toggles it, forms and labels work,
 * and VoiceOver says "switch, on/off". Use it for every app setting; keep
 * `Checkbox` for picking several items from a list.
 *
 * The props match Handy's `Switch` (`checked`, `onChange`, `ariaLabel`,
 * `ariaLabelledBy`) so fork code moved over unchanged.
 */
import { useId } from "react";

export interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  ariaLabel?: string;
  ariaLabelledBy?: string;
  ariaDescribedBy?: string;
  id?: string;
  testId?: string;
  size?: "md" | "sm";
  className?: string;
}

export function Switch({
  checked,
  onChange,
  disabled = false,
  ariaLabel,
  ariaLabelledBy,
  ariaDescribedBy,
  id,
  testId,
  size = "md",
  className = "",
}: SwitchProps) {
  const fallbackId = useId();
  const track = size === "md" ? "h-6 w-10" : "h-5 w-8";
  const knob = size === "md" ? "size-5" : "size-4";
  const shift =
    size === "md"
      ? "translate-x-4 rtl:-translate-x-4"
      : "translate-x-3 rtl:-translate-x-3";
  return (
    <span
      className={`relative inline-flex shrink-0 ${track} ${disabled ? "opacity-50" : ""} ${className}`}
    >
      <input
        id={id ?? fallbackId}
        type="checkbox"
        role="switch"
        data-testid={testId}
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        aria-describedby={ariaDescribedBy}
        aria-checked={checked}
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="peer absolute inset-0 z-10 m-0 cursor-pointer appearance-none rounded-full disabled:cursor-default"
      />
      <span
        aria-hidden="true"
        className={`absolute inset-0 rounded-full transition-colors duration-(--wb-dur-fast) peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-(--wb-accent) ${checked ? "bg-(--wb-ink)" : "bg-(--wb-line-strong)"}`}
      />
      <span
        aria-hidden="true"
        className={`pointer-events-none absolute top-0.5 start-0.5 ${knob} rounded-full shadow-[0_1px_2px_rgba(0,0,0,0.25)] transition-transform duration-(--wb-dur-base) ease-(--wb-ease-out) ${checked ? `bg-(--wb-on-ink) ${shift}` : "bg-white"}`}
      />
    </span>
  );
}
