// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-29a onboarding: every step and branch (Welcome, Move to Applications,
 * Languages, Permissions, the microphone test, the shortcut test and each
 * "No" diagnosis), resume after a relaunch at every step, preview mode,
 * Back, no step waiting on the speech-model download, no model name, and
 * axe in light/dark, RTL and keyboard-only use.
 */
import type { Page } from "@playwright/test";
import { test, expect, type IpcMock } from "./fixtures";
import { expectNoSeriousAxeFindings, handySettings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";
import { openSettingsModal } from "./settings-modal-fixture";

const STEPS = [
  "welcome",
  "move_to_applications",
  "languages",
  "permissions",
  "mic_test",
  "shortcut_test",
] as const;
type Step = (typeof STEPS)[number];

interface HotkeyStatus {
  running: boolean;
  error: string | null;
  trigger: string | null;
  globe: string;
  fn_blocked: boolean;
  issues: unknown[];
  bindings: unknown[];
}

function hotkeys(over: Partial<HotkeyStatus> = {}): HotkeyStatus {
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

interface Backend {
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

interface Options {
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
}

/**
 * A first-run backend: onboarding not completed in Handy's settings, fork
 * onboarding state that `fork_onboarding_go_to` advances, a hotkey engine
 * and a speech model still downloading (nothing may wait for it).
 */
async function openOnboarding(
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
      progress: 0.02,
      bytes_done: 2_000_000,
      bytes_total: 90_000_000,
      problem: null,
      queued_takes: 0,
    },
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
  ipc.handle("fork_set_setting", () => be.fork);
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

const flow = (page: Page) => page.getByTestId("wb-onboarding");

async function expectStep(page: Page, step: Step) {
  await expect(flow(page)).toHaveAttribute("data-step", step);
}

test.describe("S-29a onboarding", () => {
  test("a first run goes Welcome › Languages › Permissions › Setup and finishes", async ({
    page,
    ipc,
  }) => {
    const be = await openOnboarding(page, ipc, {
      systemLanguages: ["nl", "en"],
    });
    await ipc.expectCalled("fork_onboarding_go_to", { step: "welcome" });
    await ipc.expectCalled("fork_speech_setup_refresh");
    const stepper = page.getByTestId("wb-onboarding-stepper");
    await expect(stepper.locator('[aria-current="step"]')).toContainText(
      "Welcome",
    );
    await expect(page.getByTestId("wb-onboarding-back")).toHaveCount(0);

    await page.getByRole("button", { name: "Get started" }).click();
    await expectStep(page, "languages");
    // Preselected from macOS: both chips, and a Continue that saves them.
    const field = page.getByTestId("wb-onboarding-languages");
    await expect(
      field.getByRole("button", { name: "Remove Dutch" }),
    ).toBeVisible();
    await expect(
      field.getByRole("button", { name: "Remove English" }),
    ).toBeVisible();
    await expect(
      page.getByTestId("wb-onboarding-languages-note"),
    ).toContainText("languages");
    await page.getByTestId("wb-languages-continue").click();
    await expectStep(page, "permissions");
    expect(be.handy.spoken_languages).toEqual(["nl", "en"]);
    expect(
      (await ipc.calls("fork_speech_setup_refresh")).length,
    ).toBeGreaterThanOrEqual(2);
    await expect(stepper.locator('[aria-current="step"]')).toContainText(
      "Permissions",
    );

    // Both granted already: the rows tick and Continue is open.
    await expect(
      page.getByTestId("wb-permission-microphone-granted"),
    ).toBeVisible();
    await expect(
      page.getByTestId("wb-permission-accessibility-granted"),
    ).toBeVisible();
    await page.getByTestId("wb-permissions-continue").click();

    await expectStep(page, "mic_test");
    await expect(stepper.locator('[aria-current="step"]')).toContainText(
      "Setup",
    );
    // The meter starts on its own.
    await ipc.expectCalled("fork_mic_monitor_start");
    await expect(page.getByTestId("wb-mic-continue")).toBeDisabled();
    await page.getByTestId("wb-mic-yes").click();
    await page.getByTestId("wb-mic-continue").click();

    await expectStep(page, "shortcut_test");
    await ipc.expectCalled("fork_onboarding_key_test", { on: true });
    // Leaving the meter's screen stopped the monitor.
    await ipc.expectCalled("fork_mic_monitor_stop");
    const key = page.getByTestId("wb-shortcut-key");
    await expect(key).toHaveAttribute("data-pressed", "false");
    await expect(page.getByTestId("wb-shortcut-finish")).toBeDisabled();
    await ipc.waitForListener("fork-hotkey-event");
    await ipc.emit("fork-hotkey-event", {
      kind: "key_test",
      detail: "down",
      keep: false,
    });
    await expect(key).toHaveAttribute("data-pressed", "true");
    await expect(page.getByTestId("wb-shortcut-state")).toHaveText("Key held");
    await ipc.emit("fork-hotkey-event", {
      kind: "key_test",
      detail: "up",
      keep: false,
    });
    await expect(key).toHaveAttribute("data-pressed", "false");
    await expect(page.getByTestId("wb-shortcut-state")).toContainText(
      "It works",
    );
    // A take event is not the test: it never lights the key.
    await ipc.emit("fork-hotkey-event", {
      kind: "take_started",
      detail: null,
      keep: false,
    });
    await expect(key).toHaveAttribute("data-pressed", "false");

    await page.getByTestId("wb-shortcut-finish").click();
    await ipc.expectCalled("fork_onboarding_complete");
    await ipc.expectCalled("complete_onboarding");
    await expect(page.getByTestId("wb-onboarding")).toHaveCount(0);
    // The test mode ends with the screen.
    await ipc.expectCalled("fork_onboarding_key_test", { on: false });
    expect(be.fork.onboarding.status).toBe("completed");
  });

  test("no step names a speech model or waits for its download", async ({
    page,
    ipc,
  }) => {
    await openOnboarding(page, ipc);
    const forbidden = /parakeet|whisper|moonshine|sensevoice|\bmodel\b/i;
    const check = async () =>
      expect(await page.getByTestId("wb-onboarding").innerText()).not.toMatch(
        forbidden,
      );
    await check();
    await page.getByRole("button", { name: "Get started" }).click();
    await check();
    await page.getByTestId("wb-languages-continue").click();
    await check();
    await page.getByTestId("wb-permissions-continue").click();
    await check();
    await page.getByTestId("wb-mic-yes").click();
    await page.getByTestId("wb-mic-continue").click();
    await check();
    await page.getByTestId("wb-shortcut-yes").click();
    await expect(page.getByTestId("wb-shortcut-finish")).toBeEnabled();
    // The download was never asked about: no screen reads its state.
    expect(await ipc.calls("fork_speech_setup_status")).toHaveLength(0);
  });

  for (const step of STEPS.filter((s) => s !== "move_to_applications")) {
    test(`a relaunch resumes at ${step}`, async ({ page, ipc }) => {
      await openOnboarding(page, ipc, { step });
      await expect(page.locator("main h1")).toBeVisible();
      // Resuming does not write a step (the saved one stands).
      expect(await ipc.calls("fork_onboarding_go_to")).toHaveLength(0);
    });
  }

  test("a relaunch at Move to Applications goes on once the app is in Applications", async ({
    page,
    ipc,
  }) => {
    await openOnboarding(page, ipc, {
      step: "move_to_applications",
      settle: "languages",
    });
    await expectStep(page, "languages");
    await ipc.expectCalled("fork_onboarding_go_to", { step: "languages" });
  });

  test("Move to Applications: shown when needed, moves on the button, skippable", async ({
    page,
    ipc,
  }) => {
    const be = await openOnboarding(page, ipc, { location: "move_needed" });
    await page.getByRole("button", { name: "Get started" }).click();
    await expectStep(page, "move_to_applications");
    await expect(page.locator("main h1")).toHaveText(
      "Move Wispr Bro to Applications",
    );
    // A failed move says what to do and leaves the user here (a move that
    // works quits and reopens the app).
    ipc.respond("fork_onboarding_move_to_applications", {
      __error: { kind: "Internal", message: "exists" },
    });
    await page.getByTestId("wb-move-action").click();
    await ipc.expectCalled("fork_onboarding_move_to_applications");
    await expect(page.getByTestId("wb-move-error")).toContainText("Finder");
    await page.getByTestId("wb-move-skip").click();
    await expectStep(page, "languages");
    // Back from Languages skips the move screen.
    await page.getByTestId("wb-onboarding-back").click();
    await expectStep(page, "welcome");
    expect(be.fork.onboarding.current_step).toBe("welcome");
  });

  test("Back walks the steps and saves each one", async ({ page, ipc }) => {
    await openOnboarding(page, ipc, { step: "shortcut_test" });
    const back = page.getByTestId("wb-onboarding-back");
    for (const step of ["mic_test", "permissions", "languages", "welcome"]) {
      await back.click();
      await expectStep(page, step as Step);
      await ipc.expectCalled("fork_onboarding_go_to", { step });
    }
    await expect(back).toHaveCount(0);
  });

  test("Permissions: rows ask macOS, tick when granted, and gate Continue", async ({
    page,
    ipc,
  }) => {
    const be = await openOnboarding(page, ipc, {
      step: "permissions",
      mic: false,
      accessibility: false,
    });
    ipc.respond({
      "plugin:macos-permissions|request_microphone_permission": null,
      "plugin:macos-permissions|request_accessibility_permission": null,
    });
    await expect(page.getByTestId("wb-permissions-continue")).toBeDisabled();
    // Each row shows the prompt and the System Settings pane.
    await expect(
      page.getByTestId("wb-permission-art-microphone-prompt"),
    ).toHaveAttribute("aria-label", /microphone prompt/);
    await expect(
      page.getByTestId("wb-permission-art-accessibility-pane"),
    ).toHaveAttribute("data-family", "legacy");
    await page.getByTestId("wb-permission-microphone-allow").click();
    await ipc.expectCalled(
      "plugin:macos-permissions|request_microphone_permission",
    );
    be.mic = true;
    await expect(
      page.getByTestId("wb-permission-microphone-granted"),
    ).toBeVisible({ timeout: 5000 });
    await expect(page.getByTestId("wb-permissions-continue")).toBeDisabled();
    await page.getByTestId("wb-permission-accessibility-allow").click();
    await ipc.expectCalled(
      "plugin:macos-permissions|request_accessibility_permission",
    );
    be.accessibility = true;
    await expect(
      page.getByTestId("wb-permission-accessibility-granted"),
    ).toBeVisible({ timeout: 5000 });
    await expect(page.getByTestId("wb-permissions-continue")).toBeEnabled();
  });

  test("macOS 26 gets the redesigned permission pictures", async ({
    page,
    ipc,
  }) => {
    await page.addInitScript(() => {
      const w = window as unknown as Record<string, Record<string, unknown>>;
      const os = w.__TAURI_OS_PLUGIN_INTERNALS__;
      Object.defineProperty(w, "__TAURI_OS_PLUGIN_INTERNALS__", {
        configurable: true,
        get: () => ({ ...os, version: "26.1" }),
        set: () => undefined,
      });
    });
    await openOnboarding(page, ipc, { step: "permissions", mic: false });
    await expect(
      page.getByTestId("wb-permission-art-microphone-pane"),
    ).toHaveAttribute("data-family", "current");
  });

  test("Microphone test: No opens the list in the S-08 order", async ({
    page,
    ipc,
  }) => {
    await openOnboarding(page, ipc, { step: "mic_test" });
    await expect(page.locator("main")).toContainText("Bluetooth");
    await ipc.waitForListener("fork-mic-level");
    await ipc.emit("fork-mic-level", { session: 3, level: 0.6 });
    await page.getByTestId("wb-mic-no").click();
    const list = page.getByTestId("wb-onboarding-mic-list");
    await expect(list).toBeVisible();
    await expect(list.getByTestId("wb-mic-option").first()).toContainText(
      "MacBook Pro Microphone",
    );
    await expect(page.getByTestId("wb-mic-continue")).toBeDisabled();
    await page.getByTestId("wb-mic-yes").click();
    await expect(page.getByTestId("wb-mic-continue")).toBeEnabled();
  });

  test("Edit shortcut opens the hotkeys panel", async ({ page, ipc }) => {
    await openOnboarding(page, ipc, { step: "shortcut_test" });
    await page.getByTestId("wb-edit-shortcut").click();
    await expect(page.getByTestId("wb-onboarding-hotkeys-modal")).toBeVisible();
    await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
    await page.getByTestId("wb-hotkeys-done").click();
    await expect(page.getByTestId("wb-onboarding-hotkeys-modal")).toHaveCount(
      0,
    );
    // Closing renews the test with the (maybe new) binding.
    expect(
      (await ipc.calls("fork_onboarding_key_test")).length,
    ).toBeGreaterThanOrEqual(2);
  });

  test.describe("No leads to a diagnosis", () => {
    test("tap not live → Accessibility pane, Try again restarts the listener", async ({
      page,
      ipc,
    }) => {
      const be = await openOnboarding(page, ipc, {
        step: "shortcut_test",
        status: { running: false, error: "no accessibility" },
      });
      await page.getByTestId("wb-shortcut-no").click();
      await expect(page.getByTestId("wb-diagnosis-panel-tap")).toBeVisible();
      await page.getByTestId("wb-diagnosis-tap-open").click();
      await ipc.expectCalled("fork_onboarding_open_pane", {
        pane: "accessibility",
      });
      be.status = hotkeys();
      const before = (await ipc.calls("fork_onboarding_key_test")).length;
      await page.getByTestId("wb-shortcut-try-again").click();
      await expect(page.getByTestId("wb-diagnosis-panel-tap")).toHaveCount(0);
      expect((await ipc.calls("fork_onboarding_key_test")).length).toBe(
        before + 1,
      );
    });

    test("Secure Input → explains and shows the live state", async ({
      page,
      ipc,
    }) => {
      await openOnboarding(page, ipc, {
        step: "shortcut_test",
        secureInput: true,
      });
      await page.getByTestId("wb-shortcut-no").click();
      await expect(
        page.getByTestId("wb-diagnosis-panel-secure_input"),
      ).toBeVisible();
      await expect(page.getByTestId("wb-secure-input-state")).toHaveText(
        "Right now, another app is blocking key presses.",
      );
    });

    test("Globe bound to emoji → DEC-44 Do Nothing with consent, or Keyboard settings", async ({
      page,
      ipc,
    }) => {
      await openOnboarding(page, ipc, {
        step: "shortcut_test",
        status: { globe: "emoji", fn_blocked: true },
      });
      await page.getByTestId("wb-shortcut-no").click();
      await expect(page.getByTestId("wb-diagnosis-panel-globe")).toBeVisible();
      // Nothing changes before the button.
      expect(await ipc.calls("fork_onboarding_globe_do_nothing")).toHaveLength(
        0,
      );
      await page.getByTestId("wb-diagnosis-globe-open").click();
      await ipc.expectCalled("fork_onboarding_open_pane", { pane: "keyboard" });
      await page.getByTestId("wb-diagnosis-globe-action").click();
      await ipc.expectCalled("fork_onboarding_globe_do_nothing");
      await expect(
        page.getByTestId("wb-diagnosis-globe-action"),
      ).toBeDisabled();
      await expect(page.getByTestId("wb-diagnosis-globe-action")).toHaveText(
        /does nothing/,
      );
    });

    test("no fn key → switch to Control + Option", async ({ page, ipc }) => {
      await openOnboarding(page, ipc, { step: "shortcut_test" });
      await page.getByTestId("wb-shortcut-no").click();
      await expect(page.getByTestId("wb-diagnosis-panel-no_fn")).toBeVisible();
      await page.getByTestId("wb-diagnosis-no_fn-action").click();
      await ipc.expectCalled("fork_use_hotkey_profile", { fallback: true });
      await expect(page.getByTestId("wb-shortcut-key")).toHaveAttribute(
        "aria-label",
        "Control and Option",
      );
      await expect(page.locator("main")).toContainText(
        "hold Control and Option",
      );
    });

    test("every diagnosis is reachable from any other", async ({
      page,
      ipc,
    }) => {
      await openOnboarding(page, ipc, { step: "shortcut_test" });
      await page.getByTestId("wb-shortcut-no").click();
      for (const d of ["tap", "secure_input", "globe", "no_fn"]) {
        await page.getByTestId(`wb-diagnosis-${d}`).click();
        await expect(page.getByTestId(`wb-diagnosis-panel-${d}`)).toBeVisible();
      }
    });
  });

  test("preview from Settings › Advanced is inert", async ({ page, ipc }) => {
    const handy = handySettings({
      debug_mode: true,
      post_process_providers: [],
      post_process_provider_id: "local",
      post_process_api_keys: {},
      post_process_models: {},
      post_process_prompts: [],
    });
    await openSettingsModal(page, ipc, "advanced", {
      get_app_settings: handy,
      get_default_settings: handy,
    });
    await page.getByTestId("wb-settings-nav-advanced").click();
    await page.getByText("Choose a screen").click();
    await page.getByText("Try it", { exact: true }).click();
    await expectStep(page, "shortcut_test");
    await page.getByTestId("wb-shortcut-no").click();
    await expect(page.getByTestId("wb-diagnosis-panel-globe")).toBeVisible();
    await page.getByTestId("wb-diagnosis-globe-action").click();
    await page.getByTestId("wb-onboarding-back").click();
    await expectStep(page, "mic_test");
    await page.getByTestId("wb-onboarding-back").click();
    await page.getByTestId("wb-onboarding-back").click();
    await expectStep(page, "languages");
    await page.getByTestId("wb-languages-continue").click();
    await expectStep(page, "permissions");
    for (const cmd of [
      "fork_onboarding_go_to",
      "fork_onboarding_key_test",
      "fork_onboarding_globe_do_nothing",
      "fork_mic_monitor_start",
      "change_spoken_languages_setting",
      "plugin:macos-permissions|request_microphone_permission",
    ]) {
      expect(await ipc.calls(cmd), cmd).toHaveLength(0);
    }
  });

  test("Help opens a dialog and Escape closes it", async ({ page, ipc }) => {
    await openOnboarding(page, ipc);
    await page.getByRole("button", { name: "Help" }).click();
    const dialog = page.getByRole("dialog", { name: "Help with setup" });
    await expect(dialog).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
  });

  test("keyboard only: Welcome to Permissions without a pointer", async ({
    page,
    ipc,
  }) => {
    await openOnboarding(page, ipc);
    const start = page.getByRole("button", { name: "Get started" });
    for (
      let i = 0;
      i < 6 && !(await start.evaluate((e) => e === document.activeElement));
      i++
    ) {
      await page.keyboard.press("Tab");
    }
    await expect(start).toBeFocused();
    await page.keyboard.press("Enter");
    await expectStep(page, "languages");
    // Focus moves to the new screen's heading.
    await expect(page.locator("main h1")).toBeFocused();
    const next = page.getByTestId("wb-languages-continue");
    for (
      let i = 0;
      i < 250 && !(await next.evaluate((e) => e === document.activeElement));
      i++
    ) {
      await page.keyboard.press("Tab");
    }
    await expect(next).toBeFocused();
    await page.keyboard.press("Enter");
    await expectStep(page, "permissions");
  });

  test("right-to-left layout in Arabic", async ({ page, ipc }) => {
    await openOnboarding(page, ipc, {
      step: "shortcut_test",
      handy: { app_language: "ar" },
    });
    await expect(flow(page)).toHaveAttribute("dir", "rtl");
    const back = await page.getByTestId("wb-onboarding-back").boundingBox();
    const box = await flow(page).boundingBox();
    expect(back && box && back.x > box.width / 2).toBe(true);
  });

  for (const theme of ["light", "dark"] as const) {
    for (const step of [
      "welcome",
      "languages",
      "permissions",
      "mic_test",
      "shortcut_test",
    ] as const) {
      test(`axe: ${step} in ${theme}`, async ({ page, ipc }) => {
        await page.emulateMedia({
          colorScheme: theme,
          reducedMotion: "reduce",
        });
        await openOnboarding(page, ipc, {
          step,
          theme,
          mic: step !== "permissions",
          accessibility: step !== "permissions",
        });
        if (step === "shortcut_test") {
          await page.getByTestId("wb-shortcut-no").click();
        }
        await expect(page.locator("main h1")).toBeVisible();
        await expectNoSeriousAxeFindings(page);
      });
    }
  }
});
