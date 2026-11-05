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
        board(page, theme).evaluate(
          (el) => getComputedStyle(el).backgroundColor,
        );
      expect(await bg("light")).not.toEqual(await bg("dark"));
      await axe(page);

      const dir = process.env.WB_GALLERY_SHOTS_DIR;
      if (dir) {
        const height = await gallery(page).evaluate((el) => el.scrollHeight);
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
    await expect(
      light.getByRole("radio", { name: "High" }).first(),
    ).toBeFocused();
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
    await expect(
      page.getByRole("option", { name: /AirPods Pro/ }),
    ).toBeFocused();
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

// Design Phase 0b: chips, radios, option cards, setting rows, toasts, the
// wordmark, and popups that keep a pinned board's theme.
test.describe("primitives gallery: Phase 0b pieces", () => {
  test.setTimeout(180_000);

  test("keyboard: chips, radios, option cards, setting row", async ({
    page,
    ipc,
    hub,
  }) => {
    await openGallery(page, ipc, hub);
    const light = board(page, "light");

    // Toggle chips: Space/Enter flip aria-pressed.
    const chips = light.getByTestId("light-chips");
    await expect(chips).toHaveAttribute("role", "group");
    const nl = chips.getByRole("button", { name: "Nederlands" });
    await nl.focus();
    await expect(nl).toHaveAttribute("aria-pressed", "false");
    await page.keyboard.press("Space");
    await expect(nl).toHaveAttribute("aria-pressed", "true");
    await page.keyboard.press("Enter");
    await expect(nl).toHaveAttribute("aria-pressed", "false");

    // Removable chip: the ✕ is named; Delete removes, Enter removes.
    const removeSafari = light.getByRole("button", { name: "Remove Safari" });
    await removeSafari.focus();
    await page.keyboard.press("Delete");
    await expect(removeSafari).toHaveCount(0);
    const removeNotes = light.getByRole("button", { name: "Remove Notes" });
    await removeNotes.focus();
    await page.keyboard.press("Enter");
    await expect(removeNotes).toHaveCount(0);

    // Radio group: Tab enters on the chosen item, arrows move and select,
    // disabled items are skipped.
    const radios = light.getByTestId("light-radios");
    await expect(radios).toHaveAttribute("role", "radiogroup");
    const builtIn = radios.getByRole("radio", { name: "Built-in microphone" });
    await expect(builtIn).toBeChecked();
    await builtIn.focus();
    await page.keyboard.press("ArrowDown");
    const headset = radios.getByRole("radio", { name: "Headset" });
    await expect(headset).toBeFocused();
    await expect(headset).toBeChecked();
    await page.keyboard.press("ArrowDown");
    await expect(builtIn).toBeFocused();
    await expect(
      radios.getByRole("radio", { name: "Desk microphone" }),
    ).toBeDisabled();

    // Option cards: same model; the card shows the selected state.
    const options = light.getByTestId("light-options");
    const chat = options.getByRole("radio", { name: "Messages and chat" });
    await chat.focus();
    await page.keyboard.press("ArrowDown");
    const docs = options.getByRole("radio", { name: "Email and documents" });
    await expect(docs).toBeChecked();
    await expect(docs).toHaveAccessibleDescription("Longer, careful text.");
    const card = (name: string) =>
      options
        .locator("label")
        .filter({ has: page.getByRole("radio", { name }) });
    await expect(card("Email and documents")).toHaveAttribute(
      "data-selected",
      "true",
    );
    await expect(card("Messages and chat")).not.toHaveAttribute(
      "data-selected",
      "true",
    );

    // Setting row: the control is named and described by the row.
    const sw = light.getByTestId("light-setting-sounds-switch");
    await expect(sw).toHaveAccessibleName("Dictation sounds");
    await expect(sw).toHaveAccessibleDescription(
      "A soft tick when the bar starts and stops.",
    );
    await sw.focus();
    await page.keyboard.press("Space");
    await expect(sw).not.toBeChecked();
  });

  test("popups keep the theme of the board that opened them", async ({
    page,
    ipc,
    hub,
  }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await openGallery(page, ipc, hub, { handy: { theme: "light" } });
    const dark = board(page, "dark");
    await dark.getByRole("button", { name: "Open popover" }).click();
    const popover = page.getByRole("dialog", { name: "Open popover" });
    await expect(popover).toBeVisible();
    await expect(popover).toHaveAttribute("data-wb-theme", "dark");
    const bg = await popover.evaluate(
      (el) => getComputedStyle(el).backgroundColor,
    );
    // --wb-dark-surface #222220, not the light #ffffff.
    expect(bg).toBe("rgb(34, 34, 32)");
    // Its text follows the pinned theme too (--wb-dark-ink #f3f2ee).
    expect(
      await popover.locator("p").evaluate((el) => getComputedStyle(el).color),
    ).toBe("rgb(243, 242, 238)");
    await page.keyboard.press("Escape");

    await dark.getByRole("button", { name: "Open dialog" }).click();
    const dialog = page.getByRole("dialog", { name: "Add word" });
    await expect(dialog).toBeVisible();
    expect(
      await dialog.evaluate((el) => getComputedStyle(el).backgroundColor),
    ).toBe("rgb(34, 34, 32)");
    await page.keyboard.press("Escape");
  });

  test("toasts: sonner toasts wear the HUD card; the wordmark", async ({
    page,
    ipc,
    hub,
  }) => {
    await openGallery(page, ipc, hub);
    const light = board(page, "light");
    const view = light.getByTestId("light-toast");
    const hud = "rgb(27, 27, 27)"; // --wb-hud-2
    await expect(view).toHaveCSS("background-color", hud);
    await expect(board(page, "dark").getByTestId("dark-toast")).toHaveCSS(
      "background-color",
      hud,
    );

    await light.getByRole("button", { name: "Show a toast" }).click();
    const live = page.locator("[data-sonner-toast]");
    await expect(live).toBeVisible();
    await expect(live).toContainText("It will be spelled your way");
    await expect(live).toHaveCSS("background-color", hud);
    await expect(live).toHaveCSS("border-radius", "12px");
    await expect(live.getByRole("button", { name: "Undo" })).toHaveCSS(
      "background-color",
      "rgb(240, 237, 230)", // --wb-cream
    );

    await expect(light.getByTestId("light-wordmark")).toHaveText("Wispr Bro");
    await expect(
      light.getByTestId("light-wordmark-compact"),
    ).toHaveAccessibleName("Wispr Bro");
  });

  test("RTL and Reduce Motion: radios follow reading order, chips work", async ({
    page,
    ipc,
    hub,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openGallery(page, ipc, hub, { handy: { app_language: "ar" } });
    await expect(gallery(page)).toHaveAttribute("dir", "rtl");
    const light = board(page, "light");
    const tiles = light.getByRole("radiogroup", { name: "Pick your mic" });
    const usb = tiles.getByRole("radio", { name: "USB mic" });
    await usb.focus();
    // In RTL the grid runs right to left: ArrowLeft moves to the next tile.
    await page.keyboard.press("ArrowLeft");
    await expect(tiles.getByRole("radio", { name: "Headset" })).toBeChecked();
    const nl = light.getByRole("button", { name: "Nederlands" });
    await nl.click();
    await expect(nl).toHaveAttribute("aria-pressed", "true");
    await axe(page);
  });
});
