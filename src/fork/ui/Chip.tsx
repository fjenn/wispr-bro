// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Option chips (Design Phase 0b; REFERENCE-SPEC §4 "Chip (choice)").
 *
 * - `Chip`: a toggle button (`aria-pressed`), 36 pt tall (`sm` 28), 8 pt
 *   radius, a warm hairline at rest; selected = lilac fill and a purple
 *   edge, with a tick. For picking several things from a short set
 *   (languages, apps, topics). For one-of-many use `RadioGroup` or
 *   `OptionCard`.
 * - `Chip` with `onRemove`: a removable chip (a picked app, a site, a
 *   filter). The label is static text; the ✕ is its own button and needs
 *   `removeLabel` ("Remove Safari"). Backspace or Delete on the ✕ removes.
 * - `ChipGroup`: wraps chips in a named `group` with an even gap.
 *
 * Press is the shared 0.97 scale (none under Reduce Motion); colour changes
 * are the 140 ms hover transition.
 */
import {
  forwardRef,
  type ButtonHTMLAttributes,
  type HTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { motion, type HTMLMotionProps } from "motion/react";
import { FOCUS_RING } from "./Button";
import { Check, X } from "./icons";
import { useMotionPresets } from "./motion";

export type ChipSize = "sm" | "md";

const SIZES: Record<ChipSize, string> = {
  sm: "h-7 gap-1.5 px-2.5 text-[13px]",
  md: "h-9 gap-2 px-3 text-[14px]",
};

const REST =
  "border-(--wb-line-2) bg-(--wb-surface) text-(--wb-ink) hover:border-(--wb-line-strong) hover:bg-(--wb-surface-tint)";
const SELECTED =
  "border-(--wb-accent-2) bg-(--wb-accent-soft) text-(--wb-ink) shadow-[inset_0_0_0_0.5px_var(--wb-accent-2)]";

function chipClass(size: ChipSize, selected: boolean, extra = ""): string {
  return `inline-flex shrink-0 items-center rounded-(--wb-radius-sm) border font-medium whitespace-nowrap transition-[color,background-color,border-color] duration-(--wb-dur-fast) select-none ${SIZES[size]} ${selected ? SELECTED : REST} ${extra}`;
}

interface ChipBase {
  size?: ChipSize;
  /** A glyph before the label (from `./icons`), or a flag/emoji. */
  leading?: ReactNode;
  children: ReactNode;
  testId?: string;
}

export interface ToggleChipProps
  extends ChipBase,
    Omit<
      ButtonHTMLAttributes<HTMLButtonElement>,
      "children" | "onChange" | "type"
    > {
  selected: boolean;
  onSelectedChange?: (selected: boolean) => void;
  onRemove?: undefined;
  removeLabel?: undefined;
}

export interface RemovableChipProps extends ChipBase {
  onRemove: () => void;
  /** Accessible name of the ✕ ("Remove Safari"). */
  removeLabel: string;
  selected?: boolean;
  disabled?: boolean;
  className?: string;
}

export type ChipProps = ToggleChipProps | RemovableChipProps;

export const Chip = forwardRef<HTMLButtonElement, ChipProps>(
  function Chip(props, ref) {
    const m = useMotionPresets();
    const { size = "md", leading, children, testId } = props;

    if (props.onRemove) {
      const {
        onRemove,
        removeLabel,
        selected = false,
        disabled = false,
        className = "",
      } = props;
      const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
        if (event.key === "Backspace" || event.key === "Delete") {
          event.preventDefault();
          onRemove();
        }
      };
      return (
        <span
          data-testid={testId}
          className={chipClass(
            size,
            selected,
            `pe-1 ${disabled ? "opacity-50" : ""} ${className}`,
          )}
        >
          {leading}
          <span>{children}</span>
          <motion.button
            ref={ref}
            type="button"
            aria-label={removeLabel}
            disabled={disabled}
            onClick={onRemove}
            onKeyDown={onKeyDown}
            {...(disabled ? {} : m.press)}
            className={`inline-grid size-6 cursor-pointer place-items-center rounded-(--wb-radius-xs) text-(--wb-ink-3) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-fill-press) hover:text-(--wb-ink) disabled:cursor-default ${FOCUS_RING}`}
          >
            <X size={14} />
          </motion.button>
        </span>
      );
    }

    const {
      selected,
      onSelectedChange,
      onClick,
      className = "",
      disabled,
      size: _size,
      leading: _leading,
      children: _children,
      testId: _testId,
      onRemove: _onRemove,
      removeLabel: _removeLabel,
      ...rest
    } = props;
    return (
      <motion.button
        ref={ref}
        type="button"
        aria-pressed={selected}
        data-selected={selected || undefined}
        data-testid={testId}
        disabled={disabled}
        onClick={(event) => {
          onClick?.(event);
          if (!event.defaultPrevented) onSelectedChange?.(!selected);
        }}
        {...(disabled ? {} : m.press)}
        className={chipClass(
          size,
          selected,
          `cursor-pointer disabled:cursor-default disabled:opacity-50 ${FOCUS_RING} ${className}`,
        )}
        {...(rest as HTMLMotionProps<"button">)}
      >
        {selected ? (
          <Check size={size === "md" ? 16 : 14} className="-ms-0.5 shrink-0" />
        ) : (
          leading
        )}
        {children}
      </motion.button>
    );
  },
);

export interface ChipGroupProps extends HTMLAttributes<HTMLDivElement> {
  /** Accessible name of the set ("Languages"). */
  label?: string;
  labelledBy?: string;
}

export function ChipGroup({
  label,
  labelledBy,
  className = "",
  children,
  ...rest
}: ChipGroupProps) {
  return (
    <div
      role="group"
      aria-label={label}
      aria-labelledby={labelledBy}
      className={`flex flex-wrap gap-2 ${className}`}
      {...rest}
    >
      {children}
    </div>
  );
}
