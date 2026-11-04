// SPDX-License-Identifier: GPL-3.0-or-later
// S-03: the `fork-pill` web view (the active Flow Bar) with a mocked
// backend: every state in light and dark, reduced motion, the ready
// handshake, the buttons, the waveform, RTL and axe. Design W4: the
// capsule and label geometry, the morphs and their ready timing. The resting bar is
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
import { expectRtl, RTL_APP_SETTINGS } from "./pill-page";

const PAGE = "/src/fork/pill/index.html";
type Size = { w: number; h: number };
const geometry = JSON.parse(
  readFileSync(
    new URL("../../src/fork/pill/geometry.gen.json", import.meta.url),
    "utf8",
  ),
) as {
  bars: number;
  shapes: Record<ForkPillState["shape"], Size>;
  capsules: Record<ForkPillState["shape"], Size>;
  label: { h: number; gap: number };
  diff: { chrome: number; line: number; minH: number; maxH: number };
};
/** The window a state asks for: the diff card's height follows its text. */
function windowSize(s: ForkPillState): Size {
  return s.shape === "diff" && s.diff
    ? { w: geometry.shapes.diff.w, h: s.diff.height }
    : geometry.shapes[s.shape];
}
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
    progress: null,
    text: null,
    card_reason: null,
    more: 0,
    copied: false,
    sends_automatically: false,
    context: false,
    diff: null,
    holder: null,
    top: false,
    grown_at_unix_ms: null,
    ...partial,
  };
}

