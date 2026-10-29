// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * `useForkSettings()`: the fork settings for React components (F-07).
 *
 * Loads the settings on first use (`fork_get_settings`), updates them with
 * `fork_set_setting` and follows `ForkSettingsChanged` from the backend.
 *
 * ```tsx
 * const { settings, setSetting } = useForkSettings();
 * <Switch checked={settings?.context_awareness ?? true}
 *         onChange={(on) => setSetting("context_awareness", on)} />
 * ```
 */
import { useEffect } from "react";
import {
  listenForForkSettingsChanges,
  useForkSettingsStore,
  type ForkSettingsState,
} from "./store";

export type UseForkSettings = Pick<
  ForkSettingsState,
  "settings" | "status" | "error" | "pending" | "setSetting" | "refresh"
>;

export function useForkSettings(): UseForkSettings {
  // One selector per field: each re-renders only when its value changes.
  const settings = useForkSettingsStore((s) => s.settings);
  const status = useForkSettingsStore((s) => s.status);
  const error = useForkSettingsStore((s) => s.error);
  const pending = useForkSettingsStore((s) => s.pending);
  const setSetting = useForkSettingsStore((s) => s.setSetting);
  const refresh = useForkSettingsStore((s) => s.refresh);

  useEffect(() => {
    void listenForForkSettingsChanges();
    void useForkSettingsStore.getState().load();
  }, []);

  return { settings, status, error, pending, setSetting, refresh };
}
