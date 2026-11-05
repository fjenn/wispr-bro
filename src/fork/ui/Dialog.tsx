// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Dialog and Sheet (Design Phase 0). Both are modal: focus moves in and is
 * trapped (Tab cycles inside), Esc and a click on the scrim close them (both
 * can be turned off), the page behind does not scroll, and focus returns to
 * where it was on close. They spring in and fade out through the motion
 * presets (a crossfade under Reduce Motion).
 *
 * `Dialog` keeps the API of Handy's `src/components/ui/Dialog` (which the
 * fork used before), so every fork dialog moved over unchanged: title,
 * optional description, body, footer (secondary left, primary right by
 * convention), a 28 pt close button. `Sheet` is the large lesson/chooser
 * surface: back chevron, centred progress dashes, close, then the body.
 *
 * The focus-trap logic is ported from Handy's Dialog (MIT, © 2024 CJ Pais;
 * modified for the fork: motion, tokens, the Sheet layout).
 */
import {
  useEffect,
  useId,
  useRef,
  type CSSProperties,
  type MouseEvent,
  type ReactNode,
  type Ref,
  type RefObject,
} from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { IconButton } from "./Button";
import { ArrowLeft, X } from "./icons";
import { useMotionPresets } from "./motion";
import { ProgressDashes } from "./Progress";
import { usePinnedThemeProps } from "./ThemeScope";

const FOCUSABLE = [
  "a[href]",
  "button:not([disabled])",
  "textarea:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

function focusables(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => {
      if (el.hasAttribute("disabled")) return false;
      if (el.getAttribute("aria-hidden") === "true") return false;
      const style = window.getComputedStyle(el);
      return style.visibility !== "hidden" && style.display !== "none";
    },
  );
}

interface ModalBehaviour {
  open: boolean;
  dismissible: boolean;
  onOpenChange: (open: boolean) => void;
  initialFocusRef?: RefObject<HTMLElement | null>;
  panelRef: RefObject<HTMLDivElement | null>;
}

/** Focus in, trap, Esc, scroll lock and focus return (Handy's logic). */
function useModal({
  open,
  dismissible,
  onOpenChange,
  initialFocusRef,
  panelRef,
}: ModalBehaviour) {
  useEffect(() => {
    if (!open) return;
    const previous =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const overflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const frame = requestAnimationFrame(() => {
      (initialFocusRef?.current ?? panelRef.current)?.focus();
    });
    return () => {
      cancelAnimationFrame(frame);
      document.body.style.overflow = overflow;
      previous?.focus();
    };
  }, [initialFocusRef, open, panelRef]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && dismissible) {
        event.preventDefault();
        onOpenChange(false);
        return;
      }
      const panel = panelRef.current;
      if (event.key !== "Tab" || !panel) return;
      const items = focusables(panel);
      if (items.length === 0) {
        event.preventDefault();
        panel.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (active === panel) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      } else if (event.shiftKey && active === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [dismissible, onOpenChange, open, panelRef]);
}

const FADE_MASK: CSSProperties = {
  maskImage:
    "linear-gradient(to bottom, transparent 0, black 10px, black calc(100% - 20px), transparent 100%)",
  WebkitMaskImage:
    "linear-gradient(to bottom, transparent 0, black 10px, black calc(100% - 20px), transparent 100%)",
};

export interface DialogProps {
  open: boolean;
  title: ReactNode;
  children: ReactNode;
  onOpenChange: (open: boolean) => void;
  description?: ReactNode;
  footer?: ReactNode;
  closeLabel: string;
  dismissible?: boolean;
  closeOnBackdrop?: boolean;
  showCloseButton?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  /** The close button (e.g. to give it the first focus, W6). */
  closeRef?: Ref<HTMLButtonElement>;
  closeTestId?: string;
  /** Extra classes on the panel (e.g. a `max-w-*` override). */
  className?: string;
  contentClassName?: string;
  /** Fade the body's top and bottom edges while it scrolls. */
  contentFades?: boolean;
  testId?: string;
  /**
   * Where the dialog portals (default `<body>`). Inside a native modal
   * `<dialog>` (Settings) everything else is inert, so a dialog opened from
   * there portals into it.
   */
  container?: HTMLElement | null;
}

