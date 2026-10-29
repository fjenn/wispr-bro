// SPDX-License-Identifier: GPL-3.0-or-later
import { test, expect, type Page } from "@playwright/test";
import { installTauriMock, type MockResponses } from "./ipc-mock";

const pingReply = {
  message: "pong",
  seq: 0,
  ready: true,
  debug_build: true,
  cli_debug: false,
};

async function ipcCalls(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    (
      (window as unknown as { __WB_IPC_CALLS__: { cmd: string }[] })
        .__WB_IPC_CALLS__ ?? []
    ).map((c) => c.cmd),
  );
}

test.describe("fork debug panel", () => {
  test("route opens the panel and shows fork command results", async ({
    page,
  }) => {
    const responses: MockResponses = {
      fork_ping: pingReply,
      fork_model_status: {
        known_models: 4,
        downloaded_model_ids: ["parakeet-tdt-0.6b-v3"],
      },
    };
    await page.addInitScript(installTauriMock, responses);
    await page.goto("/#/fork/debug");

    const panel = page.getByTestId("fork-debug-panel");
    await expect(panel).toBeVisible();
    await expect(panel.getByRole("heading", { level: 2 })).toHaveText(
      "Fork debug panel",
    );
    await expect(page.getByTestId("fork-debug-ping")).toContainText('"pong"');
    await expect(page.getByTestId("fork-debug-models")).toContainText(
      "parakeet-tdt-0.6b-v3",
    );
    await expect(page.getByTestId("fork-debug-event")).toContainText('"seq"');

    const calls = await ipcCalls(page);
    expect(calls).toContain("fork_ping");
    expect(calls).toContain("fork_model_status");

    await panel.getByRole("button", { name: "Close" }).click();
    await expect(panel).toBeHidden();
  });

  test("NotReady error is shown as a friendly message", async ({ page }) => {
    await page.addInitScript(installTauriMock, {
      fork_ping: pingReply,
      fork_model_status: { __error: { kind: "NotReady" } },
    });
    await page.goto("/#/fork/debug");
    await expect(page.getByTestId("fork-debug-models")).toContainText(
      "Not ready yet",
    );
  });

  test("debug chord opens the panel only when the backend allows it", async ({
    page,
  }) => {
    await page.addInitScript(installTauriMock, {
      fork_ping: { ...pingReply, debug_build: false, cli_debug: false },
    });
    await page.goto("/");
    await page.waitForFunction(
      () => !!document.getElementById("fork-debug-root"),
    );
    await page.keyboard.press("ControlOrMeta+Shift+D");
    await expect
      .poll(async () => (await ipcCalls(page)).includes("fork_ping"))
      .toBe(true);
    await expect(page.getByTestId("fork-debug-panel")).toHaveCount(0);
  });

  test("debug chord opens the panel with --debug", async ({ page }) => {
    await page.addInitScript(installTauriMock, {
      fork_ping: { ...pingReply, debug_build: false, cli_debug: true },
    });
    await page.goto("/");
    await page.waitForFunction(
      () => !!document.getElementById("fork-debug-root"),
    );
    await page.keyboard.press("ControlOrMeta+Shift+D");
    await expect(page.getByTestId("fork-debug-panel")).toBeVisible();
  });
});
