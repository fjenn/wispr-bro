// SPDX-License-Identifier: GPL-3.0-or-later
import type { Page } from "@playwright/test";
import type {
  ForkTransformViewChanges,
  LiteralEdit,
  TransformEntry,
  TransformInput,
  TransformPlan,
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
  replacements: [],
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

/** A stand-in for the Rust planner: one known exact prompt plus the rows. */
function exactPlan(value: TransformInput): TransformPlan {
  const promptExact = /^replace all a and e with 0 and 1$/i.test(
    value.prompt.trim(),
  );
  const edits: LiteralEdit[] = [
    ...(promptExact
      ? (["A", "E"] as const).map(
          (find, index): LiteralEdit => ({
            kind: "replace",
            find: { kind: "text", value: find },
            with: String(index),
            match_case: false,
          }),
        )
      : []),
    ...value.replacements
      .filter((rule) => rule.find)
      .map(
        (rule): LiteralEdit => ({
          kind: "replace",
          find: { kind: "text", value: rule.find },
          with: rule.replace,
          match_case: rule.match_case,
        }),
      ),
  ];
  const anyRule = Object.values(value.rules).some(Boolean);
  return {
    edits,
    prompt_exact: promptExact,
    uses_model: !promptExact && (value.prompt.trim() !== "" || anyRule),
  };
}

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
    ipc.handle("fork_transform_preview", (args) => {
      const value = args.input as TransformInput;
      if (exactPlan(value).edits.length && !exactPlan(value).uses_model)
        return { text: "Pl10s1 s1nd it", outcome: "changed", exact: true };
      if (value.prompt.startsWith("Make it shorter"))
        return { text: null, outcome: "not_applied", exact: false };
      return {
        text: "Could we discuss the report tomorrow?",
        outcome: "changed",
        exact: false,
      };
    });
    ipc.handle("fork_transform_plan", (args) =>
      exactPlan(args.input as TransformInput),
    );
    ipc.respond({
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
  // XF2: secondary controls live behind the page's settings button.
  await page.getByTestId("wb-transforms-settings").click();
  const card = page.getByTestId("wb-command-mode-card");
  const toggle = page.getByTestId("wb-command-mode-enabled");
  await expect(toggle).not.toBeChecked();
  await expect(toggle).toHaveAccessibleName(/Command Mode/);
  await expect(card).toContainText("say what to write or change");
  await toggle.focus();
  await page.keyboard.press("Space");
  await expect(toggle).toBeChecked();
  await expect.poll(() => hub.forkSettings.command_mode_enabled).toBe(true);
  await expect.poll(() => hub.forkSettings.transforms_enabled).toBe(false);
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await expect.poll(() => hub.forkSettings.command_mode_enabled).toBe(false);
});

test("simple page: opt-in, list, settings with auto-apply and safe shortcuts", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake);
  // XF2: a title, one line with the lesson links, the list and one button.
  await expect(page.getByTestId("wb-banner-transforms.rewrite")).toHaveCount(0);
  await expect(page.getByTestId("wb-transforms")).toContainText(
    "press a shortcut to rewrite it",
  );
  await expect(page.getByTestId("wb-transforms-try-it")).toBeVisible();
  await expect(page.getByTestId("wb-transforms-how-it-works")).toBeVisible();
  await expect(page.getByTestId("wb-transform-polish")).toBeVisible();
  await expect(page.getByTestId("wb-transform-prompt_engineer")).toBeVisible();
  await expect(page.getByTestId("wb-transforms-create")).toHaveText(
    "New transform",
  );
  await expect(page.getByTestId("wb-command-mode-card")).toHaveCount(0);
  await expect(page.getByTestId("wb-transforms-auto")).toHaveCount(0);
  const enabled = page.getByTestId("wb-transforms-enabled");
  await expect(enabled).not.toBeChecked();
  await enabled.click();
  await expect(enabled).toBeChecked();
  await expect.poll(() => hub.forkSettings.transforms_enabled).toBe(true);
  await page.getByTestId("wb-transforms-settings").click();
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

test("Polish editor: one instruction, five option chips, preview, autosave, reset and Escape", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake);
  await page.getByTestId("wb-transform-open-polish").click();
  const modal = page.getByTestId("wb-transform-modal");
  await expect(modal).toBeVisible();
  await expect(
    page.getByTestId("wb-transform-rules").locator("button[aria-pressed]"),
  ).toHaveCount(5);
  await expect(page.getByTestId("wb-transform-description")).toHaveCount(0);
  await expect(page.getByTestId("wb-transform-prompt")).toHaveValue(
    "Polish this text.",
  );
  await expect(page.getByTestId("wb-transform-autosave")).toContainText(
    "All changes saved",
  );
  await page.getByTestId("wb-transform-prompt").fill("Make my email tidy");
  await ipc.expectCalled("fork_transform_update", {
    id: 1,
    input: { ...input(), prompt: "Make my email tidy" },
  });
  await expect(page.getByTestId("wb-transform-autosave")).toContainText(
    "All changes saved",
  );
  await page.getByTestId("wb-transform-preview").click();
  await expect(page.getByTestId("wb-transform-preview-result")).toHaveText(
    "Could we discuss the report tomorrow?",
  );
  const shorter = page.getByTestId("wb-transform-rule-shorter");
  await shorter.click();
  await expect(shorter).toHaveAttribute("aria-pressed", "false");
  await page.getByTestId("wb-transform-reset").click();
  await expect(shorter).toHaveAttribute("aria-pressed", "true");
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
  // XF2: an empty instruction box, no hidden default prompt.
  await expect(page.getByTestId("wb-transform-prompt")).toHaveValue("");
  await expect(page.getByTestId("wb-transform-prompt")).toHaveAttribute(
    "placeholder",
    /replace all/,
  );
  await expect(page.getByTestId("wb-transform-example-add")).toBeHidden();
  await page.getByTestId("wb-transform-more").click();
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
  await page.getByTestId("wb-transforms-settings").click();
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

test("RTL, reduced motion and axe in light/dark page, settings, editor and diff", async ({
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
    await page.getByTestId("wb-transforms-settings").click();
    await expect(
      page.getByTestId("wb-transforms-settings-dialog"),
    ).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await page.keyboard.press("Escape");
    await expect(
      page.getByTestId("wb-transforms-settings-dialog"),
    ).toBeHidden();
    await page.getByTestId("wb-transform-open-polish").click();
    await expectNoSeriousAxeFindings(page);
    await page.keyboard.press("Escape");
    await ipc.emit("fork-transform-view-changes", run);
    await expect(page.getByTestId("wb-transform-diff")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
  }
});

test("custom transform: exact edits run without AI, find and replace rows, honest preview", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeTransforms();
  await openTransforms(page, ipc, hub, fake);
  await page.getByTestId("wb-transforms-create").click();
  // A new transform starts with no options and an empty instruction.
  await ipc.expectCalled("fork_transform_create", {
    input: {
      prompt: "",
      description: "",
      rules: {
        shorter: false,
        clearer: false,
        better_order: false,
        headings_lists: false,
        keep_tone: false,
      },
      replacements: [],
    },
  });
  await page.getByTestId("wb-transform-rule-shorter").click();
  await page
    .getByTestId("wb-transform-prompt")
    .fill("Replace all A and E with 0 and 1");
  // XF2: no explanations in the editor; the preview says it ran exactly.
  await expect(page.getByTestId("wb-transform-plan")).toHaveCount(0);
  await page.getByTestId("wb-transform-preview").click();
  await expect(page.getByTestId("wb-transform-preview-exact")).toHaveText(
    "Exact · no AI",
  );
  await expect(page.getByTestId("wb-transform-preview-result")).toHaveText(
    "Pl10s1 s1nd it",
  );

  // A free-form instruction plus an exact row.
  await page.getByTestId("wb-transform-prompt").fill("Make it shorter");
  await page.getByTestId("wb-transform-more").click();
  await page.getByTestId("wb-transform-replace-add").click();
  await page.getByTestId("wb-transform-replace-find-0").fill("Bob");
  await page.getByTestId("wb-transform-replace-with-0").fill("Robert");
  await page.getByTestId("wb-transform-replace-case-0").check();
  await ipc.expectCalled("fork_transform_update", {
    id: 3,
    input: {
      prompt: "Make it shorter",
      replacements: [{ find: "Bob", replace: "Robert", match_case: true }],
    },
  });
  await page.getByTestId("wb-transform-preview").click();
  const outcome = page.getByTestId("wb-transform-preview-outcome");
  await expect(outcome).toHaveAttribute("data-outcome", "not_applied");
  await expect(outcome).toContainText("couldn't do this one");
  await expect(page.getByTestId("wb-transform-preview-result")).toHaveCount(0);
  // The editor reports the preview, not a save error.
  await expect(page.getByTestId("wb-transform-autosave")).not.toContainText(
    "Couldn't save",
  );
  await page.getByTestId("wb-transform-replace-remove-0").click();
  await expect(page.getByTestId("wb-transform-replace-row-0")).toHaveCount(0);
});

test("find and replace rows: keyboard only, RTL and axe in light/dark", async ({
  page,
  ipc,
  hub,
}) => {
  test.setTimeout(180_000);
  const fake = new FakeTransforms();
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await openTransforms(page, ipc, hub, fake, {
      handy: { theme, app_language: "ar" },
    });
    await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
    await page.getByTestId("wb-transforms-create").click();
    await expect(page.getByTestId("wb-transform-modal")).toBeVisible();
    const more = page.getByTestId("wb-transform-more");
    await more.focus();
    await page.keyboard.press("Enter");
    const add = page.getByTestId("wb-transform-replace-add");
    await add.focus();
    await page.keyboard.press("Enter");
    const find = page.getByTestId("wb-transform-replace-find-0");
    await find.focus();
    await page.keyboard.type("a");
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("wb-transform-replace-with-0")).toBeFocused();
    await page.keyboard.type("0");
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("wb-transform-replace-case-0")).toBeFocused();
    await page.keyboard.press("Space");
    await expect(page.getByTestId("wb-transform-replace-case-0")).toBeChecked();
    await page.keyboard.press("Tab");
    await expect(
      page.getByTestId("wb-transform-replace-remove-0"),
    ).toBeFocused();
    await page
      .getByTestId("wb-transform-prompt")
      .fill("Replace all A and E with 0 and 1");
    await page.getByTestId("wb-transform-preview").click();
    await expect(page.getByTestId("wb-transform-preview-exact")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("wb-transform-modal")).toBeHidden();
    fake.entries = fake.entries.filter((item) => item.builtin_key !== null);
  }
});

