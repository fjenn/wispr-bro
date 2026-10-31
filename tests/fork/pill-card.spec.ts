// SPDX-License-Identifier: GPL-3.0-or-later
// S-05: the bar's card for a take that was not pasted (no text field, focus
// moved, a password field, sleep …), with a mocked backend. It is S-07a's
// card (one Card state, one component; `pill.spec.ts` covers the queued
// take) with the reason as its label: every reason, the hidden password
// text, Copy and ✕, keyboard only, RTL, light and dark, and axe. The
// decision behind the card is Rust (`fork::insert::target`, `fork::ax`) and
// the real-Mac run.
//
// Screenshots: WB_SHOTS_DIR=/tmp/card-shots bun run wb:test:ui -- pill-card
import { readFileSync } from "node:fs";
import path from "node:path";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import type { ForkCardReason, ForkPillState } from "../../src/fork/bindings";
import { test, expect, type IpcMock } from "./fixtures";

const PAGE = "/src/fork/pill/index.html";
type Size = { w: number; h: number };
const geometry = JSON.parse(
  readFileSync(
    new URL("../../src/fork/pill/geometry.gen.json", import.meta.url),
    "utf8",
  ),
) as { shapes: Record<ForkPillState["shape"], Size> };
const en = JSON.parse(
  readFileSync(
    new URL("../../src/i18n/locales/en/fork.json", import.meta.url),
    "utf8",
  ),
) as { rust: { pill: { card: Record<ForkCardReason, string> } } };
const shotsDir = process.env.WB_SHOTS_DIR;

test.use({ ipcSetup: { windowLabel: "fork-pill" } });

const REASONS = Object.keys(en.rust.pill.card) as ForkCardReason[];
const TEXT =
  "Thanks for the notes. I will send the draft on Tuesday, and we can go through the open points on Friday morning before the call with the design team.";

function cardState(
  reason: ForkCardReason,
  partial: Partial<ForkPillState> = {},
): ForkPillState {
  return {
    seq: 1,
    phase: "card",
    shape: "card",
    mic: null,
    message: null,
    tone: null,
    fix: null,
    warning: null,
    progress: null,
    text: reason === "secure_field" ? null : TEXT,
    card_reason: reason,
    more: 0,
    copied: false,
    sends_automatically: false,
    context: false,
    ...partial,
  };
}

async function openCard(
  page: Page,
  ipc: IpcMock,
  state: ForkPillState,
): Promise<void> {
  ipc.respond({ fork_pill_snapshot: state, get_app_settings: null });
  const size = geometry.shapes.card;
  await page.setViewportSize({ width: size.w, height: size.h });
  await page.goto(PAGE);
  await expect(page.getByTestId("wb-pill-card")).toBeVisible();
}

test("every reason labels the card, which keeps its exact size", async ({
  page,
  ipc,
}) => {
  for (const r of ["no_text_field", "focus_moved", "secure_field"]) {
    expect(REASONS).toContain(r);
  }
  for (const reason of REASONS) {
    await openCard(page, ipc, cardState(reason));
    const pill = page.getByTestId("wb-pill");
    const box = await pill.boundingBox();
    expect(box?.width).toBe(geometry.shapes.card.w);
    expect(box?.height).toBe(geometry.shapes.card.h);
    await expect(page.getByTestId("wb-pill-card")).toHaveAttribute(
      "data-reason",
      reason,
    );
    await expect(page.getByTestId("wb-pill-card-label")).toHaveText(
      en.rust.pill.card[reason],
    );
    for (const id of ["wb-pill-card", "wb-pill-copy", "wb-pill-dismiss"]) {
      const b = (await page.getByTestId(id).boundingBox())!;
      expect(b.x).toBeGreaterThanOrEqual(0);
      expect(b.x + b.width).toBeLessThanOrEqual(geometry.shapes.card.w);
      expect(b.y + b.height).toBeLessThanOrEqual(geometry.shapes.card.h);
    }
  }
});

test("the no-text-field card shows the transcript", async ({ page, ipc }) => {
  await openCard(page, ipc, cardState("no_text_field"));
  await expect(page.getByTestId("wb-pill-card-label")).toHaveText(
    "Click into a text field first",
  );
  await expect(page.getByTestId("wb-pill-card-text")).toHaveText(TEXT);
});

test("a password field's text is never shown; Copy still works", async ({
  page,
  ipc,
}) => {
  await openCard(page, ipc, cardState("secure_field"));
  const text = page.getByTestId("wb-pill-card-text");
  await expect(text).toHaveAttribute("data-hidden", "true");
  await expect(text).not.toContainText("Thanks");
  await page.getByRole("button", { name: "Copy" }).click();
  await ipc.expectCalled("fork_pill_action", { action: "copy" });
});

test("Copy, then Copied; ✕ dismisses", async ({ page, ipc }) => {
  await openCard(page, ipc, cardState("focus_moved"));
  await page.getByRole("button", { name: "Copy" }).click();
  await ipc.expectCalled("fork_pill_action", { action: "copy" });
  await ipc.waitForListener("fork-pill-state");
  await ipc.emit("fork-pill-state", {
    ...cardState("focus_moved", { copied: true }),
    seq: 2,
  });
  await expect(page.getByTestId("wb-pill-copy")).toContainText("Copied");
  await page.getByRole("button", { name: "Dismiss" }).click();
  await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
});

// The panel never becomes key (it must not take focus from the app the text
// was for): Esc dismisses through the hotkey tap (Rust). Inside the page,
// every control works from the keyboard.
test("keyboard only: Copy and Dismiss act on Enter and Space", async ({
  page,
  ipc,
}) => {
  await openCard(page, ipc, cardState("no_text_field"));
  const copy = page.getByRole("button", { name: "Copy" });
  await copy.focus();
  await page.keyboard.press("Enter");
  await ipc.expectCalled("fork_pill_action", { action: "copy" });
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Dismiss" })).toBeFocused();
  await page.keyboard.press("Space");
  await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
});

test("RTL mirrors the card", async ({ page, ipc }) => {
  await openCard(page, ipc, cardState("no_text_field"));
  await page.evaluate(() => {
    document.documentElement.dir = "rtl";
  });
  await expect
    .poll(async () => {
      const text = (await page.getByTestId("wb-pill-card").boundingBox())!;
      const close = (await page.getByTestId("wb-pill-dismiss").boundingBox())!;
      return text.x > close.x;
    })
    .toBe(true);
});

test("light and dark: no serious or critical axe findings", async ({
  page,
  ipc,
}) => {
  test.setTimeout(120_000);
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    for (const s of [
      cardState("no_text_field"),
      cardState("secure_field"),
      cardState("focus_moved", { copied: true }),
    ]) {
      await openCard(page, ipc, s);
      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .disableRules(["region", "landmark-one-main", "page-has-heading-one"])
        .analyze();
      const serious = results.violations.filter(
        (v) => v.impact === "serious" || v.impact === "critical",
      );
      expect(serious.map((v) => `${theme} ${s.card_reason}: ${v.id}`)).toEqual(
        [],
      );
    }
  }
});

test.describe("screenshots", () => {
  test.skip(!shotsDir, "set WB_SHOTS_DIR to write screenshots");
  for (const theme of ["light", "dark"] as const) {
    test(`cards, ${theme}`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      for (const reason of [
        "no_text_field",
        "focus_moved",
        "secure_field",
      ] as const) {
        await openCard(page, ipc, cardState(reason));
        await page.screenshot({
          path: path.join(shotsDir!, `card-${theme}-${reason}.png`),
        });
      }
    });
  }
});
