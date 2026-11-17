// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The iPhone backend commands (`src-tauri`, iOS only), called through
 * `invoke` because they are not in the generated bindings.
 *
 *   ios_session_state        → IosSession
 *   ios_start_session        → starts a dictation session
 *   ios_end_session          → ends it
 *   ios_setup_state          → IosSetup
 *   ios_request_mic          → shows the microphone prompt
 *   ios_open_settings        → opens the app's page in iOS Settings
 *   ios_get_session_timeout  → seconds (0 = immediately, -1 = never)
 *   ios_set_session_timeout  { seconds }
 *   ios_get_auto_return      → boolean
 *   ios_set_auto_return      { enabled }
 *   ios_take_start           → take id (a take of the app's own: a note's Record)
 *   ios_take_stop            { id }
 *   ios_take_cancel          { id }
 *   ios_take_state           { id } → IosTake
 *
 * With the dev override (`?platform=ios` in a desktop browser) a missing
 * command falls back to an in-memory stand-in so every screen can be
 * worked on without a phone. On a real device nothing is faked: a failed
 * command surfaces as an error.
 */
import { invoke } from "@tauri-apps/api/core";
import { isIosDevOverride } from "./platform";

export type IosSessionState =
  | "noSession"
  | "idle"
  | "listening"
  | "transcribing"
  | "error";

export interface IosSession {
  state: IosSessionState;
  message?: string | null;
  /** When the session ends by itself, in ms since the epoch. */
  endsAt?: number | null;
}

/** 0 undetermined, 1 denied, 2 granted. */
export type IosMicPermission = 0 | 1 | 2;

export interface IosSetup {
  keyboardEnabled: boolean;
  fullAccess: boolean;
  /**
   * Whether the keyboard has reported Full Access at all. It can only do
   * so once it has been on screen; until then `fullAccess` is false and
   * means "not known", not "off".
   */
  fullAccessKnown?: boolean;
  micPermission: IosMicPermission;
}

/** "End session after" choices, in seconds (0 immediately, -1 never). */
export const SESSION_TIMEOUTS = [0, 300, 900, 3600, -1] as const;
export const DEFAULT_SESSION_TIMEOUT = 300;

const SESSION_STATES: readonly IosSessionState[] = [
  "noSession",
  "idle",
  "listening",
  "transcribing",
  "error",
];

export type IosTakePhase =
  | "none"
  | "starting"
  | "listening"
  | "transcribing"
  | "done"
  | "failed"
  | "cancelled";

/** A take the app started itself (`ios_take_start`). */
export interface IosTake {
  phase: IosTakePhase;
  /** 0…1 while listening. */
  level: number;
  text?: string | null;
  error?: string | null;
}

const TAKE_PHASES: readonly IosTakePhase[] = [
  "none",
  "starting",
  "listening",
  "transcribing",
  "done",
  "failed",
  "cancelled",
];

/** The stand-in backend for the dev override. */
const dev = {
  take: { id: 0, phase: "none" as IosTakePhase, at: 0 },
  session: { state: "noSession" } as IosSession,
  setup: {
    keyboardEnabled: false,
    fullAccess: false,
    micPermission: 0,
  } as IosSetup,
  timeout: DEFAULT_SESSION_TIMEOUT as number,
  autoReturn: true,
};

async function call<T>(
  cmd: string,
  args: Record<string, unknown> | undefined,
  fallback: () => T,
  valid: (value: unknown) => value is T,
): Promise<T> {
  try {
    const value = await invoke<unknown>(cmd, args);
    if (valid(value)) return value;
    if (isIosDevOverride()) return fallback();
    throw new Error(`${cmd}: unexpected reply`);
  } catch (e) {
    if (isIosDevOverride()) return fallback();
    throw e;
  }
}

