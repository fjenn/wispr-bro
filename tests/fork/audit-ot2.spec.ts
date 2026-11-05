// SPDX-License-Identifier: GPL-3.0-or-later
// Audit OT2 (low-severity fixes): Settings › System's Dock and menu bar
// icons are never both off, the notification switches say what they cover,
// and Advanced's "Translate to English" is off and explained for models
// that cannot translate. Light/dark axe, RTL and keyboard for each.
import type { Page } from "@playwright/test";
import { test, expect, type IpcMock } from "./fixtures";
import { expectNoSeriousAxeFindings, handySettings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";
import { openSettingsModal } from "./settings-modal-fixture";

const MODELS = [
  {
    id: "parakeet-tdt-0.6b-v3",
    name: "Parakeet V3",
    supports_translation: false,
  },
  { id: "whisper-small", name: "Whisper Small", supports_translation: true },
].map((m) => ({
  description: "",
  filename: `${m.id}.gguf`,
  url: null,
  sha256: null,
  size_mb: 100,
  is_downloaded: true,
  is_downloading: false,
  partial_size: 0,
  is_directory: false,
  engine_type: "Whisper",
  accuracy_score: 0.5,
  speed_score: 0.5,
  supports_language_selection: false,
  supported_languages: ["en"],
  is_custom: false,
  ...m,
}));

const SPEECH = {
  fork_model_policy_status: {
    automatic: true,
    phase: "settled",
    current_model: "Parakeet",
    next_model: null,
    percent: null,
    slower: false,
    languages: ["en"],
    machine: null,
  },
  fork_models_hidden: [],
  fork_llm_status: {
    state: "ready",
    model_name: "Local cleanup",
    downloaded_bytes: 1,
    total_bytes: 1,
    server: {
      phase: "stopped",
      device: "gpu",
      transport: null,
      ready_ms: null,
      restarts: 0,
      last_error: null,
    },
    error: null,
  },
  fork_llm_apple_status: {
    availability: "unsupported",
    role: "fallback",
    context_tokens: 0,
    requests: 0,
    refusals: 0,
    rate_limited: 0,
  },
  fork_llm_last_cleanup: null,
  get_microphone_channels: 1,
};

/** Fork and Handy settings the mocked backend keeps, writes included. */
function installBackend(
  ipc: IpcMock,
  fork: Record<string, unknown>,
  handy: Record<string, unknown>,
) {
  let current: Record<string, unknown> = {
    ...structuredClone(forkSettingsDefaults),
    ...fork,
  };
  let handyNow = handySettings({
    post_process_providers: [],
    post_process_provider_id: "local",
    ...handy,
  });
  ipc.handle("fork_get_settings", () => current);
  ipc.handle("fork_set_setting", (args) => {
    const update = args.update as { key: string; value: unknown };
    current = { ...current, [update.key]: update.value };
    return current;
  });
  ipc.handle("get_app_settings", () => handyNow);
  ipc.handle("change_show_tray_icon_setting", (args) => {
    handyNow = { ...handyNow, show_tray_icon: args.enabled };
    return null;
  });
  ipc.handle("change_translate_to_english_setting", (args) => {
    handyNow = { ...handyNow, translate_to_english: args.enabled };
    return null;
  });
}

async function openSystem(
  page: Page,
  ipc: IpcMock,
  fork: Record<string, unknown> = {},
  handy: Record<string, unknown> = {},
) {
  await openSettingsModal(page, ipc, "system", {
    fork_get_settings: { ...forkSettingsDefaults, ...fork },
    get_app_settings: handySettings(handy),
  });
  installBackend(ipc, fork, handy);
  await expect(page.getByTestId("wb-settings-system")).toBeVisible();
}

async function openAdvanced(page: Page, ipc: IpcMock, currentModel: string) {
  await openSettingsModal(page, ipc, "advanced", {
    ...SPEECH,
    get_available_models: MODELS,
    get_current_model: currentModel,
  });
  installBackend(ipc, {}, { translate_to_english: false });
  await expect(page.getByTestId("wb-settings-advanced")).toBeVisible();
}

test.describe("Dock and menu bar icon", () => {
  test("turning the Dock icon off keeps the menu bar icon on, and back", async ({
    page,
    ipc,
  }) => {
    await openSystem(page, ipc);
    const dock = page.getByTestId("wb-system-show-dock");
    const tray = page.getByTestId("wb-system-show-tray");
    await expect(dock).toBeChecked();
    await expect(tray).toBeChecked();
    await expect(tray).toBeEnabled();

    // Keyboard only: Space toggles the focused switch.
    await dock.focus();
    await page.keyboard.press("Space");
    await expect(dock).not.toBeChecked();
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "show_in_dock", value: false },
    });
    await expect(tray).toBeDisabled();
    await expect(page.getByTestId("wb-system-show-tray-row")).toContainText(
      "The Dock icon is off, so the menu bar icon stays.",
    );

    await dock.focus();
    await page.keyboard.press("Space");
    await expect(dock).toBeChecked();
    await expect(tray).toBeEnabled();
  });

  test("with the menu bar icon off the Dock icon cannot go", async ({
    page,
    ipc,
  }) => {
    await openSystem(page, ipc, {}, { show_tray_icon: false });
    const dock = page.getByTestId("wb-system-show-dock");
    await expect(page.getByTestId("wb-system-show-tray")).not.toBeChecked();
    await expect(dock).toBeChecked();
    await expect(dock).toBeDisabled();
    await expect(page.getByTestId("wb-system-show-dock-row")).toContainText(
      "The menu bar icon is off, so the Dock icon stays.",
    );
    await page.getByTestId("wb-system-show-tray").click();
    await ipc.expectCalled("change_show_tray_icon_setting", { enabled: true });
    await expect(dock).toBeEnabled();
  });

  test("an install that already has both off can turn either back on", async ({
    page,
    ipc,
  }) => {
    await openSystem(
      page,
      ipc,
      { show_in_dock: false },
      { show_tray_icon: false },
    );
    await expect(page.getByTestId("wb-system-show-dock")).toBeEnabled();
    await expect(page.getByTestId("wb-system-show-tray")).toBeEnabled();
  });
});