/** One example of every state the backend sends. */
const STATES: Record<string, ForkPillState> = {
  held: state({}),
  heldMic: state({ shape: "held_label", mic: "MacBook Pro Microphone" }),
  heldSends: state({ shape: "held_label", sends_automatically: true }),
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
  // S-07a.
  settingUp: state({ phase: "setting_up", shape: "setup", progress: 42 }),
  settingUpStarting: state({ phase: "setting_up", shape: "setup" }),
  card: state({
    phase: "card",
    shape: "card",
    text: "Can we move the design review to Thursday afternoon? I still need the latest mockups from Anna before we meet.",
  }),
  cardMore: state({
    phase: "card",
    shape: "card",
    text: "Short one.",
    more: 2,
  }),
  cardCopied: state({
    phase: "card",
    shape: "card",
    text: "Short one.",
    copied: true,
  }),
  downloadFailed: state({
    phase: "message",
    shape: "message",
    message: "download_failed",
    tone: "error",
    fix: "retry_download",
  }),
  checkConnection: state({
    phase: "message",
    shape: "message",
    message: "check_connection",
    tone: "error",
    fix: "retry_download",
  }),
  noSpace: state({
    phase: "message",
    shape: "message",
    message: "no_space",
    tone: "error",
    fix: "retry_download",
  }),
  // S-06: "Cancelled · Undo", and the recovery shortcuts' notes.
  cancelled: state({
    phase: "message",
    shape: "message",
    message: "cancelled",
    tone: "info",
    fix: "undo",
  }),
  noTranscript: state({
    phase: "message",
    shape: "message",
    message: "no_transcript",
    tone: "info",
  }),
  copiedLast: state({
    phase: "message",
    shape: "message",
    message: "copied_last",
    tone: "info",
  }),
  // OT2-A: no text field at the release; the text went on the clipboard.
  copiedToClipboard: state({
    phase: "message",
    shape: "message",
    message: "copied_to_clipboard",
    tone: "info",
  }),
  transformDone: state({
    phase: "message",
    shape: "message",
    message: "transform_done",
    tone: "info",
    fix: "view_changes",
  }),
  transformDiff: state({
    phase: "diff",
    shape: "diff",
    diff: {
      run: {
        run_id: 9,
        transform_id: 1,
        app_bundle_id: "com.tinyspeck.slackmacgap",
        source_text: "um we could perhaps meet Tuesday",
        result_text: "We could meet on Tuesday.",
        rating: null,
        created_at_ms: 1,
      },
      diff: {
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
      // One line of text: `geometry::diff_height` gives the smallest card.
      height: 128,
    },
  }),
};

async function openPill(
  page: Page,
  ipc: IpcMock,
  snapshot: ForkPillState | null,
  appSettings: unknown = null,
): Promise<void> {
  ipc.respond({ fork_pill_snapshot: snapshot, get_app_settings: appSettings });
  const size = snapshot ? windowSize(snapshot) : geometry.shapes.held;
  await page.setViewportSize({ width: size.w, height: size.h });
  await page.goto(PAGE);
  // The page sets `dir` from the app language; RTL tests pass an RTL
  // language and wait for it (pill-page.ts).
  if (snapshot) {
    await expect(page.getByTestId("wb-pill")).toHaveAttribute(
      "data-phase",
      snapshot.phase,
    );
  }
}

test.describe("states", () => {
  for (const [name, s] of Object.entries(STATES)) {
    test(`${name} renders at its exact size`, async ({ page, ipc }) => {
      await openPill(page, ipc, s);
      const pill = page.getByTestId("wb-pill");
      await expect(pill).toHaveAttribute("data-phase", s.phase);
      await expect(pill).toHaveAttribute("data-shape", s.shape);
      const box = await pill.boundingBox();
      const size = windowSize(s);
      expect(box?.width).toBe(size.w);
      expect(box?.height).toBe(size.h);
      // The capsule has its own size, on the window's bottom edge.
      const capsule = await page.getByTestId("wb-pill-capsule").boundingBox();
      const cap = s.shape === "diff" ? size : geometry.capsules[s.shape];
      expect(capsule?.width).toBe(cap.w);
      expect(capsule?.height).toBe(cap.h);
      expect(capsule!.y + capsule!.height).toBe(box!.y + box!.height);
      // Nothing overflows the window or the capsule's content.
      const overflow = await pill.evaluate(
        (el) => el.scrollWidth > el.clientWidth + 1,
      );
      expect(overflow).toBe(false);
      const content = await page
        .locator(".wb-pill-content")
        .evaluate((el) => el.scrollWidth > el.clientWidth + 1);
      expect(content).toBe(false);
    });
  }

  test("Design W4: the bar has Wispr Flow's proportions", async () => {
    expect(geometry.shapes.rest).toEqual({ w: 38, h: 6 });
    expect(geometry.shapes.held).toEqual({ w: 72, h: 30 });
    expect(geometry.shapes.locked).toEqual({ w: 100, h: 30 });
    expect(geometry.label.h).toBe(34);
  });

  test("bar text is 15 px on a black capsule", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.noSpeech);
    await expect(page.locator(".wb-pill-message .wb-pill-text")).toHaveCSS(
      "font-size",
      "15px",
    );
    await expect(page.getByTestId("wb-pill-capsule")).toHaveCSS(
      "background-color",
      "rgb(0, 0, 0)",
    );
  });

  test("held shows the waveform and no buttons", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.held);
    await expect(page.getByTestId("wb-pill-wave")).toBeVisible();
    await expect(page.getByTestId("wb-pill-cancel")).toHaveCount(0);
    await expect(page.getByTestId("wb-pill-finish")).toHaveCount(0);
  });

  test("the mic label floats above the capsule; warnings take its place", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, STATES.lockedMic);
    const label = page.getByTestId("wb-pill-mic");
    await expect(label).toHaveText("Mic: MacBook Pro Microphone");
    // The waveform keeps going under the label.
    await expect(page.getByTestId("wb-pill-wave")).toBeVisible();
    await expect(label).toHaveCSS("font-size", "15px");
    await expect
      .poll(async () => (await label.boundingBox())?.height)
      .toBe(geometry.label.h);
    const l = (await label.boundingBox())!;
    const c = (await page.getByTestId("wb-pill-capsule").boundingBox())!;
    expect(Math.round(c.y - (l.y + l.height))).toBe(geometry.label.gap);
    expect(Math.round(l.x + l.width / 2)).toBe(Math.round(c.x + c.width / 2));
    await expect(page.getByRole("status")).toContainText(
      "Mic: MacBook Pro Microphone",
    );
    await ipc.emit("fork-pill-state", { ...STATES.lockedSilence, seq: 2 });
    await expect(page.getByTestId("wb-pill-warning")).toBeVisible();
    await expect(page.getByTestId("wb-pill-mic")).toHaveCount(0);
  });

  test("at the top of the display the label hangs below", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, { ...STATES.heldMic, top: true });
    const label = page.getByTestId("wb-pill-mic");
    await expect
      .poll(async () => (await label.boundingBox())?.height)
      .toBe(geometry.label.h);
    const l = (await label.boundingBox())!;
    const c = (await page.getByTestId("wb-pill-capsule").boundingBox())!;
    expect(c.y).toBe(0);
    expect(Math.round(l.y - (c.y + c.height))).toBe(geometry.label.gap);
  });

  test("S-19a: Sends automatically takes the mic label's place", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, STATES.heldSends);
    await expect(page.getByTestId("wb-pill-sends")).toHaveText(
      "Sends automatically",
    );
    await expect(page.getByTestId("wb-pill-mic")).toHaveCount(0);
    await expect(page.getByRole("status").first()).toContainText(
      "Sends automatically",
    );
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

