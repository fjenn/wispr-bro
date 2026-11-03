// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Buttons (Design Phase 0, SYSTEM-PLAN §7 Phase 0.2).
 *
 * - `primary`: black ink fill, the one clear action of a screen.
 * - `secondary`: warm beige fill, every other action.
 * - `ghost`: no fill until hover, for toolbars and quiet actions.
 * - `destructive`: red text on beige, red fill on hover (Delete …).
 * - `cream`: on dark hero banners and dark celebration screens.
 * - `outline`: white with a hairline, the "white button on a tinted row".
 *
 * Sizes 32 / 36 / 40 (`sm` / `md` / `lg`); `lg` is the onboarding CTA.
 * Press is a 0.97 scale with a snappy spring back (motion presets; none under
 * Reduce Motion). `IconButton` is the square, label-required variant.
 */
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { motion, type HTMLMotionProps } from "motion/react";
import { useMotionPresets } from "./motion";

export type ButtonVariant =
  | "primary"
  | "secondary"
  | "ghost"
  | "destructive"
  | "cream"
  | "outline";
export type ButtonSize = "sm" | "md" | "lg";

const VARIANTS: Record<ButtonVariant, string> = {
  primary:
    "bg-(--wb-ink) text-(--wb-on-ink) hover:bg-(--wb-ink-hover) disabled:bg-(--wb-ink-disabled) disabled:text-(--wb-on-ink)",
  secondary:
    "bg-(--wb-secondary) text-(--wb-ink) hover:bg-(--wb-secondary-hover)",
  ghost:
    "text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) active:bg-(--wb-fill-press)",
  destructive:
    "bg-(--wb-secondary) text-(--wb-danger) hover:bg-(--wb-danger) hover:text-white",
  cream: "bg-(--wb-cream) text-(--wb-on-cream) hover:bg-white",
  outline:
    "border border-(--wb-line-2) bg-(--wb-surface) text-(--wb-ink) hover:bg-(--wb-surface-tint)",
};

const SIZES: Record<ButtonSize, string> = {
  sm: "h-8 gap-1.5 px-3 text-[13px]",
  md: "h-9 gap-2 px-4 text-[14px]",
  lg: "h-10 gap-2 px-5 text-[15px]",
};

/** The fork's focus ring: 2 px purple, offset, keyboard only. */
export const FOCUS_RING =
  "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--wb-accent)";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** A glyph before the label (from `./icons`). */
  leading?: ReactNode;
  /** A glyph or kbd after the label. */
  trailing?: ReactNode;
  /** Stretch to the container's width. */
  block?: boolean;
}

/** Shared classes, for the rare element that must look like a button. */
export function buttonClass(
  variant: ButtonVariant = "secondary",
  size: ButtonSize = "md",
  extra = "",
): string {
  return `inline-flex shrink-0 cursor-pointer items-center justify-center rounded-(--wb-radius-sm) font-medium whitespace-nowrap transition-[color,background-color,border-color,opacity] duration-(--wb-dur-fast) select-none disabled:cursor-default disabled:opacity-60 ${FOCUS_RING} ${SIZES[size]} ${VARIANTS[variant]} ${extra}`;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      variant = "secondary",
      size = "md",
      className = "",
      type = "button",
      leading,
      trailing,
      block = false,
      children,
      disabled,
      ...rest
    },
    ref,
  ) {
    const m = useMotionPresets();
    return (
      <motion.button
        ref={ref}
        type={type}
        disabled={disabled}
        {...(disabled ? {} : m.press)}
        className={buttonClass(
          variant,
          size,
          `${block ? "w-full" : ""} ${className}`,
        )}
        {...(rest as HTMLMotionProps<"button">)}
      >
        {leading}
        {children}
        {trailing}
      </motion.button>
    );
  },
);

export interface IconButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label"> {
  /** Required: the button has no visible text. */
  "aria-label": string;
  variant?: "ghost" | "secondary" | "primary" | "hud";
  size?: ButtonSize;
  /** Round instead of the 8 px square. */
  round?: boolean;
}

const ICON_SIZES: Record<ButtonSize, string> = {
  sm: "size-7",
  md: "size-8",
  lg: "size-9",
};

const ICON_VARIANTS: Record<NonNullable<IconButtonProps["variant"]>, string> = {
  ghost:
    "text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) active:bg-(--wb-fill-press)",
  secondary:
    "bg-(--wb-secondary) text-(--wb-ink) hover:bg-(--wb-secondary-hover)",
  primary: "bg-(--wb-ink) text-(--wb-on-ink) hover:bg-(--wb-ink-hover)",
  hud: "bg-(--wb-hud-control) text-(--wb-on-hud) hover:brightness-125",
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  function IconButton(
    {
      variant = "ghost",
      size = "md",
      round = false,
      className = "",
      type = "button",
      disabled,
      ...rest
    },
    ref,
  ) {
    const m = useMotionPresets();
    return (
      <motion.button
        ref={ref}
        type={type}
        disabled={disabled}
        {...(disabled ? {} : m.press)}
        className={`inline-grid shrink-0 cursor-pointer place-items-center transition-[color,background-color] duration-(--wb-dur-fast) disabled:cursor-default disabled:opacity-50 ${round ? "rounded-full" : "rounded-(--wb-radius-sm)"} ${ICON_SIZES[size]} ${ICON_VARIANTS[variant]} ${FOCUS_RING} ${className}`}
        {...(rest as HTMLMotionProps<"button">)}
      />
    );
  },
);
