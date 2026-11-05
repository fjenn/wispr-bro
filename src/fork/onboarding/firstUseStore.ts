// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * First-use state shared by the Hub (S-30): whether the chooser is open,
 * the bar spotlight as the backend reports it, and the dictation trigger
 * (fn, or Control+Option on the fallback profile) for the hint's wording.
 *
 * - The chooser opens by itself once, right after onboarding completes,
 *   until a choice is made or "Maybe later" is pressed (stored as
 *   `first_use_choice` / `dismissed_banners`); the checklist's "Show me"
 *   reopens it.
 * - "Try it now" saves the choice and asks the backend for the spotlight
 *   (`fork_spotlight_start`); `false` means there is no bar to point at
 *   (not a product build), and the home banner then says so.
 */
import { useEffect } from "react";
import { create } from "zustand";
import { commands, events, type ForkSpotlightState } from "../bindings";
import { useForkSettingsStore } from "../settings/store";
import { CHOOSER_DISMISSED_ID, type FirstUseChoice } from "./firstUse";

interface FirstUseState {
  /** Opened by hand (checklist "Show me", banner "Try something else"). */
  chooserRequested: boolean;
  /** Closed in this session (no reopen until the next launch). */
  chooserClosed: boolean;
  spotlight: ForkSpotlightState | null;
  /** The last "Try it now" found no bar to point at. */
  unavailable: boolean;
  trigger: string | null;
}

export const useFirstUseStore = create<FirstUseState>()(() => ({
  chooserRequested: false,
  chooserClosed: false,
  spotlight: null,
  unavailable: false,
  trigger: "fn",
}));

export function openChooser(): void {
  useFirstUseStore.setState({ chooserRequested: true, chooserClosed: false });
}

/** "Maybe later" or the close button: it does not come back by itself. */
export function closeChooser(): void {
  useFirstUseStore.setState({ chooserRequested: false, chooserClosed: true });
  const { settings, setSetting } = useForkSettingsStore.getState();
  if (settings && !settings.dismissed_banners.includes(CHOOSER_DISMISSED_ID)) {
    void setSetting("dismissed_banners", [
      ...settings.dismissed_banners,
      CHOOSER_DISMISSED_ID,
    ]);
  }
}

/** "Try it now": saves the choice, closes the chooser, starts the hint. */
export async function tryNow(choice: FirstUseChoice): Promise<void> {
  useFirstUseStore.setState({ chooserRequested: false, chooserClosed: true });
  await useForkSettingsStore.getState().setSetting("first_use_choice", choice);
  await showHint(choice);
}

/** Starts (or restarts) the bar spotlight for `choice`. */
export async function showHint(choice: FirstUseChoice): Promise<void> {
  const result = await commands.forkSpotlightStart(choice);
  useFirstUseStore.setState({
    unavailable: result.status === "ok" ? !result.data : true,
  });
  if (result.status === "error") {
    console.warn("first use: the spotlight did not start", result.error);
  }
}

export function hideHint(): void {
  void commands.forkSpotlightStop();
}

function newer(
  prev: ForkSpotlightState | null,
  next: ForkSpotlightState,
): ForkSpotlightState {
  return !prev || next.seq >= prev.seq ? next : prev;
}

/** Follows the spotlight and the trigger while the Hub is mounted. */
export function useFirstUseLive(): void {
  useEffect(() => {
    let alive = true;
    const off = events.forkSpotlightState.listen(({ payload }) => {
      useFirstUseStore.setState((s) => ({
        spotlight: newer(s.spotlight, payload),
      }));
    });
    void commands
      .forkSpotlightState()
      .then((state) => {
        if (alive) {
          useFirstUseStore.setState((s) => ({
            spotlight: newer(s.spotlight, state),
          }));
        }
      })
      .catch(() => undefined);
    void commands
      .forkHotkeyStatus()
      .then((result) => {
        if (alive && result.status === "ok") {
          useFirstUseStore.setState({ trigger: result.data.trigger });
        }
      })
      .catch(() => undefined);
    return () => {
      alive = false;
      void off.then((f) => f()).catch(() => undefined);
    };
  }, []);
}

/** The "how" line for the current trigger (`rust.spotlight.how*`). */
export function howKey(trigger: string | null): string {
  return trigger === "ctrl+option"
    ? "rust.spotlight.howFallback"
    : "rust.spotlight.how";
}
