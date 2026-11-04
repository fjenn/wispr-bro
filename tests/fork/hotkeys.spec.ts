// SPDX-License-Identifier: GPL-3.0-or-later
import type { Page } from "@playwright/test";
import type { HotkeyBinding, HotkeyStatus } from "../../src/fork/bindings";
import { test, expect, type IpcMock } from "./fixtures";
import { expectNoSeriousAxeFindings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";
import { openSettingsModal } from "./settings-modal-fixture";

const LABELS: Record<string, { text: string; spoken: string }> = {
  fn: { text: "fn", spoken: "Function" },
  "fn+space": { text: "fn Space", spoken: "Function Space" },
  escape: { text: "Esc", spoken: "Escape" },
  "ctrl+command+v": { text: "⌃⌘V", spoken: "Control Command V" },
  "ctrl+command+c": { text: "⌃⌘C", spoken: "Control Command C" },
  "ctrl+command+b": { text: "⌃⌘B", spoken: "Control Command B" },
  mousex1: { text: "Back mouse button", spoken: "Back mouse button" },
};

function statusFor(hotkeys: Record<string, HotkeyBinding[]>): HotkeyStatus {
  const bindings = Object.entries(hotkeys).flatMap(([action, list]) =>
    list.map((binding) => ({
      action,
      chord: binding.chord,
      activation: binding.activation,
      label: LABELS[binding.chord] ?? {
        text: binding.chord,
        spoken: binding.chord,
      },
    })),
  );
  const issues: HotkeyStatus["issues"] = [];
  const seen = new Map<string, string>();
  for (const [action, list] of Object.entries(hotkeys)) {
    for (const binding of list) {
      const other = seen.get(binding.chord);
      if (other && other !== action) {
        issues.push({
          kind: "conflict",
          action,
          other,
          chord: binding.chord,
        });
      } else {
        seen.set(binding.chord, action);
      }
    }
  }
  return {
    running: true,
    error: null,
    trigger: "fn",
    globe: "do_nothing",
    fn_blocked: false,
    issues,
    bindings,
  };
}

function installHotkeys(
  ipc: IpcMock,
  start: Record<string, HotkeyBinding[]> = {},
) {
  let current = {
    ...structuredClone(forkSettingsDefaults),
    hotkeys: {
      ...structuredClone(forkSettingsDefaults.hotkeys),
      ...start,
    },
  };
  ipc.handle("fork_get_settings", () => current);
  ipc.handle("fork_hotkey_status", () => statusFor(current.hotkeys));
  ipc.handle("fork_set_setting", (args) => {
    const update = args.update as {
      key: string;
      value: { action: string; bindings: HotkeyBinding[] };
    };
    current = {
      ...current,
      hotkeys: {
        ...current.hotkeys,
        [update.value.action]: update.value.bindings,
      },
    };
    return current;
  });
}

async function openHotkeys(
  page: Page,
  ipc: IpcMock,
  start: Record<string, HotkeyBinding[]> = {},
) {
  const initial = {
    ...structuredClone(forkSettingsDefaults),
    hotkeys: {
      ...structuredClone(forkSettingsDefaults.hotkeys),
      ...start,
    },
  };
  await openSettingsModal(page, ipc, "hotkeys", {
    fork_get_settings: initial,
    fork_hotkey_status: statusFor(initial.hotkeys),
  });
  installHotkeys(ipc, start);
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
}

async function press(
  ipc: IpcMock,
  modifiers: string[],
  key: string | null,
  hotkey: string,
) {
  await ipc.waitForListener("handy-keys-event");
  await ipc.emit("handy-keys-event", {
    modifiers,
    key,
    is_key_down: true,
    hotkey_string: hotkey,
  });
  await ipc.emit("handy-keys-event", {
    modifiers: [],
    key,
    is_key_down: false,
    hotkey_string: hotkey,
  });
}

test("lists every action with its defaults and unset states", async ({
  page,
  ipc,
}) => {
  await openHotkeys(page, ipc);
  await expect(page.getByTestId("wb-dictation-key-current")).toContainText(
    "Shortcut: fn",
  );
  // The dictation key is one row; the rest sits under "More shortcuts".
  await expect(page.getByTestId("wb-hotkey-action-dictate")).toHaveCount(0);
  for (const id of [
    "hands_free",
    "cancel",
    "paste_last",
    "copy_last",
    "press_enter",
    "undo_cancel",
  ]) {
    await expect(page.getByTestId(`wb-hotkey-action-${id}`)).toBeVisible();
  }
  await expect(page.getByTestId("wb-hotkey-hands_free-1-chord")).toHaveText(
    "fn Space",
  );
  await expect(page.getByTestId("wb-hotkey-action-press_enter")).toContainText(
    "Not set",
  );
});

test("add captures a mouse button and persists it", async ({ page, ipc }) => {
  await openHotkeys(page, ipc);
  await page.getByTestId("wb-hotkey-press_enter-add").click();
  await press(ipc, [], "mousex1", "mousex1");
  await expect(page.getByTestId("wb-hotkey-press_enter-0-chord")).toHaveText(
    "Back mouse button",
  );
  expect((await ipc.calls("fork_set_setting")).at(-1)?.args).toEqual({
    update: {
      key: "hotkey",
      value: {
        action: "press_enter",
        bindings: [{ chord: "mousex1", activation: "press" }],
      },
    },
  });
});

test("edit and delete save the changed binding list", async ({ page, ipc }) => {
  await openHotkeys(page, ipc);
  await page.getByTestId("wb-hotkey-paste_last-0-edit").click();
  await press(ipc, ["ctrl", "command"], "b", "ctrl+command+b");
  await expect(page.getByTestId("wb-hotkey-paste_last-0-chord")).toHaveText(
    "⌃⌘B",
  );
  await page.getByTestId("wb-hotkey-paste_last-0-delete").click();
  await expect(page.getByTestId("wb-hotkey-action-paste_last")).toContainText(
    "Not set",
  );
  expect((await ipc.calls("fork_set_setting")).at(-1)?.args).toEqual({
    update: {
      key: "hotkey",
      value: { action: "paste_last", bindings: [] },
    },
  });
});

test("a fourth binding is allowed and Add then stops", async ({
  page,
  ipc,
}) => {
  await openHotkeys(page, ipc, {
    press_enter: [
      { chord: "mousex1", activation: "press" },
      { chord: "ctrl+command+b", activation: "press" },
      { chord: "ctrl+command+c", activation: "press" },
    ],
  });
  await page.getByTestId("wb-hotkey-press_enter-add").click();
  await press(ipc, ["ctrl", "command"], "v", "ctrl+command+v");
  await expect(page.getByTestId("wb-hotkey-press_enter-3-chord")).toHaveText(
    "⌃⌘V",
  );
  await expect(page.getByTestId("wb-hotkey-press_enter-add")).toBeDisabled();
  await expect(page.getByTestId("wb-hotkey-action-press_enter")).toContainText(
    "4 of 4 set",
  );
});

test("Secure Input refuses capture with an explanation and no recorder", async ({
  page,
  ipc,
}) => {
  await openHotkeys(page, ipc);
  ipc.respond("get_secure_input_status", {
    enabled: true,
    sustained: true,
    culprit_pid: null,
    culprit_name: null,
    fallback_active: false,
    covered_bindings: [],
    degraded_bindings: [],
    uncovered_bindings: [],
    recorder_blocked: true,
  });
  await ipc.clearCalls();
  await page.getByTestId("wb-hotkey-press_enter-add").click();
  await expect(page.getByTestId("wb-hotkey-capture-error")).toContainText(
    "Secure Input",
  );
  await expect(page.getByTestId("wb-hotkey-capture-error")).toContainText(
    "Leave the password field",
  );
  expect(await ipc.calls("start_handy_keys_recording")).toHaveLength(0);
  await expectNoSeriousAxeFindings(page);
  await page.getByTestId("wb-hotkey-capture-cancel").click();
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
});

test("conflicts are named on the affected row", async ({ page, ipc }) => {
  await openHotkeys(page, ipc, {
    copy_last: [{ chord: "ctrl+command+v", activation: "press" }],
  });
  await expect(page.getByTestId("wb-hotkey-copy_last-0-issue")).toContainText(
    "Also used by Paste last dictation",
  );
  await expect(page.getByTestId("wb-hotkey-copy_last-0-issue")).toHaveAttribute(
    "role",
    "alert",
  );
});

test("Reset restores every default, including unset recovery actions", async ({
  page,
  ipc,
}) => {
  await openHotkeys(page, ipc, {
    dictate: [{ chord: "ctrl+option", activation: "hold" }],
    press_enter: [{ chord: "mousex1", activation: "press" }],
  });
  await expect(page.getByTestId("wb-dictation-key-current")).toContainText(
    "Shortcut: ⌃ ⌥",
  );
  await page.getByTestId("wb-hotkeys-reset").click();
  await expect(page.getByTestId("wb-dictation-key-current")).toContainText(
    "Shortcut: fn",
  );
  await expect(page.getByTestId("wb-hotkey-action-press_enter")).toContainText(
    "Not set",
  );
  await expect(page.getByTestId("wb-hotkeys-reset")).toBeEnabled();
});

test("Escape cancels capture without closing the panel", async ({
  page,
  ipc,
}) => {
  await openHotkeys(page, ipc);
  await page.getByTestId("wb-hotkey-undo_cancel-add").focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("wb-hotkey-capture-cancel")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("wb-hotkey-capture-cancel")).toHaveCount(0);
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
  expect(await ipc.calls("fork_set_setting")).toHaveLength(0);
});

