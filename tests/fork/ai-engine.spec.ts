// SPDX-License-Identifier: GPL-3.0-or-later
// S-10: Settings › Advanced › AI engine (on the debug panel until S-14),
// over mocked IPC: states, the status event, Try it, errors and Retry.
import type { Page } from "@playwright/test";
import { test, expect, ipcError, type IpcMock } from "./fixtures";
import { openSettingsModal } from "./settings-modal-fixture";

const pingReply = {
  message: "pong",
  seq: 0,
  ready: true,
  debug_build: true,
  cli_debug: false,
};

const server = {
  phase: "stopped",
  device: "gpu",
  transport: null,
  ready_ms: null,
  restarts: 0,
  last_error: null,
};

const status = (over: Record<string, unknown> = {}) => ({
  state: "ready",
  model_name: "Muesli 2B v12",
  downloaded_bytes: 0,
  total_bytes: 1_274_396_096,
  server,
  error: null,
  ...over,
});

async function openPanel(
  page: Page,
  ipc: IpcMock,
  responses: Record<string, unknown>,
) {
  await openSettingsModal(page, ipc, "advanced", responses);
  const section = page.getByTestId("wb-advanced-ai");
  await expect(section).toBeVisible({ timeout: 15_000 });
  return section;
}

async function emitStatus(ipc: IpcMock, payload: unknown) {
  await ipc.waitForListener("fork-llm-status");
  await ipc.emit("fork-llm-status", payload);
}

test.describe("AI engine", () => {
  test("shows the download, then Ready from the status event, then Try it", async ({
    page,
    ipc,
  }) => {
    const section = await openPanel(page, ipc, {
      fork_llm_status: status({
        state: "downloading",
        downloaded_bytes: 637_198_048,
      }),
      fork_llm_try: {
        cleaned: "Meet on Thursday.",
        cleanup_ms: 120,
        start_ms: 610,
        level: "light",
        outcome: "applied",
        why: null,
        guard_reasons: [],
      },
    });
    const state = section.getByTestId("fork-ai-engine-state");
    await expect(state).toHaveAttribute("data-state", "downloading");
    await expect(state).toContainText("50%");
    await expect(section.getByRole("progressbar")).toBeVisible();
    const tryButton = section.getByTestId("fork-ai-engine-try");
    await expect(tryButton).toBeDisabled();

    await emitStatus(ipc, status());
    await expect(state).toHaveAttribute("data-state", "ready");
    await expect(state).toHaveText("Ready");
    await expect(section.getByRole("progressbar")).toHaveCount(0);
    await expect(section).toContainText("Muesli 2B v12");

    const input = section.getByTestId("fork-ai-engine-input");
    await expect(input).not.toHaveValue("");
    await input.fill("um meet on thursday");
    await tryButton.click();
    const result = section.getByTestId("fork-ai-engine-result");
    await expect(result).toContainText("Meet on Thursday.");
    await expect(result).toContainText("120 ms");
    await expect(result).toContainText("610 ms");
    const tries = await ipc.calls("fork_llm_try");
    expect(tries).toHaveLength(1);
    expect(tries[0].args).toEqual({ text: "um meet on thursday" });
  });

  test("explains a failed download with Retry, and low memory", async ({
    page,
    ipc,
  }) => {
    const section = await openPanel(page, ipc, {
      fork_llm_status: status({
        state: "download_failed",
        error: "stalled for 30 s",
      }),
      fork_llm_retry: status({ state: "verifying" }),
      fork_llm_try: ipcError({ kind: "LowMemory" }),
    });
    await expect(section.getByRole("alert")).toContainText("stalled for 30 s");
    await section.getByRole("button", { name: "Retry" }).click();
    await expect(section.getByTestId("fork-ai-engine-state")).toHaveAttribute(
      "data-state",
      "verifying",
    );
    expect(await ipc.calls("fork_llm_retry")).toHaveLength(1);

    await emitStatus(ipc, status());
    await section.getByTestId("fork-ai-engine-try").click();
    await expect(section.getByRole("alert")).toContainText(
      "Not enough free memory",
    );
  });

  test("Try it says when a dictation would go in without cleanup", async ({
    page,
    ipc,
  }) => {
    const section = await openPanel(page, ipc, {
      fork_llm_status: status(),
      fork_llm_try: {
        cleaned: "what is two plus two",
        cleanup_ms: 180,
        start_ms: 0,
        level: "light",
        outcome: "guard",
        why: "guard",
        guard_reasons: ["assistant_marker"],
      },
    });
    await section
      .getByTestId("fork-ai-engine-input")
      .fill("what is two plus two");
    await section.getByTestId("fork-ai-engine-try").click();
    const result = section.getByTestId("fork-ai-engine-result");
    await expect(result).toContainText("what is two plus two");
    await expect(section.getByTestId("fork-ai-engine-why")).toContainText(
      "did not pass the safety checks",
    );
  });

  test("unsupported Macs say so and offer no Try it", async ({ page, ipc }) => {
    const section = await openPanel(page, ipc, {
      fork_llm_status: status({ state: "unsupported", model_name: null }),
    });
    await expect(section.getByTestId("fork-ai-engine-state")).toContainText(
      "not available on this Mac",
    );
    await expect(section.getByTestId("fork-ai-engine-try")).toBeDisabled();
  });
});