test("instruction model download: progress, offline retry, keyboard, RTL and axe", async ({
  page,
  ipc,
  hub,
}) => {
  test.setTimeout(180_000);
  const fake = new FakeTransforms();
  const status = (state: string, downloaded = 0): Record<string, unknown> => ({
    state,
    model_name: "Qwen3.5 2B",
    downloaded_bytes: downloaded,
    total_bytes: 1000,
    error: state === "download_failed" ? "offline" : null,
  });
  let retries = 0;
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await openTransforms(page, ipc, hub, fake, {
      handy: { theme, app_language: "ar" },
      beforeNavigate: () => {
        ipc.handle("fork_instruct_status", () => status("not_downloaded"));
        ipc.handle("fork_instruct_retry", () => {
          retries += 1;
          return status("verifying");
        });
      },
    });
    // Nothing shows until Transforms (or Command Mode) wants the model.
    const enabled = page.getByTestId("wb-transforms-enabled");
    if (theme === "light") {
      await expect(page.getByTestId("wb-transforms-model")).toHaveCount(0);
      await enabled.click();
    }
    await expect(enabled).toBeChecked();
    await expect
      .poll(() => ipc.listenerCount("fork-instruct-status"))
      .toBeGreaterThan(0);
    await ipc.emit("fork-instruct-status", status("downloading", 420));
    const line = page.getByTestId("wb-transforms-model");
    await expect(line).toHaveAttribute("data-state", "downloading");
    await expect(line).toContainText("42");
    // The design-system bar, not a native <progress> (owner, rc5).
    const bar = page.getByTestId("wb-transforms-model-progress");
    await expect(bar).toHaveAttribute("role", "progressbar");
    await expect(bar).toHaveAttribute("aria-valuenow", "42");
    await expect(line.locator("progress")).toHaveCount(0);
    await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
    await expectNoSeriousAxeFindings(page);
    // After the last byte the hash runs as "verifying", not 100 %.
    await ipc.emit("fork-instruct-status", status("verifying", 1000));
    await expect(line).toHaveAttribute("data-state", "verifying");
    await expect(bar).toHaveAttribute("aria-valuenow", "100");
    await expectNoSeriousAxeFindings(page);
    await ipc.emit("fork-instruct-status", status("download_failed"));
    await expect(line).toHaveAttribute("data-state", "download_failed");
    await expectNoSeriousAxeFindings(page);
    const retry = page.getByTestId("wb-transforms-model-retry");
    await retry.focus();
    await page.keyboard.press("Enter");
    await expect.poll(() => retries).toBeGreaterThan(theme === "light" ? 0 : 1);
    await expect(line).toHaveAttribute("data-state", "verifying");
    await ipc.emit("fork-instruct-status", status("ready"));
    // A brief "ready", then the row leaves.
    await expect(line).toHaveAttribute("data-state", "ready");
    await expect(bar).toHaveCount(0);
    await expect(line).toHaveCount(0, { timeout: 10_000 });
  }
});