/** A command with no useful reply (the stand-in runs when it is missing). */
async function run(
  cmd: string,
  args: Record<string, unknown> | undefined,
  standIn: () => void,
): Promise<void> {
  try {
    const value = await invoke<unknown>(cmd, args);
    // The browser mock answers an unknown command with null.
    if (value === null && isIosDevOverride()) standIn();
  } catch (e) {
    if (!isIosDevOverride()) throw e;
    standIn();
  }
}

const isSession = (v: unknown): v is IosSession =>
  !!v &&
  typeof v === "object" &&
  SESSION_STATES.includes((v as IosSession).state);

const isSetup = (v: unknown): v is IosSetup =>
  !!v &&
  typeof v === "object" &&
  typeof (v as IosSetup).keyboardEnabled === "boolean" &&
  typeof (v as IosSetup).fullAccess === "boolean";

const isTake = (v: unknown): v is IosTake =>
  !!v && typeof v === "object" && TAKE_PHASES.includes((v as IosTake).phase);
const isString = (v: unknown): v is string =>
  typeof v === "string" && v.length > 0;

/** The dev stand-in's take: listens until stopped, then "writes" for 0.7 s. */
function devTake(id: string): IosTake {
  if (String(dev.take.id) !== id) return { phase: "none", level: 0 };
  const age = Date.now() - dev.take.at;
  if (dev.take.phase === "starting" && age > 250) dev.take.phase = "listening";
  if (dev.take.phase === "transcribing" && age > 700) dev.take.phase = "done";
  return {
    phase: dev.take.phase,
    level:
      dev.take.phase === "listening" ? 0.25 + 0.5 * Math.abs(Math.sin(age)) : 0,
    text: dev.take.phase === "done" ? "This sentence was dictated." : null,
    error: null,
  };
}

const isNumber = (v: unknown): v is number =>
  typeof v === "number" && Number.isFinite(v);
const isBoolean = (v: unknown): v is boolean => typeof v === "boolean";

export const iosCommands = {
  sessionState: () =>
    call("ios_session_state", undefined, () => ({ ...dev.session }), isSession),
  startSession: () =>
    run("ios_start_session", undefined, () => {
      dev.session = {
        state: "idle",
        endsAt: dev.timeout > 0 ? Date.now() + dev.timeout * 1000 : null,
      };
    }),
  endSession: () =>
    run("ios_end_session", undefined, () => {
      dev.session = { state: "noSession" };
    }),
  setupState: () =>
    call("ios_setup_state", undefined, () => ({ ...dev.setup }), isSetup),
  requestMic: () =>
    run("ios_request_mic", undefined, () => {
      dev.setup.micPermission = 2;
    }),
  openSettings: () =>
    run("ios_open_settings", undefined, () => {
      dev.setup.keyboardEnabled = true;
      dev.setup.fullAccess = true;
    }),
  getSessionTimeout: () =>
    call("ios_get_session_timeout", undefined, () => dev.timeout, isNumber),
  setSessionTimeout: (seconds: number) =>
    run("ios_set_session_timeout", { seconds }, () => {
      dev.timeout = seconds;
    }),
  getAutoReturn: () =>
    call("ios_get_auto_return", undefined, () => dev.autoReturn, isBoolean),
  takeStart: () =>
    call(
      "ios_take_start",
      undefined,
      () => {
        dev.take = { id: dev.take.id + 1, phase: "starting", at: Date.now() };
        return String(dev.take.id);
      },
      isString,
    ),
  takeStop: (id: string) =>
    run("ios_take_stop", { id }, () => {
      if (String(dev.take.id) === id && dev.take.phase !== "done") {
        dev.take = { ...dev.take, phase: "transcribing", at: Date.now() };
      }
    }),
  takeCancel: (id: string) =>
    run("ios_take_cancel", { id }, () => {
      if (String(dev.take.id) === id) dev.take.phase = "cancelled";
    }),
  takeState: (id: string) =>
    call("ios_take_state", { id }, () => devTake(id), isTake),
  setAutoReturn: (enabled: boolean) =>
    run("ios_set_auto_return", { enabled }, () => {
      dev.autoReturn = enabled;
    }),
};
