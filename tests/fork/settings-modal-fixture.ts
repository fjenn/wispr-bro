// SPDX-License-Identifier: GPL-3.0-or-later
import type { Page } from "@playwright/test";
import { expect, type IpcMock } from "./fixtures";
import { handySettings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";

export async function openSettingsModal(
  page: Page,
  ipc: IpcMock,
  section = "general",
  responses: Record<string, unknown> = {},
) {
  const handy = handySettings({
    post_process_providers: [],
    post_process_provider_id: "local",
    post_process_api_keys: {},
    post_process_models: {},
    post_process_prompts: [],
  });
  ipc.respond({
    get_app_settings: handy,
    get_default_settings: handy,
    get_retired_models: [],
    get_secure_input_status: {
      active: false,
      enabled: false,
      sustained: false,
      degraded_bindings: [],
      uncovered_bindings: [],
      recorder_blocked: false,
    },
    get_available_models: [],
    get_available_microphones: [],
    get_available_output_devices: [],
    get_current_model: "",
    check_custom_sounds: { start: false, stop: false },
    is_update_checks_locked: false,
    "plugin:macos-permissions|check_accessibility_permission": true,
    "plugin:macos-permissions|check_microphone_permission": true,
    "plugin:app|version": "1.0.0-alpha.1",
    fork_get_settings: forkSettingsDefaults,
    fork_set_setting: forkSettingsDefaults,
    fork_data_locations: {
      data: "/test/Data",
      models: "/test/Data/huggingface",
      recordings: "/test/Data/recordings",
      logs: "/test/Data/logs",
      portable: true,
    },
    fork_show_data_location: null,
    fork_export_data: { path: "/test/export.zip", files: 7, bytes: 100 },
    fork_copy_diagnostics: 1234,
    fork_save_diagnostics: 100,
    fork_erase_all_data: null,
    fork_hotkey_status: {
      running: true,
      error: null,
      trigger: "fn",
      globe: "do_nothing",
      fn_blocked: false,
      issues: [],
      bindings: [],
    },
    start_handy_keys_recording: null,
    stop_handy_keys_recording: null,
    ...responses,
  });
  await page.setViewportSize({ width: 1040, height: 720 });
  await page.goto(`/#/hub/settings/${section}`);
  const modal = page.getByTestId("wb-settings-modal");
  await expect(modal).toBeVisible({ timeout: 30_000 });
  return modal;
}

/**
 * Picks `value` in a Radix `Select` (Design W3 replaced the native selects):
 * opens the combobox, clicks the option, and waits for the list to close.
 */
export async function chooseOption(page: Page, testId: string, value: string) {
  const trigger = page.getByTestId(testId);
  await expect(trigger).toBeEnabled();
  await trigger.click();
  await page.getByTestId(`${testId}-option-${value}`).click();
  await expect(page.getByRole("listbox")).toHaveCount(0);
}

/** The labels a Radix `Select` offers, in order (opens and closes it). */
export async function expectOptions(
  page: Page,
  testId: string,
  labels: string[],
) {
  await page.getByTestId(testId).click();
  await expect(page.getByRole("listbox").getByRole("option")).toHaveText(
    labels,
  );
  await page.keyboard.press("Escape");
  await expect(page.getByRole("listbox")).toHaveCount(0);
}
