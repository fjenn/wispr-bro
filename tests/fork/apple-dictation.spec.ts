// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * OT2: Apple Dictation's "Press fn twice" while fn is the trigger
 * (`shortcut/apple_dictation.rs`): the Settings › Shortcuts row (switch,
 * "still active" line with Open Keyboard settings, re-check on focus) and
 * the one-time Hub note after the app switched it off.
 */
import type { Page } from "@playwright/test";
import type { HotkeyStatus } from "../../src/fork/bindings";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
} from "./hub-fixture";
import type { IpcMock } from "./fixtures";

type State = HotkeyStatus["apple_dictation"];

function status(state: State, trigger = "fn"): HotkeyStatus {
  return {
    running: true,
    error: null,
    trigger,
    globe: "do_nothing",
    fn_blocked: false,
    apple_dictation: state,
    issues: [],
    bindings: [],
  };
}

async function openShortcuts(
  page: Page,
  ipc: IpcMock,
  hub: Parameters<typeof openHub>[2],
  current: { state: State; trigger?: string },
) {
  ipc.handle("fork_hotkey_status", () =>
    status(current.state, current.trigger),
  );
  ipc.respond({
    fork_onboarding_open_pane: null,
    start_handy_keys_recording: null,
    stop_handy_keys_recording: null,
    fork_transforms_list: [],
  });
  await openHub(page, ipc, hub, { hash: "#/hub/settings/hotkeys" });
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible({
    timeout: 30_000,
  });
}

test("still active: the warning opens Keyboard settings", async ({
  page,
  ipc,
  hub,
}) => {
  await openShortcuts(page, ipc, hub, { state: "active" });
  const row = page.getByTestId("wb-apple-dictation-row");
  await expect(row).toBeVisible();
  await expect(page.getByTestId("wb-apple-dictation-warning")).toContainText(
    "set the Dictation shortcut to Off",
  );
  await page.getByTestId("wb-apple-dictation-open").click();
  await ipc.expectCalled("fork_onboarding_open_pane", { pane: "keyboard" });
});

test("the switch keeps Apple's shortcut and the window focus re-checks", async ({
  page,
  ipc,
  hub,
}) => {
  const current = { state: "turned_off" as State };
  await openShortcuts(page, ipc, hub, current);
  const toggle = page.getByTestId("wb-apple-dictation-switch");
  await expect(toggle).toBeChecked();
  await expect(page.getByTestId("wb-apple-dictation-warning")).toHaveCount(0);
  current.state = "active";
  await toggle.click();
  await ipc.expectCalled("fork_set_setting", {
    update: { key: "apple_dictation_shortcut_off", value: false },
  });
  await expect(page.getByTestId("wb-apple-dictation-warning")).toBeVisible();
  // The user set Off in System Settings and came back.
  current.state = "none";
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await expect(page.getByTestId("wb-apple-dictation-warning")).toHaveCount(0);
  // Still shown: the switch is off, so the way back stays visible.
  await expect(page.getByTestId("wb-apple-dictation-row")).toBeVisible();
});

test("nothing shows when it does not apply", async ({ page, ipc, hub }) => {
  await openShortcuts(page, ipc, hub, { state: "none" });
  await expect(page.getByTestId("wb-apple-dictation-row")).toHaveCount(0);
  await expect(page.getByTestId("wb-apple-dictation-note")).toHaveCount(0);
});

test("the Hub note shows once and leads to Shortcuts", async ({
  page,
  ipc,
  hub,
}) => {
  ipc.handle("fork_hotkey_status", () => status("turned_off"));
  await openHub(page, ipc, hub);
  const note = page.getByTestId("wb-apple-dictation-note");
  await expect(note).toBeVisible({ timeout: 30_000 });
  await expect(note).toContainText("Press fn twice");
  await page.getByTestId("wb-apple-dictation-note-settings").click();
  await expect(page.getByTestId("wb-settings-modal")).toBeVisible();
  expect(hub.forkSettings.dismissed_banners).toContain(
    "hub.apple-dictation-off",
  );
});

for (const theme of ["light", "dark"] as const) {
  test(`row and note pass axe in ${theme}, RTL and by keyboard`, async ({
    page,
    ipc,
    hub,
  }) => {
    test.setTimeout(180_000);
    await page.emulateMedia({ colorScheme: theme });
    await openShortcuts(page, ipc, hub, { state: "active" });
    await page.evaluate(() => {
      document.documentElement.dir = "rtl";
    });
    await expect(page.getByTestId("wb-apple-dictation-warning")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    // Keyboard: the switch and the button take focus and act on Space/Enter.
    await page.getByTestId("wb-apple-dictation-switch").focus();
    await page.keyboard.press("Space");
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "apple_dictation_shortcut_off", value: false },
    });
    await page.getByTestId("wb-apple-dictation-open").focus();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_onboarding_open_pane", { pane: "keyboard" });
  });
}
