// SPDX-License-Identifier: GPL-3.0-or-later
// S-12b: the Apple Intelligence row of the AI engine panel (debug panel,
// mocked IPC): every availability reason, the role, the counts, Try it on
// Apple's model while the cleanup model downloads (keyboard only), RTL, and
// axe in light and dark.
import type { Page } from "@playwright/test";
import { test, expect, type IpcMock } from "./fixtures";
import { expectNoSeriousAxeFindings } from "./hub-fixture";
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

const llm = (over: Record<string, unknown> = {}) => ({
  state: "ready",
  model_name: "Muesli 2B v12",
  downloaded_bytes: 0,
  total_bytes: 1_274_396_096,
  server,
  error: null,
  ...over,
});

const apple = (over: Record<string, unknown> = {}) => ({
  availability: "available",
  role: "fallback",
  context_tokens: 4096,
  requests: 0,
  refusals: 0,
  rate_limited: 0,
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

test.describe("AI engine: Apple Intelligence", () => {
  const reasons: [string, string][] = [
    ["device_not_eligible", "can't run Apple Intelligence"],
    ["not_enabled", "off in System Settings"],
    ["model_not_ready", "still getting its model ready"],
    ["unsupported", "needs macOS 26"],
    ["unknown", "isn't available right now"],
  ];
  for (const [availability, text] of reasons) {
    test(`says why it is not used: ${availability}`, async ({ page, ipc }) => {
      const section = await openPanel(page, ipc, {
        fork_llm_status: llm(),
        fork_llm_apple_status: apple({ availability, context_tokens: 0 }),
      });
      const row = section.getByTestId("fork-ai-engine-apple");
      await expect(row).toHaveAttribute("data-availability", availability);
      await expect(row).toContainText(text);
      await expect(row.getByTestId("fork-ai-engine-apple-counts")).toHaveCount(
        0,
      );
    });
  }

  test("ready as the stand-in, with counts since launch", async ({
    page,
    ipc,
  }) => {
    const section = await openPanel(page, ipc, {
      fork_llm_status: llm(),
      fork_llm_apple_status: apple({
        requests: 12,
        refusals: 1,
        rate_limited: 0,
      }),
    });
    const row = section.getByTestId("fork-ai-engine-apple");
    await expect(
      row.getByRole("heading", { name: "Apple Intelligence" }),
    ).toBeVisible();
    await expect(row).toContainText("can't run yet");
    await expect(row.getByTestId("fork-ai-engine-apple-counts")).toHaveText(
      "Since the app started: 12 requests, 1 declined, 0 throttled.",
    );
  });

  test("Try it runs on Apple's model while the cleanup model downloads (keyboard only)", async ({
    page,
    ipc,
  }) => {
    const section = await openPanel(page, ipc, {
      fork_llm_status: llm({ state: "downloading", downloaded_bytes: 1000 }),
      fork_llm_apple_status: apple(),
      fork_llm_try: {
        cleaned: "Meet on Thursday.",
        cleanup_ms: 640,
        start_ms: 0,
        level: "light",
        outcome: "applied",
        why: null,
        guard_reasons: [],
        engine: "apple",
      },
    });
    const tryButton = section.getByTestId("fork-ai-engine-try");
    await expect(tryButton).toBeEnabled();
    const input = section.getByTestId("fork-ai-engine-input");
    await input.focus();
    await page.keyboard.press("ControlOrMeta+a");
    await page.keyboard.type("um meet on thursday");
    await page.keyboard.press("Tab");
    await expect(tryButton).toBeFocused();
    await page.keyboard.press("Enter");
    const result = section.getByTestId("fork-ai-engine-result");
    await expect(result).toContainText("Meet on Thursday.");
    await expect(section.getByTestId("fork-ai-engine-engine")).toHaveText(
      "Ran on Apple Intelligence.",
    );
    expect(await ipc.calls("fork_llm_try")).toHaveLength(1);
    // The counts are read again after Try it.
    expect(
      (await ipc.calls("fork_llm_apple_status")).length,
    ).toBeGreaterThanOrEqual(2);
  });

  test("without either engine Try it stays off", async ({ page, ipc }) => {
    const section = await openPanel(page, ipc, {
      fork_llm_status: llm({ state: "downloading", downloaded_bytes: 1000 }),
      fork_llm_apple_status: apple({ availability: "not_enabled" }),
    });
    await expect(section.getByTestId("fork-ai-engine-apple")).toBeVisible();
    await expect(section.getByTestId("fork-ai-engine-try")).toBeDisabled();
  });

  test("RTL: the row follows the page direction and stays accessible", async ({
    page,
    ipc,
  }) => {
    const section = await openPanel(page, ipc, {
      fork_llm_status: llm(),
      fork_llm_apple_status: apple({ requests: 3 }),
    });
    await page.evaluate(() => {
      document.documentElement.dir = "rtl";
      document
        .querySelector("[data-testid=wb-settings-modal]")
        ?.setAttribute("dir", "rtl");
    });
    const row = section.getByTestId("fork-ai-engine-apple");
    await expect(row).toBeVisible();
    expect(await row.evaluate((el) => getComputedStyle(el).direction)).toBe(
      "rtl",
    );
    const box = await row.boundingBox();
    const panel = await section.boundingBox();
    expect(
      box &&
        panel &&
        box.x >= panel.x - 1 &&
        box.x + box.width <= panel.x + panel.width + 1,
    ).toBe(true);
    await expectNoSeriousAxeFindings(page);
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe: no serious findings (${theme})`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme });
      const section = await openPanel(page, ipc, {
        fork_llm_status: llm(),
        fork_llm_apple_status: apple({ requests: 4, refusals: 1 }),
      });
      await expect(
        section.getByTestId("fork-ai-engine-apple-counts"),
      ).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }
});
