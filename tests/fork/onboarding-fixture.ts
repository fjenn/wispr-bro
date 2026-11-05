// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The onboarding specs' first-run backend (S-29a, S-29b): Handy settings
 * with onboarding not completed, fork onboarding state that
 * `fork_onboarding_go_to` advances, a hotkey engine, a speech model still
 * downloading, and the practice/typing-test commands.
 */
import type { Page } from "@playwright/test";
import { expect, type IpcMock } from "./fixtures";
import { handySettings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";

export const STEPS = [
  "welcome",
  "move_to_applications",
  "languages",
  "permissions",
  "mic_test",
  "shortcut_test",
  "practice",
  "first_dictation",
  "time_saved",
  "typing_test",
  "summary",
] as const;
export type Step = (typeof STEPS)[number];

export interface HotkeyStatus {
  running: boolean;
  error: string | null;
  trigger: string | null;
  globe: string;
  fn_blocked: boolean;
  issues: unknown[];
  bindings: unknown[];
}

export function hotkeys(over: Partial<HotkeyStatus> = {}): HotkeyStatus {
  return {
    running: true,
    error: null,
    trigger: "fn",
    globe: "do_nothing",
    fn_blocked: false,
    issues: [],
    bindings: [],
    ...over,
  };
}

export interface Backend {
  fork: Record<string, unknown> & {
    onboarding: {
      status: string;
      current_step: string | null;
      completed_steps: string[];
      checklist_done: string[];
      completed_at_ms: number | null;
    };
  };
  handy: Record<string, unknown>;
  status: HotkeyStatus;
  secureInput: boolean;
  location: string;
  mic: boolean;
  accessibility: boolean;
}

export interface Options {
  step?: Step;
  handy?: Record<string, unknown>;
  status?: Partial<HotkeyStatus>;
  secureInput?: boolean;
  location?: string;
  mic?: boolean;
  accessibility?: boolean;
  /** macOS preferred languages as Handy's backend reports them. */
  systemLanguages?: string[];
  theme?: "light" | "dark";
  /** The step the page should settle on (default: `step`, else welcome). */
  settle?: Step;
  /** `fork_speech_setup_status` fields over the default (downloading). */
  setup?: Record<string, unknown>;
}

/**
 * A first-run backend: onboarding not completed in Handy's settings, fork
 * onboarding state that `fork_onboarding_go_to` advances, a hotkey engine
 * and a speech model still downloading (nothing may wait for it).
 */
export async function openOnboarding(
  page: Page,
  ipc: IpcMock,
  options: Options = {},
): Promise<Backend> {
  const be: Backend = {
    fork: structuredClone({
      ...forkSettingsDefaults,
      onboarding: options.step
        ? {
            status: "in_progress",
            current_step: options.step,
            completed_steps: [],
            checklist_done: [],
            completed_at_ms: null,
          }
        : forkSettingsDefaults.onboarding,
    }) as Backend["fork"],
    handy: handySettings({
      onboarding_completed: false,
      spoken_languages: [],
      theme: options.theme ?? "light",
      ...(options.handy ?? {}),
    }),
    status: hotkeys(options.status),
    secureInput: options.secureInput ?? false,
    location: options.location ?? "already_in_applications",
    mic: options.mic ?? true,
    accessibility: options.accessibility ?? true,
  };
  await page.addInitScript(
    (langs) => {
      (
        window as unknown as Record<string, unknown>
      ).__HANDY_SYSTEM_LANGUAGES__ = { languages: langs };
    },
    options.systemLanguages ?? ["en"],
  );
  await page.setViewportSize({ width: 820, height: 720 });
  ipc.handle("get_app_settings", () => be.handy);
  ipc.handle("get_default_settings", () => handySettings());
  ipc.handle("change_spoken_languages_setting", (args) => {
    be.handy.spoken_languages = args.languages;
    return null;
  });
  ipc.handle(
    "plugin:macos-permissions|check_accessibility_permission",
    () => be.accessibility,
  );
  ipc.handle(
    "plugin:macos-permissions|check_microphone_permission",
    () => be.mic,
  );
  ipc.respond({
    get_retired_models: [],
    get_available_models: [],
    get_available_microphones: [],
    get_available_output_devices: [],
    get_current_model: "",
    check_custom_sounds: { start: false, stop: false },
    is_update_checks_locked: false,
    initialize_enigo: null,
    initialize_shortcuts: null,
    complete_onboarding: null,
    "plugin:app|version": "1.0.0-alpha.1",
    // The model is still downloading for the whole run.
    fork_speech_setup_status: {
      phase: "downloading",
      percent: 2,
      downloaded_bytes: 2_000_000,
      total_bytes: 90_000_000,
      problem: null,
      needed_mb: null,
      queued_takes: 0,
      eta_seconds: null,
      starter_languages: ["en"],
      ...(options.setup ?? {}),
    },
    fork_onboarding_practice: null,
    fork_speech_setup_refresh: null,
    fork_mic_list: {
      entries: [
        {
          kind: "device",
          name: "MacBook Pro Microphone",
          value: "MacBook Pro Microphone",
          recommended: true,
          wireless: false,
          system_default: true,
          transport: "built_in",
        },
        {
          kind: "auto",
          name: "",
          value: "auto",
          recommended: false,
          wireless: false,
          system_default: false,
          transport: "unknown",
        },
      ],
      selected: "auto",
      follow_system: true,
      in_use: "MacBook Pro Microphone",
      system_default: "MacBook Pro Microphone",
      avoided: null,
    },
    fork_mic_monitor_start: 3,
    fork_mic_monitor_stop: null,
    fork_onboarding_open_pane: null,
    fork_onboarding_move_to_applications: null,
    start_handy_keys_recording: null,
    stop_handy_keys_recording: null,
  });
  ipc.handle("get_secure_input_status", () => ({
    active: be.secureInput,
    enabled: be.secureInput,
    sustained: false,
    culprit_pid: null,
    culprit_name: null,
    degraded_bindings: [],
    uncovered_bindings: [],
    recorder_blocked: false,
  }));
  ipc.handle("fork_get_settings", () => be.fork);
  ipc.handle("fork_set_setting", (args) => {
    const update = args.update as { key: string; value: unknown };
    be.fork = { ...be.fork, [update.key]: update.value };
    return be.fork;
  });
  ipc.handle("change_autostart_setting", (args) => {
    be.handy.autostart_enabled = args.enabled;
    return null;
  });
  ipc.handle("fork_onboarding_go_to", (args) => {
    be.fork.onboarding = {
      ...be.fork.onboarding,
      status: "in_progress",
      current_step: args.step as string,
    };
    return be.fork.onboarding;
  });
  ipc.handle("fork_onboarding_complete", () => {
    be.fork.onboarding = {
      ...be.fork.onboarding,
      status: "completed",
      current_step: null,
    };
    return be.fork.onboarding;
  });
  ipc.handle("fork_onboarding_app_location", () => be.location);
  ipc.handle("fork_hotkey_status", () => be.status);
  ipc.handle("fork_onboarding_key_test", () => be.status);
  ipc.handle("fork_use_hotkey_profile", () => {
    be.status = { ...be.status, trigger: "ctrl+option" };
    return be.status;
  });
  ipc.handle("fork_onboarding_globe_do_nothing", () => {
    be.status = { ...be.status, globe: "do_nothing", fn_blocked: false };
    return be.status;
  });
  await page.goto("/");
  await expect(page.getByTestId("wb-onboarding")).toHaveAttribute(
    "data-step",
    options.settle ?? options.step ?? "welcome",
  );
  return be;
}

export const flow = (page: Page) => page.getByTestId("wb-onboarding");

export async function expectStep(page: Page, step: Step) {
  await expect(flow(page)).toHaveAttribute("data-step", step);
}