test.describe("Design W4 morphs", () => {
  // Timing tests: generous waits so a loaded machine (other worktrees
  // building) slows them down instead of failing them.
  const WAIT = 30_000;
  test.beforeEach(() => test.setTimeout(120_000));
  const capsuleWidth = (page: Page) =>
    page
      .getByTestId("wb-pill-capsule")
      .evaluate((el) => el.getBoundingClientRect().width);

  test("a shrink plays before the state is reported painted", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, STATES.heldMic);
    await ipc.expectCalled("fork_pill_ready", { seq: 1 }, WAIT);
    await ipc.waitForListener("fork-pill-state");
    await ipc.clearCalls();
    // The label goes: the window will shrink from 72 to 30 high.
    await ipc.emit("fork-pill-state", { ...STATES.held, seq: 2 });
    await page.waitForTimeout(120);
    expect(
      (await ipc.calls("fork_pill_ready")).filter(
        (c) => (c.args as { seq?: number }).seq === 2,
      ),
    ).toHaveLength(0);
    await ipc.expectCalled("fork_pill_ready", { seq: 2 }, WAIT);
    await expect(page.getByTestId("wb-pill-mic")).toHaveCount(0);
  });

  test("a grow is reported at once and the capsule springs to size", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, STATES.held);
    await ipc.waitForListener("fork-pill-state");
    await ipc.clearCalls();
    await ipc.emit("fork-pill-state", { ...STATES.locked, seq: 2 });
    await ipc.expectCalled("fork_pill_ready", { seq: 2 }, WAIT);
    await expect
      .poll(() => capsuleWidth(page), { timeout: WAIT })
      .toBeCloseTo(geometry.capsules.locked.w, 0);
  });

  test("a page that paints mid-grow continues the bridge's curve", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, null);
    await ipc.waitForListener("fork-pill-state");
    // Record the capsule's width on every frame from now on.
    await page.evaluate(() => {
      const seen: number[] = [];
      (window as unknown as { __widths: number[] }).__widths = seen;
      const tick = () => {
        const el = document.querySelector('[data-testid="wb-pill-capsule"]');
        if (el) seen.push(el.getBoundingClientRect().width);
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
    // A stamp just ahead of the page's clock: the grow starts at the line
    // whatever the machine's load (a busy page may paint late).
    await ipc.emit("fork-pill-state", {
      ...STATES.locked,
      grown_at_unix_ms: (await page.evaluate(() => Date.now())) + 500,
    });
    await expect
      .poll(() => capsuleWidth(page), { timeout: WAIT })
      .toBeCloseTo(geometry.capsules.locked.w, 0);
    const widths = await page.evaluate(
      () => (window as unknown as { __widths: number[] }).__widths,
    );
    // It started at the resting line's width, not at the full size.
    expect(widths[0]).toBeCloseTo(geometry.shapes.rest.w, 0);
    expect(Math.max(...widths)).toBeLessThan(geometry.capsules.locked.w + 2);
  });

  test("Reduce Motion: sizes change at once, no shrink delay", async ({
    page,
    ipc,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openPill(page, ipc, null);
    await ipc.waitForListener("fork-pill-state");
    await ipc.emit("fork-pill-state", {
      ...STATES.locked,
      grown_at_unix_ms: await page.evaluate(() => Date.now()),
    });
    await expect(page.getByTestId("wb-pill-capsule")).toBeVisible();
    expect(await capsuleWidth(page)).toBe(geometry.capsules.locked.w);
    await ipc.emit("fork-pill-state", { ...STATES.lockedMic, seq: 2 });
    await ipc.emit("fork-pill-state", { ...STATES.locked, seq: 3 });
    await ipc.expectCalled("fork_pill_ready", { seq: 3 }, WAIT);
  });
});