export function Dialog({
  open,
  title,
  children,
  onOpenChange,
  description,
  footer,
  closeLabel,
  dismissible = true,
  closeOnBackdrop = true,
  showCloseButton = true,
  initialFocusRef,
  closeRef,
  closeTestId,
  className = "",
  contentClassName = "",
  contentFades = true,
  testId,
  container,
}: DialogProps) {
  const titleId = useId();
  const descriptionId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const m = useMotionPresets();
  const pinned = usePinnedThemeProps();
  useModal({ open, dismissible, onOpenChange, initialFocusRef, panelRef });

  const onScrimDown = (event: MouseEvent<HTMLDivElement>) => {
    if (dismissible && closeOnBackdrop && event.target === event.currentTarget)
      onOpenChange(false);
  };

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          key="wb-dialog"
          {...m.dialog.scrim}
          {...pinned}
          className="fixed inset-0 z-50 flex items-center justify-center bg-(--wb-scrim) p-4 sm:p-6"
          onMouseDown={onScrimDown}
        >
          <motion.div
            {...m.dialog.panel}
            ref={panelRef}
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            aria-describedby={description ? descriptionId : undefined}
            tabIndex={-1}
            data-testid={testId}
            className={`flex max-h-[calc(100dvh-2rem)] w-full max-w-[560px] flex-col overflow-hidden rounded-(--wb-radius-lg) bg-(--wb-surface) text-(--wb-ink) shadow-(--wb-elev-3) outline-none sm:max-h-[calc(100dvh-3rem)] ${className}`}
          >
            <div className="flex shrink-0 items-start justify-between gap-3 px-6 pt-5 pb-1">
              <div className="min-w-0">
                <h2 id={titleId} className="type-heading text-(--wb-ink)">
                  {title}
                </h2>
                {description && (
                  <p
                    id={descriptionId}
                    className="mt-1 text-[14px] leading-5 text-(--wb-ink-2)"
                  >
                    {description}
                  </p>
                )}
              </div>
              {dismissible && showCloseButton && (
                <IconButton
                  ref={closeRef}
                  data-testid={closeTestId}
                  aria-label={closeLabel}
                  variant="secondary"
                  size="sm"
                  className="-me-1"
                  onClick={() => onOpenChange(false)}
                >
                  <X size={16} />
                </IconButton>
              )}
            </div>
            <div
              className={`min-h-0 overflow-y-auto px-6 pt-3 pb-5 ${contentClassName}`}
              style={contentFades ? FADE_MASK : undefined}
            >
              {children}
            </div>
            {footer && (
              <div className="flex shrink-0 justify-end gap-2 border-t border-(--wb-line) px-6 py-4">
                {footer}
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    container ?? document.body,
  );
}

export interface SheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Accessible name; usually the visible heading's text. */
  label: string;
  children: ReactNode;
  closeLabel?: string;
  /** Shows a back chevron at the top left. */
  onBack?: () => void;
  backLabel?: string;
  /** Progress dashes centred at the top. */
  progress?: { current: number; total: number; label?: string };
  /** Centred footer (one black button, usually). */
  footer?: ReactNode;
  dismissible?: boolean;
  closeOnBackdrop?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  /** `warm` is the lesson sheet (`--wb-sheet`); `plain` is white. */
  tone?: "warm" | "plain";
  /**
   * `lg` (default) is the full lesson sheet, 940 × 640 pt; `md` is 720 pt
   * wide and as tall as its content, up to the same height (What's new).
   */
  size?: "lg" | "md";
  className?: string;
  testId?: string;
}

/** The large lesson / chooser sheet: ≈ 940 × 640 pt, 16 pt radius. */
export function Sheet({
  open,
  onOpenChange,
  label,
  children,
  closeLabel,
  onBack,
  backLabel,
  progress,
  footer,
  dismissible = true,
  closeOnBackdrop = true,
  initialFocusRef,
  tone = "warm",
  size = "lg",
  className = "",
  testId,
}: SheetProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const panelRef = useRef<HTMLDivElement>(null);
  const m = useMotionPresets();
  const pinned = usePinnedThemeProps();
  useModal({ open, dismissible, onOpenChange, initialFocusRef, panelRef });

  const onScrimDown = (event: MouseEvent<HTMLDivElement>) => {
    if (dismissible && closeOnBackdrop && event.target === event.currentTarget)
      onOpenChange(false);
  };

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          key="wb-sheet"
          {...m.sheet.scrim}
          {...pinned}
          className="fixed inset-0 z-50 flex items-center justify-center bg-(--wb-scrim) p-4 sm:p-8"
          onMouseDown={onScrimDown}
        >
          <motion.div
            {...m.sheet.panel}
            ref={panelRef}
            role="dialog"
            aria-modal="true"
            aria-label={label}
            tabIndex={-1}
            data-testid={testId}
            className={`flex w-full flex-col ${size === "lg" ? "h-[min(640px,calc(100dvh-4rem))] max-w-[940px]" : "max-h-[min(640px,calc(100dvh-4rem))] max-w-[720px]"} overflow-hidden rounded-(--wb-radius-lg) text-(--wb-ink) shadow-(--wb-elev-3) outline-none ${tone === "warm" ? "bg-(--wb-sheet)" : "bg-(--wb-surface)"} ${className}`}
          >
            <div className="grid shrink-0 grid-cols-[1fr_auto_1fr] items-center px-5 pt-5">
              <div>
                {onBack && (
                  <IconButton
                    aria-label={backLabel ?? t("ui.importWizard.back")}
                    onClick={onBack}
                  >
                    <ArrowLeft size={18} className="rtl:-scale-x-100" />
                  </IconButton>
                )}
              </div>
              {progress ? (
                <ProgressDashes
                  current={progress.current}
                  total={progress.total}
                  label={progress.label}
                />
              ) : (
                <span />
              )}
              <div className="flex justify-end">
                {dismissible && (
                  <IconButton
                    aria-label={closeLabel ?? t("ui.importWizard.close")}
                    onClick={() => onOpenChange(false)}
                  >
                    <X size={18} />
                  </IconButton>
                )}
              </div>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto px-10 py-6">
              {children}
            </div>
            {footer && (
              <div className="flex shrink-0 justify-center gap-2 px-10 pt-2 pb-8">
                {footer}
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
