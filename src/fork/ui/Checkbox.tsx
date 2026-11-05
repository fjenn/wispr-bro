// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Checkbox (Design Phase 0), only where several items are picked from a list
 * (import rows, multi-select). App settings use `Switch`. A native checkbox
 * restyled: 18 pt, 5 pt radius, ink fill with a white tick when checked.
 */
import { forwardRef, type InputHTMLAttributes, type ReactNode } from "react";
import { Check, Minus } from "./icons";

export interface CheckboxProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
  /** Visible label beside the box (optional; else name it with aria-*). */
  label?: ReactNode;
  /** The mixed state of a "select all" box. */
  indeterminate?: boolean;
}

export const Checkbox = forwardRef<HTMLInputElement, CheckboxProps>(
  function Checkbox(
    { label, indeterminate = false, className = "", checked, ...rest },
    ref,
  ) {
    const box = (
      <span className="relative inline-grid size-[18px] shrink-0 place-items-center">
        <input
          ref={(el) => {
            if (el) el.indeterminate = indeterminate;
            if (typeof ref === "function") ref(el);
            else if (ref) ref.current = el;
          }}
          type="checkbox"
          checked={checked}
          className="peer absolute inset-0 m-0 cursor-pointer appearance-none rounded-(--wb-radius-xs) border border-(--wb-line-strong) bg-(--wb-surface) transition-colors duration-(--wb-dur-fast) checked:border-(--wb-ink) checked:bg-(--wb-ink) indeterminate:border-(--wb-ink) indeterminate:bg-(--wb-ink) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--wb-accent) disabled:cursor-default disabled:opacity-50"
          {...rest}
        />
        <span className="pointer-events-none relative text-(--wb-on-ink) opacity-0 peer-checked:opacity-100 peer-indeterminate:opacity-100">
          {indeterminate ? <Minus size={14} /> : <Check size={14} />}
        </span>
      </span>
    );
    if (!label) return <span className={className}>{box}</span>;
    return (
      <label
        className={`inline-flex cursor-pointer items-center gap-2.5 text-[15px] text-(--wb-ink) ${className}`}
      >
        {box}
        <span>{label}</span>
      </label>
    );
  },
);
