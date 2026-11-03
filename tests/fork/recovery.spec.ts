// SPDX-License-Identifier: GPL-3.0-or-later
// S-06: recovery with a mocked backend. The bar's "Cancelled · Undo" (Undo
// and ✕ wiring, keyboard, RTL, axe in light and dark). Shortcut editing moved
// to S-14's unified panel and is covered in hotkeys.spec.ts. The undo window,
// the last-transcript query and the delivery are Rust tests and the real-Mac
// run (docs/fork/test-runs/1.0.0-alpha.1-M-26-S-06.md).
import { readFileSync } from "node:fs";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import type { ForkPillState } from "../../src/fork/bindings";
import { test, expect, type IpcMock } from "./fixtures";
import { expectRtl, RTL_APP_SETTINGS } from "./pill-page";

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

  async function openBar(
    page: Page,
    ipc: IpcMock,
    appSettings: unknown = null,
  ) {
    ipc.respond({
      fork_pill_snapshot: cancelled,
      get_app_settings: appSettings,
    });
    const size = geometry.shapes.message;
    await page.setViewportSize({ width: size.w, height: size.h });
    await page.goto("/src/fork/pill/index.html");
    await expect(page.getByTestId("wb-pill-message")).toBeVisible();
    if (!appSettings) {
      await expect(page.getByTestId("wb-pill-message")).toHaveText("Cancelled");
    }
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
    await openBar(page, ipc, RTL_APP_SETTINGS);
    await expectRtl(page);
    const pill = await page.getByTestId("wb-pill").boundingBox();
    const undo = await page.getByTestId("wb-pill-fix").boundingBox();
    const text = await page.getByTestId("wb-pill-message").boundingBox();
    expect(pill && undo && text).toBeTruthy();
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
