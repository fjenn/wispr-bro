// SPDX-License-Identifier: GPL-3.0-or-later
// F-07: `useForkSettings()` reads the settings through `fork_get_settings`,
// writes through `fork_set_setting` and follows `ForkSettingsChanged`, via
// the fork settings section of the debug panel (mocked IPC).
import { test, expect, type Page } from "@playwright/test";
import { installTauriMock, type MockResponses } from "./ipc-mock";

const pingReply = {
  message: "pong",
  seq: 0,
  ready: true,
  debug_build: true,
  cli_debug: false,
};

/** The Rust defaults (`ForkSettings::default()`), as the backend sends them. */
const defaults = {
  schema_version: 1,
  hotkeys: {
    cancel: [{ chord: "escape", activation: "press" }],
    dictate: [{ chord: "fn", activation: "hold" }],
    hands_free: [
      { chord: "fn", activation: "double_tap" },
      { chord: "fn+space", activation: "press" },
    ],
    paste_last: [{ chord: "ctrl+command+v", activation: "press" }],
  },
  cleanup_level: "light",
  styles: {
    personal: "relaxed",
    work: "formal",
    email: "formal",
    other: "formal",
  },
  context_awareness: true,
  transforms_enabled: false,
  command_mode_enabled: false,
  auto_apply_transform: null,
  bar_always_visible: true,
  bar_position: "bottom",
  bar_hidden_until_ms: null,
  bar_hide_in_recordings: false,
  bar_x_by_display: {},
  live_text_in_pill: false,
  text_retention_days: null,
  audio_retention_days: 14,
  notifications: {
    system: true,
    model_ready: true,
    update_ready: true,
    queued_takes_ready: true,
    errors: true,
  },
  update_checks_enabled: true,
  update_channel: "stable",
  show_in_dock: true,
  onboarding: {
    status: "not_started",
    current_step: null,
    completed_steps: [],
    checklist_done: [],
    completed_at_ms: null,
  },
  dismissed_banners: [],
  seen_new_tags: [],
  first_use_choice: null,
  typing_wpm: null,
  typing_hours_per_day: 2,
  greeting_name: null,
  auto_learn_enabled: false,
  vibe_coding: { enabled: false, setup_completed: false, editors: [] },
  applied_first_run_defaults: [],
};

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
    fork_ping: pingReply,
    fork_model_status: { known_models: 0, downloaded_model_ids: [] },
    ...responses,
  });
  await page.goto("/#/fork/debug");
  const section = page.getByTestId("fork-debug-section-settings");
  await expect(section).toBeVisible();
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
