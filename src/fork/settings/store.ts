// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Zustand store for the fork settings (F-07, R5 §4.4).
 *
 * The Rust side (`fork-settings.json`, `fork/settings.rs`) is the source of
 * truth. The store loads it with `fork_get_settings`, changes one field at a
 * time with `fork_set_setting` (typed per key, validated in Rust) and follows
 * `ForkSettingsChanged`, which the backend emits after every change, so all
 * windows stay in sync. Components use `useForkSettings()`.
 */
import { create } from "zustand";
import {
  commands,
  events,
  type ForkError,
  type ForkSettingUpdate,
  type ForkSettings,
  type Result,
} from "../bindings";

export type ForkSettingKey = ForkSettingUpdate["key"];
export type ForkSettingValue<K extends ForkSettingKey> = Extract<
  ForkSettingUpdate,
  { key: K }
>["value"];

export type ForkSettingsStatus = "idle" | "loading" | "ready" | "error";

/** An error from a command, or a transport failure as a message. */
export type ForkSettingsError =
  | ForkError
  | { kind: "Transport"; message: string };

export interface ForkSettingsState {
  settings: ForkSettings | null;
  status: ForkSettingsStatus;
  /** Last load or update error; cleared by the next success. */
  error: ForkSettingsError | null;
  /** Keys with an update in flight. */
  pending: Partial<Record<ForkSettingKey, boolean>>;
  /** Loads the settings once (later calls reuse the first load). */
  load: () => Promise<void>;
  /** Re-reads the settings from the backend. */
  refresh: () => Promise<void>;
  /** Changes one setting; resolves with the backend's result. */
  setSetting: <K extends ForkSettingKey>(
    key: K,
    value: ForkSettingValue<K>,
  ) => Promise<Result<ForkSettings, ForkSettingsError>>;
  /** Applies settings pushed by the backend (`ForkSettingsChanged`). */
  receive: (settings: ForkSettings) => void;
}

function transportError(e: unknown): ForkSettingsError {
  return { kind: "Transport", message: String(e) };
}

let loadPromise: Promise<void> | null = null;
let listening: Promise<void> | null = null;

export const useForkSettingsStore = create<ForkSettingsState>()((set, get) => {
  const fetchSettings = async () => {
    set({ status: get().settings ? "ready" : "loading" });
    try {
      const result = await commands.forkGetSettings();
      if (result.status === "ok") {
        set({ settings: result.data, status: "ready", error: null });
      } else {
        set({ status: "error", error: result.error });
      }
    } catch (e) {
      set({ status: "error", error: transportError(e) });
    }
  };

  return {
    settings: null,
    status: "idle",
    error: null,
    pending: {},

    load: () => {
      loadPromise ??= fetchSettings();
      return loadPromise;
    },

    refresh: fetchSettings,

    setSetting: async (key, value) => {
      set({ pending: { ...get().pending, [key]: true } });
      let result: Result<ForkSettings, ForkSettingsError>;
      try {
        // The cast pairs `key` with its value type, which TypeScript cannot
        // narrow through a generic union member.
        result = await commands.forkSetSetting({
          key,
          value,
        } as ForkSettingUpdate);
      } catch (e) {
        result = { status: "error", error: transportError(e) };
      }
      const pending = { ...get().pending };
      delete pending[key];
      if (result.status === "ok") {
        set({ settings: result.data, status: "ready", error: null, pending });
      } else {
        set({ error: result.error, pending });
      }
      return result;
    },

    receive: (settings) => set({ settings, status: "ready" }),
  };
});

/**
 * Subscribes the store to `ForkSettingsChanged` once per page. Safe to call
 * from every hook instance.
 */
export function listenForForkSettingsChanges(): Promise<void> {
  listening ??= events.forkSettingsChanged
    .listen((event) =>
      useForkSettingsStore.getState().receive(event.payload.settings),
    )
    .then(() => undefined)
    .catch((e) => {
      listening = null;
      console.warn("fork: listening for fork settings changes failed", e);
    });
  return listening;
}

/** Test hook: forget the cached load and listener (new page state). */
export function resetForkSettingsStoreForTests(): void {
  loadPromise = null;
  listening = null;
  useForkSettingsStore.setState({
    settings: null,
    status: "idle",
    error: null,
    pending: {},
  });
}