test("RTL and reduced motion keep the hotkeys panel accessible", async ({
  page,
  ipc,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openHotkeys(page, ipc);
  await page.getByTestId("wb-settings-modal").evaluate((dialog) => {
    dialog.setAttribute("dir", "rtl");
  });
  await expect(page.getByTestId("wb-settings-modal")).toHaveCSS(
    "direction",
    "rtl",
  );
  const overflow = await page
    .getByTestId("wb-settings-content")
    .evaluate((content) => content.scrollWidth > content.clientWidth + 1);
  expect(overflow).toBe(false);
  await expectNoSeriousAxeFindings(page);
});

// Audit OT2: the panel says what each action really takes; dictation and
// cancel run on the keys the engine uses, so their "+" stops at that.
test("each action offers only as many shortcuts as it takes", async ({
  page,
  ipc,
}) => {
  await openHotkeys(page, ipc);
  await expect(page.getByTestId("wb-hotkeys-panel")).toContainText(
    "Most actions take up to 4 shortcuts. Hold to talk, cancel and Command Mode take one each, hands-free two.",
  );
  for (const id of ["hands_free", "cancel"]) {
    await expect(page.getByTestId(`wb-hotkey-${id}-add`)).toBeDisabled();
  }
  await expect(page.getByTestId("wb-hotkey-cancel-add")).toHaveAttribute(
    "title",
    "This action takes 1 shortcut.",
  );
  await expect(page.getByTestId("wb-hotkey-hands_free-add")).toHaveAttribute(
    "title",
    "This action takes up to 2 shortcuts.",
  );
  await expect(page.getByTestId("wb-hotkey-action-cancel")).toContainText(
    "1 of 1 set",
  );
  await expect(page.getByTestId("wb-hotkey-paste_last-add")).toBeEnabled();
  // Removing the hands-free Space binding frees its second slot.
  await page.getByTestId("wb-hotkey-hands_free-1-delete").click();
  await expect(page.getByTestId("wb-hotkey-hands_free-add")).toBeEnabled();
  await expectNoSeriousAxeFindings(page);
  // Keyboard: the freed "+" starts a capture from Enter.
  await page.getByTestId("wb-hotkey-hands_free-add").focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("wb-hotkey-hands_free-1-chord")).toHaveText(
    "Press a shortcut…",
  );
});

