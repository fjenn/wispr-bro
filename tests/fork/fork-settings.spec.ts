// SPDX-License-Identifier: GPL-3.0-or-later
// F-07: `useForkSettings()` reads the settings through `fork_get_settings`,
// writes through `fork_set_setting` and follows `ForkSettingsChanged`, via
// the fork settings section of the debug panel (mocked IPC).
import { test, expect, type Page } from "@playwright/test";
import { installTauriMock, type MockResponses } from "./ipc-mock";
import { forkSettingsDefaults } from "./settings-fixture";
import { handySettings } from "./hub-fixture";

const pingReply = {
  message: "pong",
  seq: 0,
  ready: true,
  debug_build: true,
  cli_debug: false,
};

/** The Rust defaults (`ForkSettings::default()`), as the backend sends them. */
const defaults = forkSettingsDefaults;

type Call = { cmd: string; args: Record<string, unknown> };

async function calls(page: Page, cmd: string): Promise<Call[]> {
  return page.evaluate(
    (name) =>
      (
        (window as unknown as { __WB_IPC_CALLS__: Call[] }).__WB_IPC_CALLS__ ??
        []
      ).filter((c) => c.cmd === name),
    cmd,
  );
}

async function openPanel(page: Page, responses: MockResponses) {
  await page.addInitScript(installTauriMock, {
    get_app_settings: handySettings(),
    get_default_settings: handySettings(),
    get_retired_models: [],
    "plugin:macos-permissions|check_accessibility_permission": true,
    "plugin:macos-permissions|check_microphone_permission": true,
    fork_ping: pingReply,
    fork_model_status: { known_models: 0, downloaded_model_ids: [] },
    ...responses,
  });
  await page.goto("/#/hub/settings/advanced");
  const section = page.getByTestId("wb-advanced-fork-diagnostics");
  await expect(section).toBeVisible();
  await section.locator("summary").first().click();
  return section;
}

test.describe("useForkSettings", () => {
  test("reads the settings and updates one through fork_set_setting", async ({
    page,
  }) => {
    const section = await openPanel(page, {
      fork_get_settings: defaults,
      fork_set_setting: { ...defaults, context_awareness: false },
    });
    const checkbox = section.getByTestId("fork-settings-context-awareness");
    await expect(checkbox).toBeChecked();
    await expect(
      section.getByTestId("fork-settings-cleanup-level"),
    ).toHaveValue("light");
    expect((await calls(page, "fork_get_settings")).length).toBeGreaterThan(0);

    await checkbox.click();
    await expect(checkbox).not.toBeChecked();
    const sets = await calls(page, "fork_set_setting");
    expect(sets).toHaveLength(1);
    expect(sets[0].args).toEqual({
      update: { key: "context_awareness", value: false },
    });
  });

  test("follows ForkSettingsChanged from the backend", async ({ page }) => {
    const section = await openPanel(page, { fork_get_settings: defaults });
    const level = section.getByTestId("fork-settings-cleanup-level");
    await expect(level).toHaveValue("light");

    // The backend broadcasts a change made elsewhere (another window).
    await expect
      .poll(() =>
        page.evaluate(() => {
          const w = window as unknown as {
            __WB_IPC_CALLS__: Call[];
          };
          return w.__WB_IPC_CALLS__.some(
            (c) =>
              c.cmd === "plugin:event|listen" &&
              c.args.event === "fork-settings-changed",
          );
        }),
      )
      .toBe(true);
    await page.evaluate(
      (settings) => {
        const T = (
          window as unknown as {
            __TAURI_INTERNALS__: {
              invoke: (cmd: string, args: unknown) => Promise<unknown>;
            };
          }
        ).__TAURI_INTERNALS__;
        return T.invoke("plugin:event|emit", {
          event: "fork-settings-changed",
          payload: { key: "cleanup_level", settings },
        });
      },
      { ...defaults, cleanup_level: "medium" },
    );
    await expect(level).toHaveValue("medium");
  });

  test("shows a rejected value and keeps the stored one", async ({ page }) => {
    const section = await openPanel(page, {
      fork_get_settings: defaults,
      fork_set_setting: {
        __error: { kind: "InvalidSetting", message: "cleanup_level: no" },
      },
    });
    const level = section.getByTestId("fork-settings-cleanup-level");
    await level.selectOption("medium");
    await expect(section.getByRole("alert")).toContainText("cleanup_level: no");
    await expect(level).toHaveValue("light");
  });
});