test.describe("buttons", () => {
  test("hands-free ✕ cancels and ✓ finishes", async ({ page, ipc }) => {
    // Like "Fix and Dismiss": under the full parallel suite at load ~60 the
    // page itself took past the 30 s default to open (fix/wave1-a run).
    test.setTimeout(180_000);
    await openPill(page, ipc, STATES.locked);
    const cancel = page.getByRole("button", { name: "Cancel dictation" });
    const finish = page.getByRole("button", { name: "Finish dictation" });
    await cancel.click();
    await ipc.expectCalled("fork_pill_action", { action: "cancel" });
    await finish.click();
    await ipc.expectCalled("fork_pill_action", { action: "stop" });
  });

  test("Fix and Dismiss on an error", async ({ page, ipc }) => {
    // Two clicks and two IPC polls; a page.evaluate waited past the 30 s
    // default under the full parallel suite (S-30 wave, load ~100).
    test.setTimeout(120_000);
    await openPill(page, ipc, STATES.micDenied);
    await page.getByRole("button", { name: "Fix" }).click();
    await ipc.expectCalled("fork_pill_action", { action: "fix" });
    await page.getByRole("button", { name: "Dismiss" }).click();
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
  });

  test("a finished transform offers View changes", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.transformDone);
    await expect(page.getByTestId("wb-pill-message")).toHaveText("Done");
    await page.getByRole("button", { name: "View changes" }).click();
    await ipc.expectCalled("fork_pill_action", { action: "fix" });
  });

  test("S-22 diff: words, Configure, Copy, feedback, Retry and Dismiss", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, STATES.transformDiff);
    await expect(page.getByTestId("wb-pill-diff")).toContainText("3 changes");
    await expect(page.locator('[data-diff="delete"]')).toContainText("um we");
    await expect(page.locator('[data-diff="insert"]').first()).toContainText(
      "We could",
    );
    await page.getByTestId("wb-pill-diff-configure").click();
    await ipc.expectCalled("fork_pill_action", { action: "configure_diff" });
    await page.getByTestId("wb-pill-diff-copy").click();
    await ipc.expectCalled("fork_pill_action", { action: "copy_diff" });
    await page.getByTestId("wb-pill-diff-good").click();
    await ipc.expectCalled("fork_pill_action", {
      action: { rate_diff: { rating: 1 } },
    });
    await page.getByTestId("wb-pill-diff-retry").click();
    await ipc.expectCalled("fork_pill_action", { action: "retry_diff" });
    await page.getByTestId("wb-pill-dismiss").click();
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
  });

  test("Polish: the diff card is as tall as its text, chips keep a gap", async ({
    page,
    ipc,
  }) => {
    // Rust sizes the window to chrome + lines × line; the page must lay
    // the card out to the same numbers, so n lines fill the body exactly.
    expect(geometry.diff.minH).toBe(geometry.diff.chrome + geometry.diff.line);
    expect(geometry.shapes.diff.h).toBe(geometry.diff.maxH);
    const base = STATES.transformDiff.diff!;
    for (const lines of [1, 3]) {
      const height = geometry.diff.chrome + lines * geometry.diff.line;
      await openPill(
        page,
        ipc,
        state({ ...STATES.transformDiff, diff: { ...base, height } }),
      );
      const body = page.locator(".wb-pill-diff-body");
      await expect(body).toBeVisible();
      const card = await page.getByTestId("wb-pill-diff").boundingBox();
      expect(card?.height).toBe(height);
      expect(await body.evaluate((el) => el.clientHeight)).toBe(
        lines * geometry.diff.line,
      );
    }
    // The one-line example fits without scrolling or a blank band.
    await openPill(page, ipc, STATES.transformDiff);
    const body = page.locator(".wb-pill-diff-body");
    const fit = await body.evaluate((el) => ({
      scroll: el.scrollHeight,
      client: el.clientHeight,
      text: el.querySelector("p")!.getBoundingClientRect().height,
    }));
    expect(fit.scroll).toBeLessThanOrEqual(fit.client);
    expect(fit.client - fit.text).toBeLessThan(geometry.diff.line);
    // A deleted chip right before an inserted one does not touch it.
    const gap = await page.evaluate(() => {
      const del = document.querySelector('[data-diff="delete"]')!;
      const ins = document.querySelector('[data-diff="insert"]')!;
      const a = del.getClientRects();
      const b = ins.getClientRects()[0];
      return b.left - a[a.length - 1].right;
    });
    expect(gap).toBeGreaterThanOrEqual(2);
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

test("the waveform: a dotted line, then centred bars on the voice", async ({
  page,
  ipc,
}) => {
  // Page load, 20 emits and three polls: room for a busy machine.
  test.setTimeout(120_000);
  await openPill(page, ipc, STATES.held);
  await ipc.waitForListener("fork-pill-levels");
  const wave = page.getByTestId("wb-pill-wave");
  const heights = () =>
    wave
      .locator("i")
      .evaluateAll((bars) =>
        bars.map((b) => (b as HTMLElement).getBoundingClientRect().height),
      );
  const quiet = await heights();
  expect(quiet).toHaveLength(geometry.bars);
  expect(Math.max(...quiet)).toBeLessThanOrEqual(2);
  await expect(wave).toHaveAttribute("data-state", "idle");
  for (let i = 0; i < 8; i++) {
    await ipc.emit("fork-pill-levels", { bars: Array(geometry.bars).fill(1) });
    await page.waitForTimeout(30);
  }
  await expect(wave).toHaveAttribute("data-state", "audio");
  // Centre-weighted: the middle bar stands tallest, the ends lower.
  const mid = Math.floor(geometry.bars / 2);
  await expect
    .poll(async () => {
      const h = await heights();
      return Math.min(...h) > 7 && h[mid] > 12 && h[mid] > h[0];
    })
    .toBe(true);
  // Silence: back down to the dotted line.
  for (let i = 0; i < 12; i++) {
    await ipc.emit("fork-pill-levels", { bars: Array(geometry.bars).fill(0) });
    await page.waitForTimeout(30);
  }
  await expect.poll(async () => Math.max(...(await heights()))).toBeLessThan(3);
});

test.describe("reduced motion", () => {
  test("the processing spinner and bar easing are stills", async ({
    page,
    ipc,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openPill(page, ipc, STATES.transcribing);
    const spinner = page.locator(".wb-pill-spinner");
    await expect(spinner).toHaveCSS("animation-name", "none");
    await ipc.emit("fork-pill-state", { ...STATES.held, seq: 2 });
    const bar = page.getByTestId("wb-pill-wave").locator("i").first();
    // The global Reduce Motion rule (tokens.css) leaves 0.01 ms.
    await expect
      .poll(async () =>
        parseFloat(
          await bar.evaluate((el) => getComputedStyle(el).transitionDuration),
        ),
      )
      .toBeLessThan(0.001);
  });

  // One test per state and theme (S-38): the single loop over all 48 page
  // loads ran past its 120 s budget when the Mac was busy. Separate tests
  // run in parallel and each stays far inside the default timeout.
  for (const theme of ["light", "dark"] as const) {
    for (const [name, s] of Object.entries(STATES)) {
      test(`no serious or critical axe findings: ${name}, ${theme}`, async ({
        page,
        ipc,
      }) => {
        // A load-only timeout: at load ~60 `newPage` alone passed 30 s.
        test.setTimeout(180_000);
        await page.emulateMedia({
          colorScheme: theme,
          reducedMotion: "reduce",
        });
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
        expect(
          serious.map(
            (v) =>
              `${theme} ${name}: ${v.id} ${v.nodes.map((n) => n.target.join(" ") + " " + n.failureSummary).join(" | ")}`,
          ),
        ).toEqual([]);
      });
    }
  }
});

test("an opaque black capsule in both themes; no halo in light", async ({
  page,
  ipc,
}) => {
  const capsule = () => page.getByTestId("wb-pill-capsule");
  const style = () =>
    capsule().evaluate((el) => {
      const s = getComputedStyle(el);
      return { bg: s.backgroundColor, edge: s.boxShadow };
    });
  await page.emulateMedia({ colorScheme: "light" });
  await openPill(page, ipc, STATES.held);
  await expect.poll(style).toEqual({ bg: "rgb(0, 0, 0)", edge: "none" });
  await page.emulateMedia({ colorScheme: "dark" });
  // Dark: the same black, with a faint hairline to find it on black.
  await expect.poll(async () => (await style()).bg).toBe("rgb(0, 0, 0)");
  await expect.poll(async () => (await style()).edge).not.toBe("none");
});

test("hands-free: a dark ✕ disc and a white ✓ disc", async ({ page, ipc }) => {
  await openPill(page, ipc, STATES.locked);
  await expect(page.getByTestId("wb-pill-finish")).toHaveCSS(
    "background-color",
    "rgb(255, 255, 255)",
  );
  await expect(page.getByTestId("wb-pill-cancel")).toHaveCSS(
    "background-color",
    "rgb(48, 48, 47)",
  );
  for (const id of ["wb-pill-cancel", "wb-pill-finish"]) {
    const b = (await page.getByTestId(id).boundingBox())!;
    expect([b.width, b.height]).toEqual([18, 18]);
  }
});

test("RTL locales mirror the hands-free buttons", async ({ page, ipc }) => {
  await openPill(page, ipc, STATES.locked, RTL_APP_SETTINGS);
  // The page sets `dir` from the app language (`getLanguageDirection`).
  await expectRtl(page);
  const cancel = await page.getByTestId("wb-pill-cancel").boundingBox();
  const finish = await page.getByTestId("wb-pill-finish").boundingBox();
  expect(cancel!.x).toBeGreaterThan(finish!.x);
});

test.describe("S-07a: setting up and the card", () => {
  test("setting up shows the progress and no model name", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, STATES.settingUp);
    const setup = page.getByTestId("wb-pill-setup");
    await expect(setup).toContainText("Setting up… 42%");
    await expect(setup).toHaveAttribute("role", "status");
    await expect(setup).not.toContainText(/whisper|parakeet/i);
    await ipc.emit("fork-pill-state", { ...STATES.settingUpStarting, seq: 2 });
    await expect(setup).toContainText("Setting up…");
    await expect(setup).not.toContainText("%");
    await page.getByRole("button", { name: "Dismiss" }).click();
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
  });

  test("the card shows the text, Copy and Dismiss", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.card);
    await expect(page.getByTestId("wb-pill-card-text")).toContainText(
      "design review",
    );
    await expect(page.getByTestId("wb-pill-card")).toContainText(
      "Your dictation",
    );
    // Long text is clamped to two lines inside the card.
    const text = page.getByTestId("wb-pill-card-text");
    const box = await text.boundingBox();
    expect(box!.height).toBeLessThanOrEqual(geometry.shapes.card.h);
    await page.getByRole("button", { name: "Copy" }).click();
    await ipc.expectCalled("fork_pill_action", { action: "copy" });
    await page.getByRole("button", { name: "Dismiss" }).click();
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
  });

  test("more takes in History, and Copied", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.cardMore);
    await expect(page.getByTestId("wb-pill-card")).toContainText(
      "2 more in History",
    );
    await ipc.emit("fork-pill-state", { ...STATES.cardCopied, seq: 2 });
    await expect(page.getByTestId("wb-pill-copy")).toHaveText("Copied");
  });

  test("setup problems offer Retry", async ({ page, ipc }) => {
    for (const [s, text] of [
      [STATES.downloadFailed, "Couldn't download"],
      [STATES.checkConnection, "Check your internet connection"],
      [STATES.noSpace, "Not enough disk space"],
    ] as const) {
      await openPill(page, ipc, s);
      await expect(page.getByTestId("wb-pill-message")).toContainText(text);
      await expect(page.getByTestId("wb-pill-message")).toHaveAttribute(
        "role",
        "alert",
      );
      await page.getByRole("button", { name: "Retry" }).click();
      await ipc.expectCalled("fork_pill_action", { action: "fix" });
    }
  });

  test("keyboard only: Copy and Dismiss act on Enter and Space", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, STATES.card);
    // As for the hands-free buttons: the panel never becomes key, so focus
    // starts on the first button (VoiceOver users reach it the same way).
    const copy = page.getByRole("button", { name: "Copy" });
    await copy.focus();
    await expect(copy).toBeFocused();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_pill_action", { action: "copy" });
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Dismiss" })).toBeFocused();
    await page.keyboard.press("Space");
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
  });

  test("RTL mirrors the card and setting up", async ({ page, ipc }) => {
    await openPill(page, ipc, STATES.card, RTL_APP_SETTINGS);
    await expectRtl(page);
    const text = await page.getByTestId("wb-pill-card").boundingBox();
    const close = await page.getByTestId("wb-pill-dismiss").boundingBox();
    expect(text!.x).toBeGreaterThan(close!.x);
    await openPill(page, ipc, STATES.settingUp, RTL_APP_SETTINGS);
    await expectRtl(page);
    const setup = await page.getByTestId("wb-pill-setup").boundingBox();
    const dismiss = await page.getByTestId("wb-pill-dismiss").boundingBox();
    expect(setup!.x).toBeGreaterThan(dismiss!.x);
  });

  test("the progress ring is still under reduced motion", async ({
    page,
    ipc,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openPill(page, ipc, STATES.settingUpStarting);
    await expect(page.locator(".wb-pill-ring")).toHaveCSS(
      "animation-name",
      "none",
    );
  });
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
            bars: [0.95, 0.8, 0.85, 0.6, 0.7, 0.5, 0.45, 0.35, 0.3, 0.2, 0.15],
          });
          await page.waitForTimeout(300);
        }
        await page.screenshot({
          path: path.join(shotsDir!, `pill-${theme}-${name}.png`),
          omitBackground: false,
        });
      }
    });
  }
});
