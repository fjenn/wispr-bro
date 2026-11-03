// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Shortcut health in the Hub (S-02): the status from `fork::shortcut::health`
 * (both event taps), live through `ForkShortcutHealthChanged`, and whether
 * the Fix sheet is open. The bar's Fix button and the tray ask for the sheet
 * with `ForkShortcutFixRequested`.
 */
import { create } from "zustand";
import { commands, events, type HealthStatus } from "../bindings";

/** The Hub section that opens the Fix sheet (the S-18 notification). */
export const FIX_SHORTCUT_SECTION = "fix-shortcut";

interface ShortcutHealthState {
  status: HealthStatus | null;
  sheetOpen: boolean;
  setStatus: (status: HealthStatus) => void;
  openSheet: () => void;
  closeSheet: () => void;
  refresh: () => Promise<void>;
  recheck: () => Promise<void>;
}

export const useShortcutHealth = create<ShortcutHealthState>()((set) => ({
  status: null,
  sheetOpen: false,
  setStatus: (status) => set({ status }),
  openSheet: () => set({ sheetOpen: true }),
  closeSheet: () => set({ sheetOpen: false }),
  refresh: async () => {
    try {
      set({ status: await commands.forkShortcutHealth() });
    } catch (e) {
      console.warn("fork: reading the shortcut health failed", e);
    }
  },
  recheck: async () => {
    try {
      set({ status: await commands.forkShortcutRecheck() });
    } catch (e) {
      console.warn("fork: re-checking the shortcut failed", e);
    }
  },
}));

/** Loads the status and follows its changes (mounted once by the Hub). */
export function listenForShortcutHealth(): () => void {
  const store = useShortcutHealth.getState();
  void store.refresh();
  const unlisteners = [
    events.forkShortcutHealthChanged.listen((e) =>
      useShortcutHealth.getState().setStatus(e.payload.status),
    ),
    events.forkShortcutFixRequested.listen(() =>
      useShortcutHealth.getState().openSheet(),
    ),
  ];
  return () => {
    for (const u of unlisteners) void u.then((fn) => fn());
  };
}
