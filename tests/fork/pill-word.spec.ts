// SPDX-License-Identifier: GPL-3.0-or-later
// S-34: the bar's "New word" note after auto-learn picked up a correction,
// with a mocked backend: the word, Add and ✕ (Not now), the countdown ring
// (paused, Reduce Motion), Add's confirmation, keyboard only, RTL, light and
// dark, and axe. The queue, the 15 s timer, hover pausing and "never during
// a take" are Rust (`fork::pill::model` tests); the real panel is the Mac
// wave (MAC-TEST-SCRIPT S-34).
import { readFileSync } from "node:fs";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import type { ForkPillState, ForkPillWord } from "../../src/fork/bindings";
import { test, expect, type IpcMock } from "./fixtures";
import { expectRtl, RTL_APP_SETTINGS } from "./pill-page";

const PAGE = "/src/fork/pill/index.html";
type Size = { w: number; h: number };
const geometry = JSON.parse(
  readFileSync(
    new URL("../../src/fork/pill/geometry.gen.json", import.meta.url),
    "utf8",
  ),
) as { shapes: Record<ForkPillState["shape"], Size> };

test.use({ ipcSetup: { windowLabel: "fork-pill" } });

function wordState(
  word: Partial<ForkPillWord> = {},
  partial: Partial<ForkPillState> = {},
): ForkPillState {
  return {
    seq: 1,
    phase: "word",
    shape: "word",
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
    word: {
      id: 7,
      word: "Kubernetes",
      remaining_ms: 15_000,
      total_ms: 15_000,
      paused: false,
      result: null,
      ...word,
    },
    ...partial,
  };
}

async function openNote(
  page: Page,
  ipc: IpcMock,
  state: ForkPillState,
  appSettings: unknown = null,
): Promise<void> {
  ipc.respond({ fork_pill_snapshot: state, get_app_settings: appSettings });
  const size = geometry.shapes.word;
  await page.setViewportSize({ width: size.w, height: size.h });
  await page.goto(PAGE);
  await expect(page.getByTestId("wb-pill")).toHaveAttribute(
    "data-phase",
    "word",
  );
}

test("the note names the word and fits its window", async ({ page, ipc }) => {
  test.setTimeout(180_000);
  await openNote(page, ipc, wordState());
  await expect(page.getByTestId("wb-pill-word")).toHaveText(
    "New word Kubernetes",
  );
  const size = geometry.shapes.word;
  const pill = (await page.getByTestId("wb-pill").boundingBox())!;
  expect([pill.width, pill.height]).toEqual([size.w, size.h]);
  for (const id of ["wb-pill-word", "wb-pill-word-add", "wb-pill-dismiss"]) {
    const b = (await page.getByTestId(id).boundingBox())!;
    expect(b.x).toBeGreaterThanOrEqual(0);
    expect(b.x + b.width).toBeLessThanOrEqual(size.w);
    expect(b.y + b.height).toBeLessThanOrEqual(size.h);
  }
  // A long word is cut, never pushes Add or ✕ out.
  await openNote(
    page,
    ipc,
    wordState({ word: "Supercalifragilisticexpialidocious-Kubernetes" }),
  );
  const close = (await page.getByTestId("wb-pill-dismiss").boundingBox())!;
  expect(close.x + close.width).toBeLessThanOrEqual(size.w);
});

test("Add asks the backend; the confirmation follows", async ({
  page,
  ipc,
}) => {
  test.setTimeout(180_000);
  await openNote(page, ipc, wordState());
  await page
    .getByRole("button", { name: "Add Kubernetes to Dictionary" })
    .click();
  await ipc.expectCalled("fork_pill_action", { action: "add_word" });
  await ipc.waitForListener("fork-pill-state");
  await ipc.emit(
    "fork-pill-state",
    wordState(
      { result: "added", remaining_ms: 1_800, total_ms: 1_800 },
      { seq: 2 },
    ),
  );
  await expect(page.getByTestId("wb-pill-word-result")).toHaveText(
    "Added to Dictionary",
  );
  await expect(page.getByTestId("wb-pill-word-add")).toHaveCount(0);
  await ipc.emit("fork-pill-state", wordState({ result: "full" }, { seq: 3 }));
  await expect(page.getByTestId("wb-pill-word-result")).toHaveAttribute(
    "data-result",
    "full",
  );
});