test("a conflict with a custom transform names the transform", async ({
  page,
  ipc,
}) => {
  ipc.respond("fork_transforms_list", [
    {
      id: 3,
      key: "custom-3",
      action_id: "transform.custom-3",
      builtin_key: null,
      name: "Make it friendly",
      description: "",
      rules: {
        shorter: false,
        clearer: false,
        better_order: false,
        headings_lists: false,
        keep_tone: true,
      },
      prompt: "",
      examples: [],
      updated_at: 1,
    },
  ]);
  await openHotkeys(page, ipc, {
    "transform.custom-3": [{ chord: "ctrl+command+b", activation: "press" }],
    press_enter: [{ chord: "ctrl+command+b", activation: "press" }],
  });
  await expect(page.getByTestId("wb-hotkey-press_enter-0-issue")).toContainText(
    "Also used by Make it friendly",
  );
  await expect(page.getByTestId("wb-hotkeys-panel")).not.toContainText(
    "transform.custom-3",
  );
  await expect(
    page.getByRole("heading", { name: "Make it friendly" }),
  ).toBeVisible();
});

for (const theme of ["light", "dark"] as const) {
  test(`the limited actions pass axe in ${theme} and RTL`, async ({
    page,
    ipc,
  }) => {
    await page.emulateMedia({ colorScheme: theme });
    await openHotkeys(page, ipc);
    await page.evaluate(() => {
      document.documentElement.dir = "rtl";
    });
    await expect(page.getByTestId("wb-hotkey-cancel-add")).toBeDisabled();
    await expectNoSeriousAxeFindings(page);
  });
}

