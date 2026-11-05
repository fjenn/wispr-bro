// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Toasts (Design Phase 0b; GAP-AUDIT: the reference's toasts are dark cards
 * with an action).
 *
 * Handy and the fork both raise toasts through sonner (`toast.success(…)`,
 * 13 Handy files plus the fork pages), and Handy's `App.tsx` renders the one
 * `<Toaster unstyled>`. Replacing sonner would mean editing those files, so
 * the toast engine stays and its look is restyled through the tokens instead:
 * `styles/tokens.css` ("Toasts") paints every sonner toast as the HUD card
 * below, in both themes. Unlayered CSS outranks the Tailwind classes Handy
 * passes, so no Handy file changes.
 *
 * - `toast`: sonner's API, re-exported so fork code imports it from `../ui`.
 *   `toast.success(title, { description, action: { label, onClick } })`.
 * - `ToastView`: the same card as a static element, for the gallery and for
 *   an in-place confirmation that is not a floating toast.
 *
 * Toasts stay at Handy's position (bottom right; sonner's `position` is a
 * prop of Handy's `<Toaster>`), announce politely (sonner's live region) and
 * honour Reduce Motion through the global rule.
 */
import type { ReactNode } from "react";
import { toast } from "sonner";
import { CheckCircle2, Info, TriangleAlert } from "./icons";

export { toast };

export type ToastTone = "neutral" | "success" | "error" | "info";

export interface ToastViewProps {
  title: ReactNode;
  description?: ReactNode;
  tone?: ToastTone;
  action?: { label: string; onClick: () => void };
  testId?: string;
  className?: string;
}

const ICONS = {
  success: CheckCircle2,
  error: TriangleAlert,
  info: Info,
} as const;

export function ToastView({
  title,
  description,
  tone = "neutral",
  action,
  testId,
  className = "",
}: ToastViewProps) {
  const G = tone === "neutral" ? null : ICONS[tone];
  return (
    <div
      role="status"
      data-testid={testId}
      data-type={tone}
      className={`wb-toast ${className}`}
    >
      {G && (
        <div data-icon="">
          <G size={18} />
        </div>
      )}
      <div data-content="">
        <div data-title="">{title}</div>
        {description && <div data-description="">{description}</div>}
      </div>
      {action && (
        <button type="button" data-button="" onClick={action.onClick}>
          {action.label}
        </button>
      )}
    </div>
  );
}
