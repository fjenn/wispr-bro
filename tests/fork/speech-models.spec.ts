// SPDX-License-Identifier: GPL-3.0-or-later
// S-07b: Settings › Advanced › Speech models (on the debug panel until
// S-14), over mocked IPC: the policy's states from the status event,
// "Choose the model automatically", the slower-Mac note, axe in light and
// dark, RTL and keyboard-only use.
import type { Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { test, expect, type IpcMock } from "./fixtures";
import { openSettingsModal } from "./settings-modal-fixture";

const pingReply = {
  message: "pong",
  seq: 0,
  ready: true,
  debug_build: true,
  cli_debug: false,
};

const policy = (over: Record<string, unknown> = {}) => ({
  automatic: true,
  phase: "settled",
  current_model: "Parakeet TDT-CTC 110M",
  next_model: null,
  percent: null,
  slower: false,
  languages: ["en", "nl"],
  machine: {
    chip: { kind: "apple_silicon", generation: 1, tier: "base" },
    ram_gb: 8,
    free_disk_gb: 212,
  },
  ...over,
});

const SECTION = "wb-advanced-models";

async function openPanel(
  page: Page,
  ipc: IpcMock,
  responses: Record<string, unknown> = {},
) {
  await openSettingsModal(page, ipc, "advanced", {
    fork_model_policy_status: policy(),
    fork_models_hidden: [],
    ...responses,
  });
  const section = page.getByTestId(SECTION);
  await expect(section).toBeVisible({ timeout: 15_000 });
  return section;
}

async function emitPolicy(ipc: IpcMock, payload: unknown) {
  await ipc.waitForListener("fork-model-policy");
  await ipc.emit("fork-model-policy", payload);
}

async function expectAccessible(page: Page) {
  await page.waitForFunction(() =>
    document.getAnimations().every((a) => a.playState !== "running"),
  );
  const results = await new AxeBuilder({ page })
    .include(`[data-testid="${SECTION}"]`)
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  const serious = results.violations.filter(
    (v) => v.impact === "serious" || v.impact === "critical",
  );
  expect(
    serious.map((v) => ({
      id: v.id,
      nodes: v.nodes.map((n) => n.target.join(" ")).slice(0, 5),
    })),
  ).toEqual([]);
}

test.describe("Speech models", () => {
  test("shows the model in use, this Mac and the languages", async ({
    page,
    ipc,
  }) => {
    const section = await openPanel(page, ipc);
    await expect(section.getByTestId("fork-models-current")).toHaveText(
      "In use: Parakeet TDT-CTC 110M",
    );
    await expect(section.getByTestId("fork-models-phase")).toHaveAttribute(
      "data-phase",
      "settled",
    );
    await expect(section.getByTestId("fork-models-machine")).toContainText(
      "Apple M1, 8 GB memory",
    );
    await expect(section.getByTestId("fork-models-machine")).toContainText(
      "212 GB free",
    );
    await expect(section).toContainText("English");
    await expect(section).toContainText("Dutch");
    await expect(section.getByTestId("fork-models-automatic")).toBeChecked();
  });

  test("follows a background download to the swap", async ({ page, ipc }) => {
    const section = await openPanel(page, ipc, {
      fork_model_policy_status: policy({
        phase: "downloading",
        next_model: "Parakeet TDT 0.6B v3",
        percent: 0,
      }),
    });
    const phase = section.getByTestId("fork-models-phase");
    await expect(phase).toHaveAttribute("data-phase", "downloading");
    await expect(section.getByTestId("fork-models-next")).toHaveText(
      "Next: Parakeet TDT 0.6B v3",
    );
    await emitPolicy(
      ipc,
      policy({
        phase: "downloading",
        next_model: "Parakeet TDT 0.6B v3",
        percent: 42,
      }),
    );
    await expect(phase).toContainText("42%");
    await expect(section.getByRole("progressbar")).toBeVisible();
    await emitPolicy(
      ipc,
      policy({ phase: "waiting_to_swap", next_model: "Parakeet TDT 0.6B v3" }),
    );
    await expect(phase).toContainText("when you are not dictating");
    await expect(section.getByRole("progressbar")).toHaveCount(0);
    await emitPolicy(
      ipc,
      policy({ phase: "settled", current_model: "Parakeet TDT 0.6B v3" }),
    );
    await expect(section.getByTestId("fork-models-current")).toHaveText(
      "In use: Parakeet TDT 0.6B v3",
    );
    await expect(section.getByTestId("fork-models-next")).toHaveCount(0);
  });

  test("turning automatic off pins, on again unpins", async ({ page, ipc }) => {
    let automatic = true;
    ipc.handle("fork_model_policy_set_automatic", (args) => {
      automatic = (args as { on: boolean }).on;
      return policy({ automatic });
    });
    const section = await openPanel(page, ipc);
    const box = section.getByTestId("fork-models-automatic");
    await box.click();
    await expect(box).not.toBeChecked();
    await expect(section.getByTestId("fork-models-phase")).toContainText(
      "You chose this model yourself",
    );
    await ipc.expectCalled("fork_model_policy_set_automatic", { on: false });
    await box.click();
    await expect(box).toBeChecked();
    await ipc.expectCalled("fork_model_policy_set_automatic", { on: true });
  });

  test("says when this Mac is slower", async ({ page, ipc }) => {
    const section = await openPanel(page, ipc, {
      fork_model_policy_status: policy({ slower: true }),
    });
    await expect(section.getByTestId("fork-models-slower")).toContainText(
      "may be slower on this Mac",
    );
  });

  test("names no model before the policy reports", async ({ page, ipc }) => {
    const section = await openPanel(page, ipc, {
      fork_model_policy_status: policy({
        phase: "checking",
        current_model: null,
        machine: null,
        languages: [],
      }),
    });
    await expect(section.getByTestId("fork-models-current")).toHaveText(
      "No speech model yet",
    );
  });

  test("keyboard only: toggle and the model list disclosure", async ({
    page,
    ipc,
  }) => {
    ipc.handle("fork_model_policy_set_automatic", (args) =>
      policy({ automatic: (args as { on: boolean }).on }),
    );
    const section = await openPanel(page, ipc);
    const box = section.getByTestId("fork-models-automatic");
    await box.focus();
    await expect(box).toBeFocused();
    await page.keyboard.press("Space");
    await expect(box).not.toBeChecked();
    await page.keyboard.press("Tab");
    const toggle = section.getByTestId("fork-models-list-toggle");
    await expect(toggle).toBeFocused();
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe (${theme})`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme });
      await openPanel(page, ipc, {
        fork_model_policy_status: policy({
          phase: "downloading",
          next_model: "Parakeet TDT 0.6B v3",
          percent: 30,
          slower: true,
        }),
      });
      await expectAccessible(page);
      if (process.env.WB_SHOTS_DIR) {
        await page.getByTestId(SECTION).screenshot({
          path: `${process.env.WB_SHOTS_DIR}/speech-models-${theme}.png`,
        });
      }
    });
  }

  test("RTL: the panel mirrors and stays accessible", async ({ page, ipc }) => {
    const section = await openPanel(page, ipc);
    await page.evaluate(() => {
      document.documentElement.dir = "rtl";
      document
        .querySelector("[data-testid=wb-settings-modal]")
        ?.setAttribute("dir", "rtl");
    });
    await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
    const box = await section
      .getByTestId("fork-models-automatic")
      .boundingBox();
    const label = await section
      .getByText("Choose the model automatically")
      .boundingBox();
    expect(box && label && box.x > label.x).toBe(true);
    await expectAccessible(page);
  });
});
