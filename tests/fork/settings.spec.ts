// SPDX-License-Identifier: GPL-3.0-or-later
import type { Page } from "@playwright/test";
import { test, expect, type IpcMock } from "./fixtures";
import { expectNoSeriousAxeFindings, handySettings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";
import { openSettingsModal } from "./settings-modal-fixture";

const responses = {
  fork_mic_list: {
    entries: [
      {
        value: "auto",
        name: "",
        kind: "auto",
        recommended: false,
        wireless: false,
        system_default: false,
        transport: "unknown",
      },
    ],
    selected: "auto",
    follow_system: true,
    in_use: "Built-in Microphone",
    system_default: "Built-in Microphone",
    avoided: null,
  },
  fork_mic_monitor_start: 1,
  fork_mic_monitor_stop: null,
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

function openSection(page: Page, ipc: IpcMock, section = "general") {
  return openSettingsModal(page, ipc, section, responses);
}

function installPersistentSettings(ipc: IpcMock) {
  let current = structuredClone(forkSettingsDefaults);
  ipc.handle("fork_get_settings", () => current);
  ipc.handle("fork_set_setting", (args) => {
    const update = args.update as { key: string; value: unknown };
    if (update.key === "hotkey") {
      const change = update.value as {
        action: string;
        bindings: unknown[];
      };
      current = {
        ...current,
        hotkeys: { ...current.hotkeys, [change.action]: change.bindings },
      };
    } else {
      current = { ...current, [update.key]: update.value };
    }
    return current;
  });
}

test("General opens as a modal and Change reaches the one hotkeys panel", async ({
  page,
  ipc,
}) => {
  await openSection(page, ipc);
  await expect(page.getByRole("dialog", { name: "Settings" })).toBeVisible();
  await expect(page.getByTestId("wb-mic-picker")).toBeVisible();
  await expect(page.getByText("My languages", { exact: true })).toBeVisible();
  await expect(page.getByText("App language and theme")).toBeVisible();
  await expect(page.getByTestId("wb-settings-advanced")).toHaveCount(0);
  await page.getByTestId("wb-shortcuts-change").click();
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
  await expect(page).toHaveURL(/#\/hub\/settings\/hotkeys$/);
  await page.getByTestId("wb-hotkeys-done").click();
  await expect(page.getByTestId("wb-settings-general")).toBeVisible();
});

test("System, Data and Privacy, collapsed Advanced, and About deep-link", async ({
  page,
  ipc,
}) => {
  await openSection(page, ipc, "system");
  await expect(page.getByTestId("wb-system-show-dock")).toBeChecked();
  await expect(page.getByTestId("wb-system-notifications")).toBeVisible();
  await expect(page.getByTestId("wb-scratchpad-open-behavior")).toHaveValue(
    "resume_last",
  );
  await page.getByTestId("wb-settings-nav-privacy").click();
  await expect(page.getByTestId("wb-settings-privacy")).toContainText(
    "Retention and privacy controls",
  );
  await page.getByTestId("wb-settings-nav-advanced").click();
  await expect(page.getByTestId("wb-settings-nav-advanced")).toHaveAttribute(
    "aria-expanded",
    "true",
  );
  await expect(page.getByTestId("wb-advanced-models")).toBeVisible();
  await expect(page.getByTestId("wb-advanced-ai")).toBeVisible();
  await expect(page.getByTestId("wb-advanced-live-text")).toBeVisible();
  await expect(page.getByTestId("wb-advanced-update-channel")).toHaveValue(
    "stable",
  );
  await page.getByTestId("wb-settings-nav-about").click();
  await expect(page.getByTestId("wb-about-version")).toHaveText(
    "Version 1.0.0-alpha.1",
  );
});

test("Help > Shortcuts opens the hotkeys panel", async ({ page, ipc }) => {
  await openSection(page, ipc);
  await page.keyboard.press("Escape");
  await page.getByTestId("wb-nav-help").click();
  await page.getByTestId("wb-menu-item-shortcuts").click();
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
});

test("Dock and Scratchpad choices survive modal close and relaunch", async ({
  page,
  ipc,
}) => {
  await openSection(page, ipc, "system");
  installPersistentSettings(ipc);
  await page.getByTestId("wb-system-show-dock").uncheck();
  await page
    .getByTestId("wb-scratchpad-open-behavior")
    .selectOption("new_note");
  await page.getByTestId("wb-settings-close").click();
  await expect(page.getByTestId("wb-settings-modal")).toHaveCount(0);
  await page.getByTestId("wb-nav-settings").click();
  await page.getByTestId("wb-settings-nav-system").click();
  await expect(page.getByTestId("wb-system-show-dock")).not.toBeChecked();
  await expect(page.getByTestId("wb-scratchpad-open-behavior")).toHaveValue(
    "new_note",
  );
  await page.reload();
  await expect(page.getByTestId("wb-system-show-dock")).not.toBeChecked();
  await expect(page.getByTestId("wb-scratchpad-open-behavior")).toHaveValue(
    "new_note",
  );
});

test("Advanced choices persist across relaunch", async ({ page, ipc }) => {
  await openSection(page, ipc, "advanced");
  installPersistentSettings(ipc);
  await page.getByTestId("wb-advanced-live-text").check();
  await page.getByTestId("wb-advanced-update-channel").selectOption("beta");
  await page.reload();
  await expect(page.getByTestId("wb-advanced-live-text")).toBeChecked();
  await expect(page.getByTestId("wb-advanced-update-channel")).toHaveValue(
    "beta",
  );
});

test("onboarding opens the same hotkeys panel", async ({ page, ipc }) => {
  const handy = handySettings({ onboarding_completed: false });
  ipc.respond({
    get_app_settings: handy,
    get_default_settings: handy,
    get_retired_models: [],
    "plugin:macos-permissions|check_accessibility_permission": true,
    "plugin:macos-permissions|check_microphone_permission": true,
    fork_get_settings: forkSettingsDefaults,
    fork_set_setting: forkSettingsDefaults,
    fork_hotkey_status: {
      running: true,
      error: null,
      trigger: "fn",
      globe: "do_nothing",
      fn_blocked: false,
      issues: [],
      bindings: [],
    },
  });
  await page.goto("/");
  await page.getByTestId("wb-onboarding-shortcuts").click();
  await expect(page.getByTestId("wb-onboarding-hotkeys-modal")).toBeVisible();
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
  await page.getByTestId("wb-hotkeys-done").click();
  await expect(page.getByTestId("wb-onboarding-hotkeys-modal")).toHaveCount(0);
});

test("keyboard-only navigation stays in the modal and Escape closes it", async ({
  page,
  ipc,
}) => {
  await openSection(page, ipc);
  await expect(page.getByTestId("wb-settings-close")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("wb-settings-nav-general")).toBeFocused();
  await page.keyboard.press("Tab");
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("wb-settings-system")).toBeVisible();
  await page.keyboard.press("Shift+Tab");
  await expect(page.getByTestId("wb-settings-nav-general")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("wb-settings-modal")).toHaveCount(0);
});

test("RTL mirrors the two-column modal in every section", async ({
  page,
  ipc,
}) => {
  await openSection(page, ipc);
  for (const section of ["general", "system", "privacy", "advanced", "about"]) {
    if (section !== "general") {
      await page.getByTestId(`wb-settings-nav-${section}`).click();
    }
    await expect(page.getByTestId("wb-settings-modal")).toHaveAttribute(
      "dir",
      "ltr",
    );
    await page.getByTestId("wb-settings-modal").evaluate((dialog) => {
      dialog.setAttribute("dir", "rtl");
    });
    const nav = await page.getByTestId("wb-settings-nav-general").boundingBox();
    const content = await page.getByTestId("wb-settings-content").boundingBox();
    expect(nav && content && nav.x > content.x).toBe(true);
    await page.getByTestId("wb-settings-modal").evaluate((dialog) => {
      dialog.setAttribute("dir", "ltr");
    });
  }
});

for (const section of [
  "general",
  "system",
  "privacy",
  "advanced",
  "about",
  "hotkeys",
]) {
  for (const theme of ["light", "dark"] as const) {
    test(`axe and reduced motion: ${section} (${theme})`, async ({
      page,
      ipc,
    }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openSection(page, ipc, section);
      await expectNoSeriousAxeFindings(page);
      await expect(page.getByTestId("wb-settings-modal")).toBeVisible();
    });
  }
}
