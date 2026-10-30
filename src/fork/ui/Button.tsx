// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hub button in the fork palette: `primary` (indigo fill) for the one clear
 * action of a page or empty state, `secondary` (outlined) for the rest,
 * `ghost` for toolbar icons.
 */
import { forwardRef, type ButtonHTMLAttributes } from "react";

export type ButtonVariant = "primary" | "secondary" | "ghost";

const VARIANTS: Record<ButtonVariant, string> = {
  primary:
    "bg-(--wb-accent-fill) text-(--wb-on-accent) hover:brightness-110 active:brightness-95",
  secondary:
    "border border-(--wb-line-strong) bg-(--wb-card) text-(--wb-ink) hover:bg-(--wb-hover)",
  ghost: "text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink)",
};

export const FOCUS_RING =
  "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--wb-accent)";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    { variant = "secondary", className = "", type = "button", ...rest },
    ref,
  ) {
    return (
      <button
        ref={ref}
        type={type}
        className={`inline-flex cursor-pointer items-center justify-center gap-2 rounded-(--wb-radius-control) px-3.5 py-2 text-[13px] font-medium transition-[background-color,filter] disabled:cursor-default disabled:opacity-50 ${FOCUS_RING} ${VARIANTS[variant]} ${className}`}
        {...rest}
      />
    );
  },
);
