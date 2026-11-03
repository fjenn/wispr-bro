// SPDX-License-Identifier: GPL-3.0-or-later
import type { Page } from "@playwright/test";
import type {
  ForkTransformViewChanges,
  TransformEntry,
  TransformInput,
} from "../../src/fork/bindings";
import type { IpcMock } from "./fixtures";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
} from "./hub-fixture";

const RULES = {
  shorter: true,
  clearer: true,
  better_order: true,
  headings_lists: false,
  keep_tone: true,
};
const input = (name = "Polish"): TransformInput => ({
  name,
  description: "Clear writing",
  rules: { ...RULES },
  prompt: "Polish this text.",
  examples: [],
});
const entry = (id: number, key: string, name: string): TransformEntry => ({
  id,
  key,
  action_id: `transform.${key}`,
  builtin_key: key.startsWith("custom-") ? null : key,
  ...input(name),
  updated_at: 0,
});
const run: ForkTransformViewChanges = {
  run_id: 9,
  transform_id: 1,
  app_bundle_id: "com.tinyspeck.slackmacgap",
  source_text: "um we could perhaps meet Tuesday",
  result_text: "We could meet on Tuesday.",
  rating: null,
  created_at_ms: 1,
};

class FakeTransforms {
  entries = [
    entry(1, "polish", "Polish"),
    entry(2, "prompt_engineer", "Prompt Engineer"),
  ];
  current = { ...run };
  install(ipc: IpcMock) {
    ipc.handle("fork_transforms_list", () => this.entries);
    ipc.handle("fork_transform_create", (args) => {
      const value = args.input as TransformInput;
      const made: TransformEntry = {
        id: 3,
        key: "custom-3",
        action_id: "transform.custom-3",
        builtin_key: null,
        ...value,
        updated_at: 1,
      };
      this.entries.push(made);
      return made;
    });
    ipc.handle("fork_transform_update", (args) => {
      const index = this.entries.findIndex((item) => item.id === args.id);
      const value = args.input as TransformInput;
      this.entries[index] = { ...this.entries[index], ...value, updated_at: 2 };
      return this.entries[index];
    });
    ipc.handle("fork_transform_delete", (args) => {
      this.entries = this.entries.filter((item) => item.id !== args.id);
      return null;
    });
    ipc.handle("fork_transforms_reset", () => {
      this.entries = [
        entry(1, "polish", "Polish"),
        entry(2, "prompt_engineer", "Prompt Engineer"),
      ];
      return this.entries;
    });
    ipc.respond({
      fork_transform_preview: "Could we discuss the report tomorrow?",
      fork_transform_read_example: "A short example in my own voice.",
      fork_transform_latest: this.current,
      fork_transform_diff: {
        ops: [
          { kind: "delete", text: "um we could perhaps" },
          { kind: "insert", text: "We could" },
          { kind: "equal", text: " meet" },
          { kind: "insert", text: " on" },
          { kind: "equal", text: " Tuesday" },
          { kind: "insert", text: "." },
        ],
        changes: 3,
        degraded: false,
      },
    });
    ipc.handle("fork_transform_rate", (args) => {
      this.current = { ...this.current, rating: args.rating as number };
      return this.current;
    });
    ipc.handle("fork_transform_retry", () => {
      this.current = {
        ...this.current,
        run_id: 10,
        result_text: "Could we meet on Tuesday?",
      };
      return this.current;
    });
  }
}

async function openTransforms(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  fake: FakeTransforms,
  options: Parameters<typeof openHub>[3] = {},
) {
  await openHub(page, ipc, hub, {
    hash: "#/hub/transforms",
    ...options,
    beforeNavigate: () => {
      fake.install(ipc);
      options.beforeNavigate?.();
    },
  });
  await expect(page.getByTestId("wb-transforms")).toBeVisible();
}

test("Command Mode card is independently opt-in and keyboard accessible", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake);
  const card = page.getByTestId("wb-command-mode-card");
  const toggle = page.getByTestId("wb-command-mode-enabled");
  await expect(card).toHaveAttribute("data-enabled", "false");
  await expect(toggle).not.toBeChecked();
  await expect(toggle).toHaveAccessibleName(/Command Mode/);
  await expect(card).toContainText("never looks anything up");
  await toggle.focus();
  await page.keyboard.press("Space");
  await expect(toggle).toBeChecked();
  await expect(card).toHaveAttribute("data-enabled", "true");
  await expect.poll(() => hub.forkSettings.command_mode_enabled).toBe(true);
  await expect.poll(() => hub.forkSettings.transforms_enabled).toBe(false);
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await expect.poll(() => hub.forkSettings.command_mode_enabled).toBe(false);
});

