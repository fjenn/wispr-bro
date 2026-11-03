// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Popover on Radix Popover (R4 "UI primitives", MIT): a non-modal panel
 * anchored to its trigger, closed by Esc or a click outside, with focus moved
 * in and returned to the trigger. Used for the notifications panel (S-13
 * shell, S-18 content) and the greeting's name editor (S-15a). Uncontrolled
 * unless `open` is passed (then `onOpenChange` reports every change).
 */
import * as RadixPopover from "@radix-ui/react-popover";
import type { ReactElement, ReactNode } from "react";

export interface PopoverProps {
  trigger: ReactElement;
  children: ReactNode;
  /** Accessible name of the panel (it gets `role="dialog"`). */
  label: string;
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
  testId?: string;
  className?: string;
  /** Controlled open state (optional). */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function Popover({
  trigger,
  children,
  label,
  side = "bottom",
  align = "end",
  testId,
  className = "",
  open,
  onOpenChange,
}: PopoverProps) {
  return (
    <RadixPopover.Root open={open} onOpenChange={onOpenChange}>
      <RadixPopover.Trigger asChild>{trigger}</RadixPopover.Trigger>
      <RadixPopover.Portal>
        <RadixPopover.Content
          side={side}
          align={align}
          sideOffset={6}
          collisionPadding={8}
          aria-label={label}
          data-testid={testId}
          className={`wb-pop z-50 w-80 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) text-(--wb-ink) shadow-(--wb-elev-1) outline-none ${className}`}
        >
          {children}
        </RadixPopover.Content>
      </RadixPopover.Portal>
    </RadixPopover.Root>
  );
}
