// SPDX-License-Identifier: GPL-3.0-or-later
// Design Phase 0: the primitives gallery (`#/fork/gallery`, debug builds
// only). Screenshots it for the area agents and the owner (set
// WB_GALLERY_SHOTS_DIR), checks that release builds never show it, and runs
// axe, keyboard and RTL checks on the primitives in both themes.
import path from "node:path";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { test, expect, openHub, type HubState } from "./hub-fixture";
import type { IpcMock } from "./fixtures";

const pingReply = (debug: boolean) => ({
  message: "pong",
  seq: 0,
  ready: true,
  debug_build: debug,
  cli_debug: false,
});

async function openGallery(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  options: { debug?: boolean; handy?: Record<string, unknown> } = {},
) {
  await openHub(page, ipc, hub, {
    handy: options.handy,
    size: { width: 1400, height: 900 },
  });
  // After openHub, which answers fork_ping as a release build.
  ipc.respond("fork_ping", pingReply(options.debug ?? true));
  await page.evaluate(() => {
    window.location.hash = "#/fork/gallery";
  });
}

const gallery = (page: Page) => page.getByTestId("wb-gallery");
const board = (page: Page, theme: "light" | "dark") =>
  page.getByTestId(`wb-gallery-${theme}`);

async function settle(page: Page) {
  await page.waitForFunction(() =>
    document.getAnimations().every((a) => a.playState !== "running"),
  );
}

async function axe(page: Page) {
  await settle(page);
  const results = await new AxeBuilder({ page })
    .include('[data-testid="wb-gallery"]')
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  const serious = results.violations.filter(
    (v) => v.impact === "serious" || v.impact === "critical",
  );
  expect(
    serious.map((v) => ({
      id: v.id,
      nodes: v.nodes.map((n) => n.target.join(" ")).slice(0, 5),
    })),
  ).toEqual([]);
}

test.describe("primitives gallery", () => {
  test.setTimeout(180_000);

  test("is not available in a release build", async ({ page, ipc, hub }) => {
    await openGallery(page, ipc, hub, { debug: false });
    await ipc.expectCalled("fork_ping");
    await expect(gallery(page)).toHaveCount(0);
  });

  for (const scheme of ["light", "dark"] as const) {
    test(`shows every primitive in both themes, axe clean (${scheme} OS)`, async ({
      page,
      ipc,
      hub,
    }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await openGallery(page, ipc, hub, { handy: { theme: scheme } });
      await expect(board(page, "light")).toBeVisible();
      await expect(board(page, "dark")).toBeVisible();
      await expect(board(page, "dark")).toHaveAttribute(
        "data-wb-theme",
        "dark",
      );
      // The pinned boards really differ.
      const bg = (theme: "light" | "dark") =>
        board(page, theme).evaluate((el) => getComputedStyle(el).backgroundColor);
      expect(await bg("light")).not.toEqual(await bg("dark"));
      await axe(page);

      const dir = process.env.WB_GALLERY_SHOTS_DIR;
      if (dir) {
        const height = await gallery(page).evaluate(
          (el) => el.scrollHeight,
        );
        await page.setViewportSize({ width: 1400, height });
        await gallery(page).evaluate((el) => el.scrollTo({ top: 0 }));
        await settle(page);
        await page.screenshot({
          path: path.join(dir, `gallery-${scheme}-os.png`),
        });
      }
    });
  }

  test("keyboard: segmented control, switch, select, dialog", async ({
    page,
    ipc,
    hub,
  }) => {
    await openGallery(page, ipc, hub);
    const light = board(page, "light");

    // Segmented control: arrows move and select; Tab enters on the choice.
    const radio = light.getByRole("radio", { name: "Medium" }).first();
    await radio.focus();
    await page.keyboard.press("ArrowRight");
    await expect(light.getByRole("radio", { name: "High" }).first()).toBeFocused();
    await expect(
      light.getByRole("radio", { name: "High" }).first(),
    ).toHaveAttribute("aria-checked", "true");

    // Switch: Space toggles, role switch.
    const sw = light.getByTestId("light-switch-on");
    await sw.focus();
    await expect(sw).toBeChecked();
    await page.keyboard.press("Space");
    await expect(sw).not.toBeChecked();

    // Select: opens with Enter, arrows, Enter picks, focus returns.
    const select = light.getByTestId("light-select");
    await select.focus();
    await page.keyboard.press("Enter");
    const listbox = page.getByRole("listbox");
    await expect(listbox).toBeVisible();
    await expect(
      page.getByRole("option", { name: "MacBook Pro Microphone" }),
    ).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option", { name: /AirPods Pro/ })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(listbox).toHaveCount(0);
    await expect(select).toBeFocused();
    await expect(select).toContainText("AirPods Pro");

    // Dialog: focus moves in, Tab is trapped, Esc closes, focus returns.
    const open = light.getByRole("button", { name: "Open dialog" });
    await open.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Add word" });
    await expect(dialog).toBeVisible();
    await settle(page);
    for (let i = 0; i < 6; i++) {
      await page.keyboard.press("Tab");
      expect(
        await dialog.evaluate((el) => el.contains(document.activeElement)),
      ).toBe(true);
    }
    await axe(page);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(open).toBeFocused();

    // Sheet: back and close buttons, Esc closes.
    await light.getByRole("button", { name: "Open sheet" }).click();
    const sheet = page.getByRole("dialog", { name: "Lesson" });
    await expect(sheet).toBeVisible();
    await expect(sheet.getByRole("img", { name: "Step 1 of 3" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(sheet).toHaveCount(0);
  });

  test("RTL: the gallery mirrors and arrows follow reading order", async ({
    page,
    ipc,
    hub,
  }) => {
    await openGallery(page, ipc, hub, { handy: { app_language: "ar" } });
    await expect(gallery(page)).toHaveAttribute("dir", "rtl");
    const light = board(page, "light");
    const medium = light.getByRole("radio").nth(1);
    await medium.focus();
    // In RTL, ArrowLeft moves forward (to the third option).
    await page.keyboard.press("ArrowLeft");
    await expect(light.getByRole("radio").nth(2)).toBeFocused();
    await axe(page);
  });

  test("reduced motion: dialogs still open and close", async ({
    page,
    ipc,
    hub,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openGallery(page, ipc, hub);
    await board(page, "light")
      .getByRole("button", { name: "Open dialog" })
      .click();
    const dialog = page.getByRole("dialog", { name: "Add word" });
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveCSS("opacity", "1");
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
  });
});
