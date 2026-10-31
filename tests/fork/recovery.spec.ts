// SPDX-License-Identifier: GPL-3.0-or-later
// S-06: recovery with a mocked backend. The bar's "Cancelled · Undo" (Undo
// and ✕ wiring, keyboard, RTL, axe in light and dark) and the Recovery
// shortcuts section on the debug panel (labels, Not set, the mouse-button
// note, recording through Handy's recorder events, Escape, Clear, Default,
// an unusable binding, keyboard only, RTL, axe in light and dark). The undo
// window, the last-transcript query and the delivery are Rust tests and the
// real-Mac run (docs/fork/test-runs/1.0.0-alpha.1-M-26-S-06.md).
import { readFileSync } from "node:fs";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import type {
  ForkPillState,
  HotkeyBinding,
  HotkeyStatus,
} from "../../src/fork/bindings";
import { test, expect, type IpcMock } from "./fixtures";
import { expectNoSeriousAxeFindings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";

const geometry = JSON.parse(
  readFileSync(
    new URL("../../src/fork/pill/geometry.gen.json", import.meta.url),
    "utf8",
  ),
) as { shapes: Record<ForkPillState["shape"], { w: number; h: number }> };

const cancelled: ForkPillState = {
  seq: 1,
  phase: "message",
  shape: "message",
  mic: null,
  message: "cancelled",
  tone: "info",
  fix: "undo",
  warning: null,
  progress: null,
  text: null,
  card_reason: null,
  more: 0,
  copied: false,
  context: false,
};

test.describe("Cancelled · Undo on the bar", () => {
  test.use({ ipcSetup: { windowLabel: "fork-pill" } });

  async function openBar(page: Page, ipc: IpcMock) {
    ipc.respond({ fork_pill_snapshot: cancelled, get_app_settings: null });
    const size = geometry.shapes.message;
    await page.setViewportSize({ width: size.w, height: size.h });
    await page.goto("/src/fork/pill/index.html");
    await expect(page.getByTestId("wb-pill-message")).toHaveText("Cancelled");
  }

  test("Undo and ✕ act, and the message is a polite status", async ({
    page,
    ipc,
  }) => {
    await openBar(page, ipc);
    await expect(page.getByTestId("wb-pill-message")).toHaveAttribute(
      "role",
      "status",
    );
    const undo = page.getByRole("button", { name: "Undo" });
    await expect(undo).toBeVisible();
    await undo.click();
    await ipc.expectCalled("fork_pill_action", { action: "fix" });
    await page.getByRole("button", { name: "Dismiss" }).click();
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
    // Nothing overflows the message capsule.
    const overflow = await page
      .getByTestId("wb-pill")
      .evaluate((el) => el.scrollWidth > el.clientWidth + 1);
    expect(overflow).toBe(false);
  });

  test("keyboard only: Undo acts on Enter, Dismiss on Space", async ({
    page,
    ipc,
  }) => {
    await openBar(page, ipc);
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Undo" })).toBeFocused();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_pill_action", { action: "fix" });
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Dismiss" })).toBeFocused();
    await page.keyboard.press("Space");
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
  });

  test("RTL keeps Undo next to the text, inside the capsule", async ({
    page,
    ipc,
  }) => {
    await openBar(page, ipc);
    await page.evaluate(() => {
      document.documentElement.dir = "rtl";
    });
    const pill = await page.getByTestId("wb-pill").boundingBox();
    const undo = await page.getByRole("button", { name: "Undo" }).boundingBox();
    const text = await page.getByTestId("wb-pill-message").boundingBox();
    expect(pill && undo && text).toBeTruthy();
    // Mirrored: the text is on the right, Undo to its left.
    expect(undo!.x + undo!.width).toBeLessThanOrEqual(text!.x + 1);
    expect(undo!.x).toBeGreaterThanOrEqual(pill!.x);
  });

  for (const theme of ["light", "dark"] as const) {
    test(`no serious axe findings, ${theme}`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openBar(page, ipc);
      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .disableRules(["region", "landmark-one-main", "page-has-heading-one"])
        .analyze();
      const serious = results.violations.filter(
        (v) => v.impact === "serious" || v.impact === "critical",
      );
      expect(serious.map((v) => v.id)).toEqual([]);
    });
  }
});

// ---------------------------------------------------------------------------
// The Recovery shortcuts section