test("✕ is Not now: the word stays waiting", async ({ page, ipc }) => {
  test.setTimeout(180_000);
  await openNote(page, ipc, wordState());
  await page.getByRole("button", { name: "Not now" }).click();
  await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
});

test("the countdown ring empties, stops while paused and under Reduce Motion", async ({
  page,
  ipc,
}) => {
  test.setTimeout(180_000);
  const offset = () =>
    page
      .getByTestId("wb-pill-word-ring")
      .locator("circle")
      .evaluate((c) => parseFloat(getComputedStyle(c).strokeDashoffset));
  await openNote(page, ipc, wordState({ remaining_ms: 3_000 }));
  const first = await offset();
  await expect.poll(offset, { timeout: 10_000 }).toBeGreaterThan(first + 5);
  // Paused (pointer over the note): it stands still at the time left.
  await ipc.waitForListener("fork-pill-state");
  await ipc.emit(
    "fork-pill-state",
    wordState({ remaining_ms: 7_500, paused: true }, { seq: 2 }),
  );
  await expect(page.getByTestId("wb-pill-word-ring")).toHaveAttribute(
    "data-paused",
    "true",
  );
  const paused = await offset();
  // Half the time left: half the ring (r = 11).
  expect(paused).toBeCloseTo(Math.PI * 11, 0);
  await page.waitForTimeout(600);
  expect(await offset()).toBeCloseTo(paused, 3);
  // Reduce Motion: no animation, the ring shows the time left.
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openNote(page, ipc, wordState({ remaining_ms: 15_000 }));
  const still = await offset();
  await page.waitForTimeout(600);
  expect(await offset()).toBeCloseTo(still, 3);
});

// The panel never becomes key (the user's app keeps focus); inside the page
// both controls work from the keyboard when the user focuses them
// (VoiceOver).
test("keyboard only: Add and Not now act on Enter and Space", async ({
  page,
  ipc,
}) => {
  test.setTimeout(180_000);
  await openNote(page, ipc, wordState());
  // Nothing takes focus on its own.
  await expect(page.locator("button:focus")).toHaveCount(0);
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("wb-pill-word-add")).toBeFocused();
  await page.keyboard.press("Enter");
  await ipc.expectCalled("fork_pill_action", { action: "add_word" });
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Not now" })).toBeFocused();
  await page.keyboard.press("Space");
  await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
});

test("RTL mirrors the note", async ({ page, ipc }) => {
  test.setTimeout(180_000);
  await openNote(page, ipc, wordState(), RTL_APP_SETTINGS);
  await expectRtl(page);
  await expect
    .poll(
      async () => {
        const text = await page.getByTestId("wb-pill-word").boundingBox();
        const close = await page.getByTestId("wb-pill-dismiss").boundingBox();
        return !!text && !!close && text.x > close.x;
      },
      { timeout: 60_000 },
    )
    .toBe(true);
});

test("light and dark: no serious or critical axe findings", async ({
  page,
  ipc,
}) => {
  test.setTimeout(180_000);
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    for (const s of [
      wordState(),
      wordState({ paused: true, remaining_ms: 4_000 }),
      wordState({ result: "added" }),
      wordState({ result: "full" }),
    ]) {
      await openNote(page, ipc, s);
      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .disableRules(["region", "landmark-one-main", "page-has-heading-one"])
        .analyze();
      const serious = results.violations.filter(
        (v) => v.impact === "serious" || v.impact === "critical",
      );
      expect(
        serious.map((v) => `${theme} ${s.word?.result ?? "offer"}: ${v.id}`),
      ).toEqual([]);
    }
  }
});
