// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * When the one-time feature popup may show (S-18; Mac wave 1, Finding 9).
 *
 * The popup is modal, so it must never cover something the user is
 * looking at or doing. It waits (it is not dismissed; it shows once the
 * way is clear) while:
 *
 * - a take runs: from Handy's `started`/`locked` until a short while after
 *   `stopped`/`cancelled`, while the text is still being delivered;
 * - S-21's View changes diff is open (`#/hub/transforms/changes`): ⌥O after
 *   a transform opens the Hub on it, often on the Hub's first open;
 * - any other dialog is open (What's New, the Fix shortcut sheet, a
 *   page's own dialog, the notifications panel).
 *
 * It also waits [`SETTLE_MS`] after the Hub page loads, so a handoff that
 * arrives with the page (View changes, a notification's Open) lands first.
 */
import { useEffect, useState } from "react";
import { events as handyEvents } from "@/bindings";
import { useHubNav } from "./nav";

/** After the page loads, before the popup may first show. */
export const SETTLE_MS = 800;
/** After a take stops, while its text is delivered. */
export const AFTER_TAKE_MS = 1500;

/** The popup's own content marker (its dialog is not "another" dialog). */
export const POPUP_MARKER = '[data-testid="wb-feature-popup"]';

/** Whether a dialog other than the popup is open in `root`. */
export function otherDialogOpen(root: ParentNode = document): boolean {
  return Array.from(
    root.querySelectorAll('[role="dialog"], [role="alertdialog"]'),
  ).some((dialog) => !dialog.querySelector(POPUP_MARKER));
}

function useTakeRunning(): boolean {
  const [running, setRunning] = useState(false);
  useEffect(() => {
    let timer: number | undefined;
    let off: (() => void) | undefined;
    let alive = true;
    handyEvents.recordingStateEvent
      .listen(({ payload }) => {
        window.clearTimeout(timer);
        if (payload.phase === "started" || payload.phase === "locked") {
          setRunning(true);
        } else {
          timer = window.setTimeout(() => setRunning(false), AFTER_TAKE_MS);
        }
      })
      .then((unlisten) => {
        if (alive) off = unlisten;
        else unlisten();
      })
      .catch(() => {
        // No take events (a test page): never deferred for a take.
      });
    return () => {
      alive = false;
      window.clearTimeout(timer);
      off?.();
    };
  }, []);
  return running;
}

function useOtherDialogOpen(): boolean {
  const [open, setOpen] = useState(() => otherDialogOpen());
  useEffect(() => {
    const check = () => setOpen(otherDialogOpen());
    const observer = new MutationObserver(check);
    observer.observe(document.body, { childList: true, subtree: true });
    check();
    return () => observer.disconnect();
  }, []);
  return open;
}

function useSettled(): boolean {
  const [settled, setSettled] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(true), SETTLE_MS);
    return () => window.clearTimeout(timer);
  }, []);
  return settled;
}

/** `true` while the feature popup must wait. */
export function useFeaturePopupDeferred(): boolean {
  const settled = useSettled();
  const take = useTakeRunning();
  const dialog = useOtherDialogOpen();
  const diff = useHubNav(
    (state) => state.page === "transforms" && state.section === "changes",
  );
  return !settled || take || dialog || diff;
}