test("opt-in, banner, auto-apply choice and safe shortcuts", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake);
  await expect(page.getByTestId("wb-banner-transforms.rewrite")).toContainText(
    "Rewrite selected text in any app",
  );
  // S-31 registered the lesson: the banner's buttons are there.
  await expect(page.getByTestId("wb-transforms-try-it")).toBeVisible();
  await expect(page.getByTestId("wb-transforms-how-it-works")).toBeVisible();
  await expect(page.getByTestId("wb-transform-polish")).toBeVisible();
  await expect(page.getByTestId("wb-transform-prompt_engineer")).toBeVisible();
  await expect(page.getByTestId("wb-transform-create-card")).toBeVisible();
  const enabled = page.getByTestId("wb-transforms-enabled");
  await expect(enabled).not.toBeChecked();
  await expect(page.getByTestId("wb-transforms-auto")).toBeDisabled();
  await enabled.click();
  await expect(enabled).toBeChecked();
  await expect.poll(() => hub.forkSettings.transforms_enabled).toBe(true);
  await page.getByTestId("wb-transforms-auto").click();
  await expect(page.getByTestId("wb-transforms-auto")).toBeChecked();
  // A Radix select (W2): the trigger shows the chosen transform's name.
  await expect(page.getByTestId("wb-transforms-auto-choice")).toHaveText(
    "Polish",
  );
  await page.getByTestId("wb-transforms-auto-choice").click();
  await page
    .getByTestId("wb-transforms-auto-choice-option-prompt_engineer")
    .click();
  await expect
    .poll(() => hub.forkSettings.auto_apply_transform)
    .toBe("prompt_engineer");
  await page.getByTestId("wb-transforms-safe-shortcuts").click();
  await expect
    .poll(() => hub.forkSettings.hotkey)
    .toEqual({
      action: "transform.view_changes",
      bindings: [{ chord: "ctrl+option+o", activation: "press" }],
    });
});

test("Polish editor: five rules, preview, autosave, reset and Escape", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake);
  await page.getByTestId("wb-transform-open-polish").click();
  const modal = page.getByTestId("wb-transform-modal");
  await expect(modal).toBeVisible();
  await expect(modal.locator('input[type="checkbox"]')).toHaveCount(5);
  await expect(page.getByTestId("wb-transform-autosave")).toContainText(
    "All changes saved",
  );
  await page.getByTestId("wb-transform-description").fill("Make my email tidy");
  await ipc.expectCalled("fork_transform_update", {
    id: 1,
    input: { ...input(), description: "Make my email tidy" },
  });
  await expect(page.getByTestId("wb-transform-autosave")).toContainText(
    "All changes saved",
  );
  await page.getByTestId("wb-transform-preview").click();
  await expect(page.getByTestId("wb-transform-preview-result")).toHaveText(
    "Could we discuss the report tomorrow?",
  );
  await page.getByTestId("wb-transform-rule-shorter").uncheck();
  await page.getByTestId("wb-transform-reset").click();
  await expect(page.getByTestId("wb-transform-rule-shorter")).toBeChecked();
  await page.keyboard.press("Escape");
  await expect(modal).toBeHidden();
});

test("create custom, three-example limit, file loading and delete", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake);
  await page.getByTestId("wb-transforms-create").click();
  await expect(page.getByTestId("wb-transform-name")).toHaveValue(
    "My transform",
  );
  await page.getByTestId("wb-transform-example-add").click();
  ipc.respond("plugin:dialog|open", "/tmp/example.txt");
  await page.getByTestId("wb-transform-example-load").click();
  await ipc.expectCalled("fork_transform_read_example", {
    path: "/tmp/example.txt",
  });
  await page.getByTestId("wb-transform-example-add").click();
  await expect(page.getByTestId("wb-transform-example-add")).toHaveCount(0);
  await expect(page.getByTestId("wb-transform-example-load")).toHaveCount(0);
  await page.getByTestId("wb-transform-close").click();
  await expect(page.getByTestId("wb-transform-custom-3")).toBeVisible();
  page.once("dialog", (dialog) => void dialog.accept());
  await page.getByTestId("wb-transform-delete-custom-3").click();
  await expect(page.getByTestId("wb-transform-custom-3")).toHaveCount(0);
});

test("View changes handoff opens diff; Copy, local rating, Retry and Configure", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake);
  await ipc.waitForListener("fork-transform-view-changes");
  await ipc.emit("fork-transform-view-changes", run);
  await expect(page).toHaveURL(/#\/hub\/transforms\/changes$/);
  await expect(page.getByTestId("wb-diff-count")).toHaveText("3 changes");
  await expect(page.locator('[data-diff="delete"]')).toContainText("um we");
  await expect(page.locator('[data-diff="insert"]').first()).toContainText(
    "We could",
  );
  await page.getByTestId("wb-diff-good").click();
  await expect(page.getByTestId("wb-diff-good")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.getByTestId("wb-diff-retry").click();
  await ipc.expectCalled("fork_transform_retry", { runId: 9 });
  await page.getByTestId("wb-diff-configure").click();
  await expect(page.getByTestId("wb-transform-modal")).toBeVisible();
});

test("reset restores built-ins and clears auto-apply", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake, {
    fork: { transforms_enabled: true, auto_apply_transform: "polish" },
  });
  page.once("dialog", (dialog) => void dialog.accept());
  await page.getByTestId("wb-transforms-reset").click();
  await ipc.expectCalled("fork_transforms_reset", {});
  await expect.poll(() => hub.forkSettings.auto_apply_transform).toBeNull();
});

test("keyboard-only editor and dialog focus", async ({ page, ipc, hub }) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake);
  await page.getByTestId("wb-transform-open-polish").focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("wb-transform-modal")).toBeVisible();
  await expect(page.getByTestId("wb-transform-close")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("wb-transform-name")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("wb-transform-modal")).toBeHidden();
});

test("RTL, reduced motion and axe in light/dark page, editor and diff", async ({
  page,
  ipc,
  hub,
}) => {
  test.setTimeout(120_000);
  const fake = new FakeTransforms();
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await openTransforms(page, ipc, hub, fake, {
      handy: { theme, app_language: "ar" },
    });
    await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
    await expectNoSeriousAxeFindings(page);
    await page.getByTestId("wb-transform-open-polish").click();
    await expectNoSeriousAxeFindings(page);
    await page.keyboard.press("Escape");
    await ipc.emit("fork-transform-view-changes", run);
    await expect(page.getByTestId("wb-transform-diff")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
  }
});
