// SPDX-License-Identifier: GPL-3.0-or-later
import { test, expect, NOT_READY } from "./fixtures";

const pingReply = {
  message: "pong",
  seq: 0,
  ready: true,
  debug_build: true,
  cli_debug: false,
};

test.describe("fork debug panel", () => {
  test("route opens the panel and shows fork command results", async ({
    page,
    ipc,
  }) => {
    ipc.respond({
      fork_ping: pingReply,
      fork_model_status: {
        known_models: 4,
        downloaded_model_ids: ["parakeet-tdt-0.6b-v3"],
      },
    });
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

    const calls = await ipc.commandNames();
    expect(calls).toContain("fork_ping");
    expect(calls).toContain("fork_model_status");

    await panel.getByRole("button", { name: "Close" }).click();
    await expect(panel).toBeHidden();
  });

  test("NotReady error is shown as a friendly message", async ({
    page,
    ipc,
  }) => {
    ipc.respond({ fork_ping: pingReply, fork_model_status: NOT_READY });
    await page.goto("/#/fork/debug");
    await expect(page.getByTestId("fork-debug-models")).toContainText(
      "Not ready yet",
    );
  });

  test("an event emitted by the test reaches the panel", async ({
    page,
    ipc,
  }) => {
    ipc.respond({ fork_ping: pingReply });
    await page.goto("/#/fork/debug");
    await ipc.waitForListener("fork-ping-event");
    // Let the panel's own pings and their events settle first, or one of
    // them can land after ours and replace it.
    await expect(page.getByTestId("fork-debug-ping")).toContainText('"pong"');
    await expect(page.getByTestId("fork-debug-event")).toContainText('"seq"');
    await page.waitForTimeout(100);
    expect(await ipc.emit("fork-ping-event", { seq: 4242 })).toBeGreaterThan(0);
    await expect(page.getByTestId("fork-debug-event")).toContainText("4242");
  });

  test("debug chord opens the panel only when the backend allows it", async ({
    page,
    ipc,
  }) => {
    ipc.respond({
      fork_ping: { ...pingReply, debug_build: false, cli_debug: false },
    });
    await page.goto("/");
    await page.waitForFunction(
      () => !!document.getElementById("fork-debug-root"),
    );
    await page.keyboard.press("ControlOrMeta+Shift+D");
    await ipc.expectCalled("fork_ping");
    await expect(page.getByTestId("fork-debug-panel")).toHaveCount(0);
  });

  test("debug chord opens the panel with --debug", async ({ page, ipc }) => {
    ipc.respond({
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