const pingReply = {
  message: "pong",
  seq: 0,
  ready: true,
  debug_build: true,
  cli_debug: false,
};

type Hotkeys = Record<string, HotkeyBinding[]>;

const LABELS: Record<string, { text: string; spoken: string }> = {
  "ctrl+command+v": { text: "⌃⌘V", spoken: "Control Command V" },
  "ctrl+command+c": { text: "⌃⌘C", spoken: "Control Command C" },
  mousex1: { text: "Back mouse button", spoken: "Back mouse button" },
  return: { text: "↩", spoken: "Return" },
};

function statusFor(hotkeys: Hotkeys): HotkeyStatus {
  const bindings = Object.entries(hotkeys).flatMap(([action, list]) =>
    list
      .filter((b) => LABELS[b.chord])
      .map((b) => ({
        action,
        chord: b.chord,
        activation: b.activation,
        label: LABELS[b.chord],
      })),
  );
  const issues = (hotkeys.press_enter ?? [])
    .filter((b) => b.chord === "return")
    .map((b) => ({
      kind: "unsupported" as const,
      action: "press_enter",
      chord: b.chord,
      reason: "Press Enter cannot be bound to Return itself",
    }));
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

/** A fake backend: settings and hotkey status that follow `fork_set_setting`. */
async function openSection(page: Page, ipc: IpcMock, start: Hotkeys = {}) {
  let hotkeys: Hotkeys = { ...forkSettingsDefaults.hotkeys, ...start };
  ipc.respond({
    fork_ping: pingReply,
    fork_llm_last_cleanup: null,
    start_handy_keys_recording: null,
    stop_handy_keys_recording: null,
  });
  ipc.handle("fork_get_settings", () => ({
    ...forkSettingsDefaults,
    hotkeys,
  }));
  ipc.handle("fork_hotkey_status", () => statusFor(hotkeys));
  ipc.handle("fork_set_setting", (args) => {
    const update = args.update as {
      key: string;
      value: { action: string; bindings: HotkeyBinding[] };
    };
    hotkeys = { ...hotkeys, [update.value.action]: update.value.bindings };
    return { ...forkSettingsDefaults, hotkeys };
  });
  await page.goto("/#/fork/debug");
  const section = page.getByTestId("fork-debug-section-recovery");
  await expect(section).toBeVisible({ timeout: 15_000 });
  await expect(section.getByTestId("wb-recovery")).toBeVisible();
  return section;
}

/** Handy's recorder, as `handy-keys-event`s for one chord. */
async function press(
  ipc: IpcMock,
  modifiers: string[],
  key: string,
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
    modifiers,
    key,
    is_key_down: false,
    hotkey_string: hotkey,
  });
}

