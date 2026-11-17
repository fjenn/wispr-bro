// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Live iPhone state for the UI: the dictation session, the setup checks
 * (microphone, keyboard, Full Access) and the speech model's readiness.
 *
 * The backend is asked on mount, whenever the app comes back to the front
 * (the user returns from iOS Settings or from another app) and on a slow
 * poll while the app is visible; the events `ios-session-state` and
 * `ios-setup-state`, when the backend sends them, update it at once. The
 * speech model follows the same command and event the Mac onboarding uses
 * (`fork_speech_setup_status`, `fork-speech-setup`).
 */
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { commands, events, type ForkSpeechSetup } from "../bindings";
import {
  DEFAULT_SESSION_TIMEOUT,
  iosCommands,
  type IosSession,
  type IosSetup,
} from "./commands";

const POLL_MS = 1500;

interface IosState {
  session: IosSession | null;
  setup: IosSetup | null;
  speech: ForkSpeechSetup | null;
  /** The backend did not answer the last session or setup question. */
  unreachable: boolean;
  /** "End session after", in seconds (0 immediately, -1 never). */
  timeout: number;
  autoReturn: boolean;
  refresh: () => Promise<void>;
  refreshSpeech: () => Promise<void>;
  loadPrefs: () => Promise<void>;
  setTimeout: (seconds: number) => Promise<boolean>;
  setAutoReturn: (enabled: boolean) => Promise<boolean>;
}

export const useIosStore = create<IosState>()((set, get) => ({
  session: null,
  setup: null,
  speech: null,
  unreachable: false,
  timeout: DEFAULT_SESSION_TIMEOUT,
  autoReturn: true,
  loadPrefs: async () => {
    try {
      const [timeout, autoReturn] = await Promise.all([
        iosCommands.getSessionTimeout(),
        iosCommands.getAutoReturn(),
      ]);
      set({ timeout, autoReturn });
    } catch (e) {
      console.warn("ios: reading the session settings failed", e);
    }
  },
  setTimeout: async (seconds) => {
    const before = get().timeout;
    set({ timeout: seconds });
    try {
      await iosCommands.setSessionTimeout(seconds);
      return true;
    } catch (e) {
      console.warn("ios: saving the session timeout failed", e);
      set({ timeout: before });
      return false;
    }
  },
  setAutoReturn: async (enabled) => {
    const before = get().autoReturn;
    set({ autoReturn: enabled });
    try {
      await iosCommands.setAutoReturn(enabled);
      return true;
    } catch (e) {
      console.warn("ios: saving auto-return failed", e);
      set({ autoReturn: before });
      return false;
    }
  },
  refresh: async () => {
    try {
      const [session, setup] = await Promise.all([
        iosCommands.sessionState(),
        iosCommands.setupState(),
      ]);
      set({ session, setup, unreachable: false });
    } catch (e) {
      console.warn("ios: reading the session failed", e);
      set({ unreachable: true });
    }
  },
  refreshSpeech: async () => {
    try {
      const speech = await commands.forkSpeechSetupStatus();
      if (speech && typeof speech === "object") set({ speech });
    } catch (e) {
      console.warn("ios: reading the speech model state failed", e);
    }
  },
}));

let users = 0;
let stop: (() => void) | null = null;

function start(): () => void {
  const { refresh, refreshSpeech, loadPrefs } = useIosStore.getState();
  const tick = () => {
    if (document.visibilityState === "visible") void refresh();
  };
  const onVisible = () => {
    if (document.visibilityState === "visible") {
      void refresh();
      void refreshSpeech();
    }
  };
  void refresh();
  void refreshSpeech();
  void loadPrefs();
  const timer = window.setInterval(tick, POLL_MS);
  document.addEventListener("visibilitychange", onVisible);
  window.addEventListener("focus", onVisible);
  const offs = [
    listen<IosSession>("ios-session-state", ({ payload }) => {
      if (payload && typeof payload === "object") {
        useIosStore.setState({ session: payload });
      }
    }),
    listen<IosSetup>("ios-setup-state", ({ payload }) => {
      if (payload && typeof payload === "object") {
        useIosStore.setState({ setup: payload });
      }
    }),
    events.forkSpeechSetup.listen(({ payload }) => {
      useIosStore.setState({ speech: payload });
    }),
  ];
  return () => {
    window.clearInterval(timer);
    document.removeEventListener("visibilitychange", onVisible);
    window.removeEventListener("focus", onVisible);
    for (const off of offs) void off.then((fn) => fn()).catch(() => undefined);
  };
}

/** Keeps the store current while any component using it is mounted. */
export function useIosLive(): void {
  useEffect(() => {
    users += 1;
    if (users === 1) stop = start();
    return () => {
      users -= 1;
      if (users === 0) {
        stop?.();
        stop = null;
      }
    };
  }, []);
}

export interface SetupProgress {
  mic: boolean;
  keyboard: boolean;
  fullAccess: boolean;
  model: boolean;
  /** Every check passed. */
  done: boolean;
  /** Both answers are in (nothing is shown as missing before that). */
  known: boolean;
}

export function setupProgress(
  setup: IosSetup | null,
  speech: ForkSpeechSetup | null,
): SetupProgress {
  const mic = setup?.micPermission === 2;
  const keyboard = !!setup?.keyboardEnabled;
  const fullAccess = !!setup?.fullAccess;
  const model = speech?.phase === "ready";
  return {
    mic,
    keyboard,
    fullAccess,
    model,
    done: mic && keyboard && fullAccess && model,
    known: setup !== null && speech !== null,
  };
}
