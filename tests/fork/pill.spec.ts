// SPDX-License-Identifier: GPL-3.0-or-later
// S-03: the `fork-pill` web view (the active Flow Bar) with a mocked
// backend: every state in light and dark, reduced motion, the ready
// handshake, the buttons, the waveform, RTL and axe. The resting bar is
// native (src-tauri/src/fork/pill/native.rs) and is checked on the real Mac
// (docs/fork/MAC-TEST-SCRIPT.md, S-03).
//
// Screenshots of every state: WB_SHOTS_DIR=/tmp/pill-shots bun run wb:test:ui -- pill
import { readFileSync } from "node:fs";
import path from "node:path";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import type { ForkPillState } from "../../src/fork/bindings";
import { test, expect, type IpcMock } from "./fixtures";

const PAGE = "/src/fork/pill/index.html";
type Size = { w: number; h: number };
const geometry = JSON.parse(
  readFileSync(
    new URL("../../src/fork/pill/geometry.gen.json", import.meta.url),
    "utf8",
  ),
) as { bars: number; shapes: Record<ForkPillState["shape"], Size> };
const shotsDir = process.env.WB_SHOTS_DIR;

test.use({ ipcSetup: { windowLabel: "fork-pill" } });

function state(partial: Partial<ForkPillState>): ForkPillState {
  return {
    seq: 1,
    phase: "held",
    shape: "held",
    mic: null,
    message: null,
    tone: null,
    fix: null,
    warning: null,
    ...partial,
  };
}

/** One example of every state the backend sends. */
const STATES: Record<string, ForkPillState> = {
  held: state({}),
  heldMic: state({ shape: "held_label", mic: "MacBook Pro Microphone" }),
  locked: state({ phase: "locked", shape: "locked" }),
  lockedMic: state({
    phase: "locked",
    shape: "locked_label",
    mic: "MacBook Pro Microphone",
  }),
  lockedSilence: state({
    phase: "locked",
    shape: "locked_label",
    warning: "silence",
  }),
  transcribing: state({ phase: "transcribing", shape: "status" }),
  cleaningUp: state({ phase: "cleaning_up", shape: "status" }),
  noSpeech: state({
    phase: "message",
    shape: "message",
    message: "no_speech",
    tone: "info",
  }),
  notPasted: state({
    phase: "message",
    shape: "message",
    message: "not_pasted",
    tone: "info",
  }),
  micDenied: state({
    phase: "message",
    shape: "message",
    message: "mic_denied",
    tone: "error",
    fix: "microphone_privacy",
  }),
  micMuted: state({
    phase: "message",
    shape: "message",
    message: "mic_muted",
    tone: "error",
    fix: "sound_input",
  }),
};

async function openPill(
  page: Page,
  ipc: IpcMock,
  snapshot: ForkPillState | null,
): Promise<void> {
  ipc.respond({ fork_pill_snapshot: snapshot, get_app_settings: null });
  const size = snapshot
    ? geometry.shapes[snapshot.shape]
    : geometry.shapes.held;
  await page.setViewportSize({ width: size.w, height: size.h });
  await page.goto(PAGE);
}

test.describe("states", () => {
  for (const [name, s] of Object.entries(STATES)) {
    test(`${name} renders at its exact size`, async ({ page, ipc }) => {
      await openPill(page, ipc, s);
      const pill = page.getByTestId("wb-pill");
      await expect(pill).toHaveAttribute("data-phase", s.phase);
      await expect(pill).toHaveAttribute("data-shape", s.shape);
      const box = await pill.boundingBox();
      expect(box?.width).toBe(geometry.shapes[s.shape].w);
      expect(box?.height).toBe(geometry.shapes[s.shape].h);
      // Nothing overflows the capsule.
      const overflow = await pill.evaluate(
        (el) => el.scrollWidth > el.clientWidth + 1,
      );
      expect(overflow).toBe(false);
    });
  }

  test("held shows the waveform and no buttons", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.held);
    await expect(page.getByTestId("wb-pill-wave")).toBeVisible();
    await expect(page.getByTestId("wb-pill-cancel")).toHaveCount(0);
    await expect(page.getByTestId("wb-pill-finish")).toHaveCount(0);
  });

  test("the mic label and warnings take the waveform's place", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, STATES.lockedMic);
    await expect(page.getByTestId("wb-pill-mic")).toHaveText(
      "Mic: MacBook Pro Microphone",
    );
    await expect(page.getByTestId("wb-pill-wave")).toHaveCount(0);
    await ipc.emit("fork-pill-state", { ...STATES.lockedSilence, seq: 2 });
    await expect(page.getByTestId("wb-pill-warning")).toBeVisible();
  });

  test("messages use the shared Rust strings", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.noSpeech);
    await expect(page.getByTestId("wb-pill-message")).toHaveText(
      "No speech heard",
    );
    await expect(page.getByTestId("wb-pill-message")).toHaveAttribute(
      "role",
      "status",
    );
    await expect(page.getByTestId("wb-pill-fix")).toHaveCount(0);
  });

  test("errors are alerts with Fix", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.micDenied);
    await expect(page.getByTestId("wb-pill-message")).toHaveAttribute(
      "role",
      "alert",
    );
    await expect(page.getByTestId("wb-pill-fix")).toBeVisible();
  });
});

