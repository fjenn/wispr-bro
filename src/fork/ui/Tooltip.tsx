// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Tooltip (Design Phase 0) on Radix Tooltip (MIT): the black HUD pill of the
 * reference (15 px white text, full radius), shown on hover after a short
 * delay and on keyboard focus, hidden on Esc; the trigger gets
 * `aria-describedby`. Wrap a window's root in `TooltipProvider` once
 * (`ForkApp` does); a tooltip outside one still works with its own.
 *
 * Tooltips only repeat a control's name (icon buttons, a collapsed
 * sidebar); never put information only in a tooltip.
 */
import * as RadixTooltip from "@radix-ui/react-tooltip";
import type { ReactElement, ReactNode } from "react";
import { usePinnedThemeProps } from "./ThemeScope";

export function TooltipProvider({ children }: { children: ReactNode }) {
  return (
    <RadixTooltip.Provider delayDuration={500} skipDelayDuration={300}>
      {children}
    </RadixTooltip.Provider>
  );
}

export interface TooltipProps {
  /** The tooltip's text. */
  text: ReactNode;
  /** One focusable element (rendered through `asChild`). */
  children: ReactElement;
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
  /** Keep the tooltip off (e.g. when the label is visible anyway). */
  disabled?: boolean;
  /** Force it open (the gallery, tests). */
  open?: boolean;
}

export function Tooltip({
  text,
  children,
  side = "bottom",
  align = "center",
  disabled = false,
  open,
}: TooltipProps) {
  const pinned = usePinnedThemeProps();
  if (disabled) return children;
  return (
    <RadixTooltip.Provider delayDuration={500} skipDelayDuration={300}>
      <RadixTooltip.Root open={open}>
        <RadixTooltip.Trigger asChild>{children}</RadixTooltip.Trigger>
        <RadixTooltip.Portal>
          <RadixTooltip.Content
            {...pinned}
            side={side}
            align={align}
            sideOffset={8}
            collisionPadding={8}
            className="wb-pop z-[60] max-w-64 rounded-full bg-(--wb-hud) px-3 py-1.5 text-[13px] leading-[18px] font-medium text-(--wb-on-hud) shadow-(--wb-elev-1)"
          >
            {text}
          </RadixTooltip.Content>
        </RadixTooltip.Portal>
      </RadixTooltip.Root>
    </RadixTooltip.Provider>
  );
}