test.describe("Recovery shortcuts", () => {
  test("shows the defaults, Not set, and the mouse-button note", async ({
    page,
    ipc,
  }) => {
    const section = await openSection(page, ipc);
    await expect(
      section.getByTestId("wb-recovery-paste_last-chord"),
    ).toHaveText("⌃⌘V");
    await expect(
      section.getByTestId("wb-recovery-paste_last-chord"),
    ).toHaveAttribute("aria-label", "Shortcut: Control Command V");
    await expect(section.getByTestId("wb-recovery-copy_last-chord")).toHaveText(
      "⌃⌘C",
    );
    for (const id of ["press_enter", "undo_cancel"]) {
      await expect(section.getByTestId(`wb-recovery-${id}-chord`)).toHaveText(
        "Not set",
      );
      await expect(section.getByTestId(`wb-recovery-${id}-clear`)).toHaveCount(
        0,
      );
    }
    // Defaults show no "Use default"; unset rows without a default neither.
    await expect(
      section.getByRole("button", { name: "Use default" }),
    ).toHaveCount(0);
    await expect(section.getByTestId("wb-recovery-mouse-note")).toContainText(
      "side button",
    );
    // Each row is a named region described by its help text.
    await expect(
      section.getByRole("region", { name: "Press Enter" }),
    ).toHaveAccessibleDescription(/Presses Return for you/);
  });

  test("records a mouse button for Press Enter through Handy's recorder", async ({
    page,
    ipc,
  }) => {
    const section = await openSection(page, ipc);
    await section.getByTestId("wb-recovery-press_enter-record").click();
    await ipc.expectCalled("start_handy_keys_recording", {
      bindingId: "fork:press_enter",
    });
    await expect(
      section.getByTestId("wb-recovery-press_enter-chord"),
    ).toHaveText("Press a shortcut…");
    // The other rows wait.
    await expect(
      section.getByTestId("wb-recovery-paste_last-record"),
    ).toBeDisabled();
    await press(ipc, [], "mousex1", "mousex1");
    await expect(
      section.getByTestId("wb-recovery-press_enter-chord"),
    ).toHaveText("Back mouse button");
    expect((await ipc.calls("fork_set_setting")).at(-1)?.args).toEqual({
      update: {
        key: "hotkey",
        value: {
          action: "press_enter",
          bindings: [{ chord: "mousex1", activation: "press" }],
        },
      },
    });
    await ipc.expectCalled("stop_handy_keys_recording");
    // Clear unsets it.
    await section.getByTestId("wb-recovery-press_enter-clear").click();
    await expect(
      section.getByTestId("wb-recovery-press_enter-chord"),
    ).toHaveText("Not set");
    expect((await ipc.calls("fork_set_setting")).at(-1)?.args).toEqual({
      update: {
        key: "hotkey",
        value: { action: "press_enter", bindings: [] },
      },
    });
  });

  test("Escape cancels a recording and leaves the panel open", async ({
    page,
    ipc,
  }) => {
    const section = await openSection(page, ipc);
    await section.getByTestId("wb-recovery-undo_cancel-record").click();
    await expect(
      section.getByTestId("wb-recovery-undo_cancel-cancel"),
    ).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(
      section.getByTestId("wb-recovery-undo_cancel-chord"),
    ).toHaveText("Not set");
    await expect(page.getByTestId("fork-debug-panel")).toBeVisible();
    // Escape from Handy's recorder cancels too; nothing was saved.
    await section.getByTestId("wb-recovery-undo_cancel-record").click();
    await press(ipc, [], "escape", "escape");
    await expect(
      section.getByTestId("wb-recovery-undo_cancel-record"),
    ).toBeVisible();
    expect(await ipc.calls("fork_set_setting")).toHaveLength(0);
  });

  test("a changed default offers Use default; Return is reported", async ({
    page,
    ipc,
  }) => {
    const section = await openSection(page, ipc, {
      copy_last: [],
      press_enter: [{ chord: "return", activation: "press" }],
    });
    await expect(section.getByTestId("wb-recovery-copy_last-chord")).toHaveText(
      "Not set",
    );
    await expect(
      section.getByTestId("wb-recovery-press_enter-issue"),
    ).toHaveAttribute("role", "alert");
    await section.getByTestId("wb-recovery-copy_last-default").click();
    await expect(section.getByTestId("wb-recovery-copy_last-chord")).toHaveText(
      "⌃⌘C",
    );
    await expect(
      section.getByTestId("wb-recovery-copy_last-default"),
    ).toHaveCount(0);
  });

  test("keyboard only: record a chord with Tab and Enter", async ({
    page,
    ipc,
  }) => {
    const section = await openSection(page, ipc, { paste_last: [] });
    const record = section.getByTestId("wb-recovery-paste_last-record");
    await record.focus();
    await expect(record).toBeFocused();
    await page.keyboard.press("Enter");
    await press(ipc, ["ctrl", "command"], "v", "ctrl+command+v");
    await expect(
      section.getByTestId("wb-recovery-paste_last-chord"),
    ).toHaveText("⌃⌘V");
  });

  test("RTL lays the rows out mirrored", async ({ page, ipc }) => {
    const section = await openSection(page, ipc);
    await page.evaluate(() => {
      document.documentElement.dir = "rtl";
    });
    const row = section.getByTestId("wb-recovery-paste_last");
    const title = await row.getByRole("heading").boundingBox();
    const chord = await row
      .getByTestId("wb-recovery-paste_last-chord")
      .boundingBox();
    expect(title && chord).toBeTruthy();
    // The title starts at the right edge; the shortcut sits to its left.
    expect(chord!.x + chord!.width).toBeLessThanOrEqual(title!.x + 1);
  });

  for (const theme of ["light", "dark"] as const) {
    test(`no serious axe findings, ${theme}`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      const section = await openSection(page, ipc, {
        press_enter: [{ chord: "return", activation: "press" }],
      });
      await expect(
        section.getByTestId("wb-recovery-press_enter-issue"),
      ).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }
});
