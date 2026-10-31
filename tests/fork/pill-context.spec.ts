// SPDX-License-Identifier: GPL-3.0-or-later
// S-20: the context glyph on the bar while a take is processed (DEC-24:
// a visible indicator that nearby text helped), with a mocked backend:
// shown only when the state says so, inside the exact capsule size, named
// for VoiceOver, axe in light and dark, RTL, and nothing to focus (it is
// not a control, so keyboard users never land on it).
import { readFileSync } from "node:fs";
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
) as { shapes: Record<ForkPillState["shape"], Size> };

test.use({ ipcSetup: { windowLabel: "fork-pill" } });

function state(partial: Partial<ForkPillState>): ForkPillState {
  return {
    seq: 1,
    phase: "transcribing",
    shape: "status",
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
    context: false,
    ...partial,
  };
}

const WITH = {
  transcribing: state({ context: true }),
  cleaningUp: state({ phase: "cleaning_up", context: true }),
};

async function openPill(page: Page, ipc: IpcMock, s: ForkPillState) {
  ipc.respond({ fork_pill_snapshot: s, get_app_settings: null });
  const size = geometry.shapes[s.shape];
  await page.setViewportSize({ width: size.w, height: size.h });
  await page.goto(PAGE);
  await expect(page.getByTestId("wb-pill")).toHaveAttribute(
    "data-phase",
    s.phase,
  );
}

test("the glyph shows only when nearby text was used", async ({
  page,
  ipc,
}) => {
  await openPill(page, ipc, state({}));
  await expect(page.getByTestId("wb-pill-working")).toBeVisible();
  await expect(page.getByTestId("wb-pill-context")).toHaveCount(0);
  await ipc.waitForListener("fork-pill-state");
  await ipc.emit("fork-pill-state", { ...WITH.cleaningUp, seq: 2 });
  const glyph = page.getByTestId("wb-pill-context");
  await expect(glyph).toBeVisible();
  await expect(page.getByTestId("wb-pill-working")).toContainText(
    "Cleaning up…",
  );
  // Named for VoiceOver; an image, not a control.
  await expect(
    page.getByRole("img", { name: "Nearby text helped with spelling" }),
  ).toBeVisible();
  // A message after the take carries no glyph.
  await ipc.emit("fork-pill-state", {
    ...state({
      phase: "message",
      shape: "message",
      message: "no_speech",
      tone: "info",
    }),
    seq: 3,
  });
  await expect(page.getByTestId("wb-pill-context")).toHaveCount(0);
});

test("the glyph fits the capsule in both processing states", async ({
  page,
  ipc,
}) => {
  for (const s of Object.values(WITH)) {
    await openPill(page, ipc, s);
    const pill = page.getByTestId("wb-pill");
    const box = await pill.boundingBox();
    expect(box?.width).toBe(geometry.shapes.status.w);
    expect(box?.height).toBe(geometry.shapes.status.h);
    const overflow = await pill.evaluate(
      (el) => el.scrollWidth > el.clientWidth + 1,
    );
    expect(overflow).toBe(false);
    const g = await page.getByTestId("wb-pill-context").boundingBox();
    expect(g!.x + g!.width).toBeLessThanOrEqual(box!.x + box!.width);
  }
});

test("keyboard only: nothing on the processing bar takes focus", async ({
  page,
  ipc,
}) => {
  await openPill(page, ipc, WITH.transcribing);
  await page.keyboard.press("Tab");
  const focused = await page.evaluate(
    () => document.activeElement?.getAttribute("data-testid") ?? "body",
  );
  expect(focused).not.toBe("wb-pill-context");
  await expect(page.locator("[tabindex]")).toHaveCount(0);
});

test("RTL puts the glyph at the reading end", async ({ page, ipc }) => {
  await openPill(page, ipc, WITH.cleaningUp);
  const ltrText = await page.locator(".wb-pill-text").boundingBox();
  const ltrGlyph = await page.getByTestId("wb-pill-context").boundingBox();
  expect(ltrGlyph!.x).toBeGreaterThan(ltrText!.x);
  await page.evaluate(() => {
    document.documentElement.dir = "rtl";
  });
  await expect
    .poll(async () => {
      const text = await page.locator(".wb-pill-text").boundingBox();
      const glyph = await page.getByTestId("wb-pill-context").boundingBox();
      return glyph!.x < text!.x;
    })
    .toBe(true);
});

test("no serious or critical axe findings, light and dark", async ({
  page,
  ipc,
}) => {
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    for (const s of Object.values(WITH)) {
      await openPill(page, ipc, s);
      await expect(page.getByTestId("wb-pill-context")).toBeVisible();
      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
        .disableRules(["region", "landmark-one-main", "page-has-heading-one"])
        .analyze();
      const serious = results.violations.filter(
        (v) => v.impact === "serious" || v.impact === "critical",
      );
      expect(serious.map((v) => `${theme} ${s.phase}: ${v.id}`)).toEqual([]);
    }
  }
});