test.describe("backend handshake", () => {
  test("reports each painted state, never an older one over a newer", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, STATES.held);
    await ipc.expectCalled("fork_pill_ready", { seq: 1 });
    await ipc.waitForListener("fork-pill-state");
    await ipc.emit("fork-pill-state", { ...STATES.transcribing, seq: 5 });
    await expect(page.getByTestId("wb-pill")).toHaveAttribute(
      "data-phase",
      "transcribing",
    );
    await ipc.expectCalled("fork_pill_ready", { seq: 5 });
    // A late event with a lower seq is ignored.
    await ipc.emit("fork-pill-state", { ...STATES.held, seq: 3 });
    await page.waitForTimeout(100);
    await expect(page.getByTestId("wb-pill")).toHaveAttribute(
      "data-phase",
      "transcribing",
    );
  });

  test("a page loaded at rest waits for the first state", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, null);
    await expect(page.getByTestId("wb-pill-idle")).toBeAttached();
    await ipc.waitForListener("fork-pill-state");
    await ipc.emit("fork-pill-state", { ...STATES.locked, seq: 1 });
    await expect(page.getByTestId("wb-pill")).toHaveAttribute(
      "data-phase",
      "locked",
    );
    await ipc.expectCalled("fork_pill_ready", { seq: 1 });
  });
});

test.describe("buttons", () => {
  test("hands-free ✕ cancels and ✓ finishes", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.locked);
    const cancel = page.getByRole("button", { name: "Cancel dictation" });
    const finish = page.getByRole("button", { name: "Finish dictation" });
    await cancel.click();
    await ipc.expectCalled("fork_pill_action", { action: "cancel" });
    await finish.click();
    await ipc.expectCalled("fork_pill_action", { action: "stop" });
  });

  test("Fix and Dismiss on an error", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.micDenied);
    await page.getByRole("button", { name: "Fix" }).click();
    await ipc.expectCalled("fork_pill_action", { action: "fix" });
    await page.getByRole("button", { name: "Dismiss" }).click();
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
  });

  // The bar's panel never becomes key (it must not take focus from the
  // frontmost app), so keyboard users cancel with Esc and finish with fn;
  // the buttons still are real, named buttons for VoiceOver.
  test("buttons are named buttons that act on Enter", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.locked);
    const finish = page.getByRole("button", { name: "Finish dictation" });
    await finish.focus();
    await expect(finish).toBeFocused();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_pill_action", { action: "stop" });
  });
});

test("the waveform follows the mic levels", async ({ page, ipc }) => {
  await openPill(page, ipc, STATES.held);
  await ipc.waitForListener("fork-pill-levels");
  const heights = () =>
    page
      .getByTestId("wb-pill-wave")
      .locator("i")
      .evaluateAll((bars) => bars.map((b) => (b as HTMLElement).offsetHeight));
  const quiet = await heights();
  expect(quiet).toHaveLength(geometry.bars);
  expect(Math.max(...quiet)).toBeLessThanOrEqual(3);
  for (let i = 0; i < 8; i++) {
    await ipc.emit("fork-pill-levels", { bars: Array(geometry.bars).fill(1) });
    await page.waitForTimeout(30);
  }
  await expect
    .poll(async () => Math.min(...(await heights())))
    .toBeGreaterThan(12);
});

test.describe("reduced motion", () => {
  test("processing dots and bar easing are stills", async ({ page, ipc }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openPill(page, ipc, STATES.transcribing);
    const dot = page.locator(".wb-pill-dots i").first();
    await expect(dot).toHaveCSS("animation-name", "none");
    await ipc.emit("fork-pill-state", { ...STATES.held, seq: 2 });
    const bar = page.getByTestId("wb-pill-wave").locator("i").first();
    await expect(bar).toHaveCSS("transition-duration", "0s");
  });

  test("no serious or critical axe findings in any state", async ({
    page,
    ipc,
  }) => {
    for (const theme of ["light", "dark"] as const) {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      for (const s of Object.values(STATES)) {
        await openPill(page, ipc, s);
        await expect(page.getByTestId("wb-pill")).toBeVisible();
        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          // A capsule window has no page landmarks or title to speak of.
          .disableRules(["region", "landmark-one-main", "page-has-heading-one"])
          .analyze();
        const serious = results.violations.filter(
          (v) => v.impact === "serious" || v.impact === "critical",
        );
        expect(serious.map((v) => `${theme} ${s.phase}: ${v.id}`)).toEqual([]);
      }
    }
  });
});

test("dark mode lightens the capsule", async ({ page, ipc }) => {
  const bg = async () =>
    page
      .getByTestId("wb-pill")
      .evaluate((el) => getComputedStyle(el).backgroundColor);
  await page.emulateMedia({ colorScheme: "light" });
  await openPill(page, ipc, STATES.held);
  const light = await bg();
  await page.emulateMedia({ colorScheme: "dark" });
  await expect.poll(bg).not.toBe(light);
});

test("RTL locales mirror the hands-free buttons", async ({ page, ipc }) => {
  await openPill(page, ipc, STATES.locked);
  // The page sets `dir` from the app language (`getLanguageDirection`).
  await page.evaluate(() => {
    document.documentElement.dir = "rtl";
  });
  const cancel = await page.getByTestId("wb-pill-cancel").boundingBox();
  const finish = await page.getByTestId("wb-pill-finish").boundingBox();
  expect(cancel!.x).toBeGreaterThan(finish!.x);
});

test.describe("screenshots", () => {
  test.skip(!shotsDir, "set WB_SHOTS_DIR to write screenshots");

  for (const theme of ["light", "dark"] as const) {
    test(`every state, ${theme}`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      for (const [name, s] of Object.entries(STATES)) {
        await openPill(page, ipc, s);
        if (s.phase === "held" || s.phase === "locked") {
          await ipc.waitForListener("fork-pill-levels");
          await ipc.emit("fork-pill-levels", {
            bars: [0.2, 0.5, 0.9, 0.6, 0.8, 0.4, 0.7, 0.3, 0.15],
          });
          await page.waitForTimeout(120);
        }
        await page.screenshot({
          path: path.join(shotsDir!, `pill-${theme}-${name}.png`),
          omitBackground: false,
        });
      }
    });
  }
});
