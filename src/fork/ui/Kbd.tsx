// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Key caps (Design Phase 0): `<Kbd>⌥</Kbd><Kbd>O</Kbd>`, or `<KbdCombo
 * keys={["⌥", "O"]} />`. 24 pt tall on a beige fill with a hairline; `dark`
 * is the black inline pill used inside lesson text and on the bar.
 */
import type { ReactNode } from "react";

export function Kbd({
  children,
  tone = "light",
  pressed = false,
  size = "md",
  className = "",
}: {
  children: ReactNode;
  tone?: "light" | "dark";
  /** `lg`: 30 pt caps on a transform card or the view-changes pill. */
  size?: "md" | "lg";
  /** A key held right now (the onboarding key test): purple. */
  pressed?: boolean;
  className?: string;
}) {
  const look = pressed
    ? "border-(--wb-accent) bg-(--wb-accent) text-(--wb-on-ink)"
    : tone === "dark"
      ? "border-transparent bg-(--wb-hud) text-(--wb-on-hud)"
      : "border-(--wb-line-2) bg-(--wb-surface-tint) text-(--wb-ink)";
  return (
    <kbd
      className={`inline-flex items-center justify-center rounded-(--wb-radius-xs) border font-(family-name:--wb-font-ui) leading-none font-medium transition-colors duration-(--wb-dur-fast) ${size === "lg" ? "h-[30px] min-w-[30px] px-2 text-[15px]" : "h-6 min-w-6 px-1.5 text-[13px]"} ${look} ${className}`}
    >
      {children}
    </kbd>
  );
}

export function KbdCombo({
  keys,
  tone,
  label,
  size,
  className = "",
}: {
  keys: string[];
  tone?: "light" | "dark";
  size?: "md" | "lg";
  /** Spoken name of the combination (e.g. "Option O"). */
  label?: string;
  className?: string;
}) {
  return (
    <span
      className={`inline-flex items-center gap-1 ${className}`}
      aria-label={label}
      role={label ? "img" : undefined}
    >
      {keys.map((key, i) => (
        <Kbd key={`${key}-${i}`} tone={tone} size={size}>
          {key}
        </Kbd>
      ))}
    </span>
  );
}