// Owner test: changing the dictation key is one step, not the whole list.
test("the dictation key changes in one step and moves hands-free with it", async ({
  page,
  ipc,
}) => {
  await openHotkeys(page, ipc);
  await page.getByTestId("wb-dictation-key-change").click();
  const capture = page.getByTestId("wb-dictation-key-capture");
  await expect(capture).toContainText("Press the key or keys you want");
  await expect(page.getByTestId("wb-dictation-key")).toContainText(
    "Hold to talk. Double-tap for hands-free.",
  );
  await expect(page.getByTestId("wb-dictation-key-cancel")).toBeFocused();
  await expect(page.getByTestId("wb-dictation-key-save")).toBeDisabled();
  // Live caps while the keys are down.
  await ipc.waitForListener("handy-keys-event");
  await ipc.emit("handy-keys-event", {
    modifiers: ["ctrl", "option"],
    key: null,
    is_key_down: true,
    hotkey_string: "ctrl+option",
  });
  await expect(page.getByTestId("wb-dictation-key-field")).toHaveAttribute(
    "data-live",
    "ctrl+option",
  );
  await expect(page.getByTestId("wb-dictation-key-field")).toHaveText("⌃ ⌥");
  await ipc.emit("handy-keys-event", {
    modifiers: [],
    key: null,
    is_key_down: false,
    hotkey_string: "",
  });
  await expect(page.getByTestId("wb-dictation-key-save")).toBeEnabled();
  await page.getByTestId("wb-dictation-key-save").click();
  await expect(capture).toHaveCount(0);
  const saved = (await ipc.calls("fork_set_setting")).map(
    (call) => (call.args as { update: { value: unknown } }).update.value,
  );
  expect(saved).toEqual([
    {
      action: "dictate",
      bindings: [{ chord: "ctrl+option", activation: "hold" }],
    },
    {
      action: "hands_free",
      bindings: [
        { chord: "ctrl+option", activation: "double_tap" },
        { chord: "ctrl+option+space", activation: "press" },
      ],
    },
  ]);
  await expect(page.getByTestId("wb-dictation-key-current")).toContainText(
    "Shortcut: ⌃ ⌥",
  );
});

test("a key that can't be the dictation key says why, inline", async ({
  page,
  ipc,
}) => {
  await openHotkeys(page, ipc, {
    paste_last: [{ chord: "ctrl+option", activation: "press" }],
  });
  await page.getByTestId("wb-dictation-key-change").click();
  await press(ipc, ["option"], null, "option");
  await expect(page.getByTestId("wb-dictation-key-problem")).toHaveText(
    "Use fn, or two modifier keys together, such as ⌃ ⌥.",
  );
  await expect(page.getByTestId("wb-dictation-key-save")).toBeDisabled();
  await press(ipc, ["ctrl", "option"], null, "ctrl+option");
  await expect(page.getByTestId("wb-dictation-key-problem")).toContainText(
    "Also used by Paste last dictation",
  );
  await expect(page.getByTestId("wb-dictation-key-save")).toBeDisabled();
  await press(ipc, ["fn"], null, "fn");
  await expect(page.getByTestId("wb-dictation-key-problem")).toHaveCount(0);
  await expect(page.getByTestId("wb-dictation-key-save")).toBeEnabled();
});

test("Escape cancels the dictation key capture and saves nothing", async ({
  page,
  ipc,
}) => {
  await openHotkeys(page, ipc);
  // Keyboard only: Tab to Change, Enter starts the capture.
  await page.getByTestId("wb-dictation-key-change").focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("wb-dictation-key-cancel")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("wb-dictation-key-capture")).toHaveCount(0);
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
  expect(await ipc.calls("fork_set_setting")).toHaveLength(0);
  expect(await ipc.calls("stop_handy_keys_recording")).not.toHaveLength(0);
});

for (const theme of ["light", "dark"] as const) {
  test(`the dictation key capture passes axe in ${theme} and RTL`, async ({
    page,
    ipc,
  }) => {
    await page.emulateMedia({ colorScheme: theme });
    await openHotkeys(page, ipc);
    await page.getByTestId("wb-dictation-key-change").click();
    await press(ipc, ["option"], null, "option");
    await expect(page.getByTestId("wb-dictation-key-problem")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await page.evaluate(() => {
      document.documentElement.dir = "rtl";
    });
    await expectNoSeriousAxeFindings(page);
  });
}
