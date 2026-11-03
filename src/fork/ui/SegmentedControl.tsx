// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Segmented control (Design Phase 0): two to five mutually exclusive
 * options in one beige track; the chosen one sits on a white pill that
 * slides between options. A radio group to assistive tech: arrow keys move
 * and select (with RTL), Home/End jump, Tab enters on the chosen option.
 */
import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";
import { motion } from "motion/react";
import { FOCUS_RING } from "./Button";
import { SPRING, useMotionPresets } from "./motion";

export interface SegmentOption<V extends string> {
  value: V;
  label: ReactNode;
  /** Accessible name when the label is only an icon. */
  ariaLabel?: string;
  disabled?: boolean;
}

export interface SegmentedControlProps<V extends string> {
  value: V;
  onChange: (value: V) => void;
  options: SegmentOption<V>[];
  /** Accessible name of the group. */
  label: string;
  size?: "sm" | "md";
  testId?: string;
  className?: string;
}

export function SegmentedControl<V extends string>({
  value,
  onChange,
  options,
  label,
  size = "md",
  testId,
  className = "",
}: SegmentedControlProps<V>) {
  const group = useId();
  const m = useMotionPresets();
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const enabled = options.filter((o) => !o.disabled);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const rtl =
      getComputedStyle(event.currentTarget).direction === "rtl" ? -1 : 1;
    const at = enabled.findIndex((o) => o.value === value);
    let next = -1;
    if (event.key === "ArrowRight" || event.key === "ArrowDown")
      next = at + (event.key === "ArrowDown" ? 1 : rtl);
    else if (event.key === "ArrowLeft" || event.key === "ArrowUp")
      next = at - (event.key === "ArrowUp" ? 1 : rtl);
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = enabled.length - 1;
    else return;
    event.preventDefault();
    const target = enabled[(next + enabled.length) % enabled.length];
    onChange(target.value);
    refs.current[options.indexOf(target)]?.focus();
  };

  const h = size === "md" ? "h-9 text-[14px]" : "h-8 text-[13px]";
  return (
    <div
      role="radiogroup"
      aria-label={label}
      data-testid={testId}
      onKeyDown={onKeyDown}
      className={`inline-flex rounded-(--wb-radius-sm) bg-(--wb-secondary) p-0.5 ${className}`}
    >
      {options.map((option, i) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={option.ariaLabel}
            disabled={option.disabled}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(option.value)}
            className={`relative inline-flex cursor-pointer items-center justify-center gap-1.5 rounded-[6px] px-3 font-medium transition-colors duration-(--wb-dur-fast) disabled:cursor-default disabled:opacity-50 ${h} ${selected ? "text-(--wb-ink)" : "text-(--wb-ink-2) hover:text-(--wb-ink)"} ${FOCUS_RING}`}
          >
            {selected && (
              <motion.span
                layoutId={`${group}-pill`}
                aria-hidden="true"
                className="absolute inset-0 rounded-[6px] bg-(--wb-surface) shadow-[0_1px_2px_rgba(0,0,0,0.08)]"
                transition={m.reduced ? { duration: 0 } : SPRING.snappy}
              />
            )}
            <span className="relative">{option.label}</span>
          </button>
        );
      })}
    </div>
  );
}