test("notification switches say what they cover", async ({ page, ipc }) => {
  await openSystem(page, ipc);
  const group = page.getByTestId("wb-system-notifications");
  await expect(group).toContainText("Shortcut problems");
  await expect(group).not.toContainText("Problems that need attention");
  await expect(page.getByTestId("wb-notifications-hint")).toHaveText(
    "Each switch covers that notice in Wispr Bro and on the Mac. “Mac notifications” turns off only the Mac banners. Crash and recovery notices always appear in Wispr Bro.",
  );
  await page.getByTestId("wb-notification-model_ready").click();
  await ipc.expectCalled("fork_set_setting", {
    update: { key: "notifications", value: { model_ready: false } },
  });
});

test.describe("Translate to English", () => {
  test("is off and explained when the model cannot translate", async ({
    page,
    ipc,
  }) => {
    await openAdvanced(page, ipc, "parakeet-tdt-0.6b-v3");
    const row = page.getByTestId("wb-advanced-translate-row");
    await expect(row).toContainText("Translate to English");
    await expect(row).toContainText(
      "Parakeet V3 can't translate. Choose a model that can, such as Whisper or Canary, in Speech models.",
    );
    await expect(page.getByTestId("wb-advanced-translate")).toBeDisabled();
    await expect(page.getByTestId("wb-advanced-translate")).not.toBeChecked();
  });

  test("works from the keyboard when the model can translate", async ({
    page,
    ipc,
  }) => {
    await openAdvanced(page, ipc, "whisper-small");
    const toggle = page.getByTestId("wb-advanced-translate");
    await expect(toggle).toBeEnabled();
    await toggle.focus();
    await page.keyboard.press("Space");
    await ipc.expectCalled("change_translate_to_english_setting", {
      enabled: true,
    });
    await expect(toggle).toBeChecked();
  });
});

for (const theme of ["light", "dark"] as const) {
  test(`System and Advanced pass axe in ${theme}, also in RTL`, async ({
    page,
    ipc,
  }) => {
    test.setTimeout(180_000);
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await openSystem(page, ipc, { show_in_dock: false });
    await expect(page.getByTestId("wb-system-show-tray")).toBeDisabled();
    await expectNoSeriousAxeFindings(page);
    await page.getByTestId("wb-settings-modal").evaluate((dialog) => {
      dialog.setAttribute("dir", "rtl");
    });
    const overflow = await page
      .getByTestId("wb-settings-content")
      .evaluate((content) => content.scrollWidth > content.clientWidth + 1);
    expect(overflow).toBe(false);
    await expectNoSeriousAxeFindings(page);

    await openAdvanced(page, ipc, "parakeet-tdt-0.6b-v3");
    await expect(page.getByTestId("wb-advanced-translate")).toBeDisabled();
    await expectNoSeriousAxeFindings(page);
  });
}
