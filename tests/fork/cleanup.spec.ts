// SPDX-License-Identifier: GPL-3.0-or-later
// S-12a: the cleanup level picker on the debug panel (mocked IPC): levels,
// saving, the last take's result from the command and the event, the
// "Inserted without cleanup" notice, keyboard use and axe in light and dark.
import type { Page } from "@playwright/test";
import { test, expect, ipcError, type IpcMock } from "./fixtures";
import { expectNoSeriousAxeFindings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";

const pingReply = {
  message: "pong",
  seq: 0,
  ready: true,
  debug_build: true,
  cli_debug: false,
};

const settings = (level: "none" | "light" | "medium") => ({
  ...forkSettingsDefaults,
  cleanup_level: level,
});

const result = (over: Record<string, unknown> = {}) => ({
  take: 1,
  level: "light",
  engine: "local",
  outcome: "applied",
  why: null,
  notice: false,
  chunks_total: 1,
  chunks_applied: 1,
  llm_ms: 240,
  fixes: 2,
  guard_reasons: [],
  ...over,
});

async function openPicker(
  page: Page,
  ipc: IpcMock,
  responses: Record<string, unknown>,
) {
  ipc.respond({
    fork_ping: pingReply,
    fork_get_settings: settings("light"),
    fork_llm_last_cleanup: null,
    ...responses,
  });
  await page.goto("/#/fork/debug");
  const section = page.getByTestId("fork-debug-section-cleanup");
  await expect(section).toBeVisible({ timeout: 15_000 });
  await expect(section.getByTestId("fork-cleanup")).toBeVisible();
  return section;
}

test.describe("Cleanup level picker", () => {
  test("shows the three levels with Light chosen and saves a new level", async ({
    page,
    ipc,
  }) => {
    ipc.handle("fork_set_setting", (args) => {
      const update = args.update as { key: string; value: string };
      return settings(update.value as "none" | "light" | "medium");
    });
    const section = await openPicker(page, ipc, {});
    const radios = section.getByRole("radio");
    await expect(radios).toHaveCount(3);
    await expect(section.getByRole("radio", { name: /^Light/ })).toBeChecked();
    await expect(section).toContainText("Recommended");
    await expect(
      section.getByRole("radio", { name: /^None/ }),
    ).toHaveAccessibleDescription(/Exactly what you said/);

    await section.getByText("Medium", { exact: true }).click();
    await expect(section.getByRole("radio", { name: /^Medium/ })).toBeChecked();
    const sets = await ipc.calls("fork_set_setting");
    expect(sets.at(-1)?.args).toEqual({
      update: { key: "cleanup_level", value: "medium" },
    });

    // Keyboard: arrows move the choice inside the group.
    await section.getByRole("radio", { name: /^Medium/ }).focus();
    await page.keyboard.press("ArrowUp");
    await expect(section.getByRole("radio", { name: /^Light/ })).toBeChecked();
    expect((await ipc.calls("fork_set_setting")).at(-1)?.args).toEqual({
      update: { key: "cleanup_level", value: "light" },
    });
  });

  test("reports the last take and follows the result event", async ({
    page,
    ipc,
  }) => {
    const section = await openPicker(page, ipc, {
      fork_llm_last_cleanup: result(),
    });
    const last = section.getByTestId("fork-cleanup-last");
    await expect(last).toHaveAttribute("data-outcome", "applied");
    await expect(last).toContainText("Cleaned in 240 ms, 2 changes.");
    await expect(last).not.toContainText("Inserted without cleanup");

    await ipc.waitForListener("fork-cleanup-result");
    await ipc.emit(
      "fork-cleanup-result",
      result({
        take: 2,
        outcome: "guard",
        why: "guard",
        notice: true,
        chunks_applied: 0,
        guard_reasons: ["content_words"],
      }),
    );
    await expect(last).toHaveAttribute("data-notice", "true");
    await expect(last).toContainText("Inserted without cleanup");
    await expect(last).toContainText("did not pass the safety checks");
    await expect(last).toContainText("content_words");

    await ipc.emit(
      "fork-cleanup-result",
      result({
        take: 3,
        outcome: "skipped",
        why: "ai_chat",
        notice: false,
        engine: "none",
        chunks_total: 0,
        chunks_applied: 0,
        fixes: 0,
      }),
    );
    await expect(last).toHaveAttribute("data-notice", "false");
    await expect(last).toContainText("AI chat app: inserted as you said it.");

    await ipc.emit(
      "fork-cleanup-result",
      result({
        take: 4,
        outcome: "timeout",
        why: "timeout",
        notice: true,
        chunks_total: 3,
        chunks_applied: 2,
      }),
    );
    await expect(last).toContainText("Cleaned in part (2 of 3 paragraphs)");
    await expect(last).toContainText("cleanup took too long");
  });

  test("says when there was no dictation yet and when saving fails", async ({
    page,
    ipc,
  }) => {
    ipc.handle("fork_set_setting", () =>
      ipcError({ kind: "Internal", message: "disk full" }),
    );
    const section = await openPicker(page, ipc, {});
    await expect(section.getByTestId("fork-cleanup-last")).toContainText(
      "No dictation since the app started.",
    );
    await section.getByText("None", { exact: true }).click();
    await expect(section.getByRole("alert")).toContainText("disk full");
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe: no serious findings (${theme})`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme });
      const section = await openPicker(page, ipc, {
        fork_llm_last_cleanup: result({
          outcome: "guard",
          why: "guard",
          notice: true,
          chunks_applied: 0,
          guard_reasons: ["entity"],
        }),
      });
      await expect(section.getByTestId("fork-cleanup-last")).toContainText(
        "Inserted without cleanup",
      );
      await expectNoSeriousAxeFindings(page);
    });
  }
});
