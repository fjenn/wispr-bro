// SPDX-License-Identifier: GPL-3.0-or-later
// S-13 Hub shell: navigation, keyboard, banners, tags, theme, deep links,
// file-drop routing, collapse, RTL, reduced motion and accessibility
// (R5 S-13 tests, §5.5, §11.2). Mocked IPC; see hub-fixture.ts.
import type { Page } from "@playwright/test";
import { HUB_PAGE_IDS } from "../../src/fork/hub/nav";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
} from "./hub-fixture";

const PAGE_TITLES: Record<(typeof HUB_PAGE_IDS)[number], string> = {
  dictation: "Dictation",
  insights: "Insights",
  dictionary: "Dictionary",
  snippets: "Snippets",
  style: "Style",
  transforms: "Transforms",
  scratchpad: "Scratchpad",
  share: "Share with a friend",
  settings: "Settings",
};

const main = (page: Page) => page.getByTestId("wb-main");

async function currentPage(page: Page) {
  return main(page).getAttribute("data-page");
}

/** Center of an element, in CSS px (the mock reports scale factor 1). */
async function centerOf(page: Page, testId: string) {
  const box = await page.getByTestId(testId).boundingBox();
  if (!box) throw new Error(`${testId} has no box`);
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

test.describe("hub shell", () => {
  test("every nav item opens its page and is marked current", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    expect(await currentPage(page)).toBe("dictation");

    for (const id of HUB_PAGE_IDS) {
      await page.getByTestId(`wb-nav-${id}`).click();
      await expect(main(page)).toHaveAttribute("data-page", id);
      await expect(main(page).getByRole("heading", { level: 1 })).toHaveText(
        PAGE_TITLES[id],
      );
      const current = page.locator('[aria-current="page"]');
      await expect(current).toHaveCount(1);
      await expect(current).toHaveAttribute("data-testid", `wb-nav-${id}`);
      await expect(page).toHaveURL(new RegExp(`#/hub/${id}$`));
    }
  });

  test("sidebar items are buttons with names in a labelled nav", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    const nav = page.getByRole("navigation", { name: "Main" });
    await expect(nav).toBeVisible();
    for (const id of HUB_PAGE_IDS) {
      await expect(
        nav.getByRole("button", { name: PAGE_TITLES[id], exact: false }),
      ).toBeVisible();
    }
    await expect(nav.getByRole("button", { name: "Help" })).toBeVisible();
  });

  test("keyboard only: tab through the sidebar, open pages and the menus", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await page.locator("body").focus();

    // First stop is the collapse button, then the pages in sidebar order.
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("wb-sidebar-toggle")).toBeFocused();
    const order = [
      "dictation",
      "insights",
      "dictionary",
      "snippets",
      "style",
      "transforms",
      "scratchpad",
      "share",
      "settings",
      "help",
    ];
    for (const id of order) {
      await page.keyboard.press("Tab");
      await expect(page.getByTestId(`wb-nav-${id}`)).toBeFocused();
    }
    // Then the header controls.
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("wb-theme-toggle")).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("wb-notifications-bell")).toBeFocused();

    // Every page opens from the keyboard.
    for (const id of HUB_PAGE_IDS) {
      await page.getByTestId(`wb-nav-${id}`).focus();
      await page.keyboard.press("Enter");
      await expect(main(page)).toHaveAttribute("data-page", id);
    }
    await page.getByTestId("wb-nav-scratchpad").focus();
    await page.keyboard.press("Space");
    await expect(main(page)).toHaveAttribute("data-page", "scratchpad");

    // Help is a menu: Enter opens it, arrows move, Enter picks, focus returns.
    await page.getByTestId("wb-nav-help").focus();
    await page.keyboard.press("Enter");
    const menu = page.getByRole("menu", { name: "Help" });
    await expect(menu).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByTestId("wb-menu-item-models")).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(menu).toBeHidden();
    await expect(main(page)).toHaveAttribute("data-page", "settings");
    await expect(page.getByRole("tab", { name: "Models" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(page.getByTestId("wb-nav-help")).toBeFocused();

    // The notifications panel opens with Enter and closes with Esc.
    await page.getByTestId("wb-notifications-bell").focus();
    await page.keyboard.press("Enter");
    const panel = page.getByRole("dialog", { name: "Notifications" });
    await expect(panel).toBeVisible();
    await expect(page.getByTestId("wb-notifications-empty")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(page.getByTestId("wb-notifications-bell")).toBeFocused();
  });

  test("settings tabs move with the arrow keys", async ({ page, ipc, hub }) => {
    await openHub(page, ipc, hub, { hash: "#/hub/settings" });
    const general = page.getByRole("tab", { name: "General" });
    await general.focus();
    await page.keyboard.press("ArrowRight");
    const history = page.getByRole("tab", { name: "History" });
    await expect(history).toBeFocused();
    await expect(history).toHaveAttribute("aria-selected", "true");
    await expect(page).toHaveURL(/#\/hub\/settings\/history$/);
  });

  test("banner dismissal persists across a reload", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    const banner = page.getByTestId("wb-banner-hub.dictation.start");
    await expect(banner).toBeVisible();
    await expect(
      banner.getByRole("list", { name: "Things to try" }).getByRole("listitem"),
    ).toHaveCount(3);
    await banner.getByRole("button", { name: "Dismiss" }).click();
    await expect(banner).toBeHidden();
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "dismissed_banners", value: ["hub.dictation.start"] },
    });

    await page.reload();
    await expect(page.getByTestId("wb-empty-dictation")).toBeVisible();
    await expect(banner).toHaveCount(0);
  });

  test("a dismissed banner never flashes while settings load", async ({
    page,
    ipc,
    hub,
  }) => {
    hub.forkSettings.dismissed_banners = ["hub.dictation.start"];
    await openHub(page, ipc, hub);
    await expect(page.getByTestId("wb-empty-dictation")).toBeVisible();
    await expect(page.getByTestId("wb-banner-hub.dictation.start")).toHaveCount(
      0,
    );
  });

  test("the New! tag clears once its page is opened", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    const insights = page.getByTestId("wb-nav-insights");
    await expect(insights.getByTestId("wb-tag-new")).toHaveText("New!");
    await expect(
      page.getByTestId("wb-nav-transforms").getByTestId("wb-tag-beta"),
    ).toHaveText("Beta");

    await insights.click();
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "seen_new_tags", value: ["hub.page.insights"] },
    });
    await expect(insights.getByTestId("wb-tag-new")).toHaveCount(0);
    await page.reload();
    await expect(page.getByTestId("wb-sidebar")).toBeVisible();
    await expect(insights.getByTestId("wb-tag-new")).toHaveCount(0);
    // Beta tags stay.
    await expect(
      page.getByTestId("wb-nav-scratchpad").getByTestId("wb-tag-beta"),
    ).toBeVisible();
  });

  test("theme toggle switches light and dark and is saved", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub, { handy: { theme: "light" } });
    const toggle = page.getByTestId("wb-theme-toggle");
    const canvas = () =>
      page
        .getByTestId("wb-hub")
        .evaluate((el) => getComputedStyle(el).backgroundColor);
    const light = await canvas();
    await expect(toggle).toHaveAccessibleName("Switch to dark appearance");

    await toggle.click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
    await ipc.expectCalled("change_theme_setting", { theme: "dark" });
    await expect(toggle).toHaveAccessibleName("Switch to light appearance");
    expect(await canvas()).not.toBe(light);

    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

    await page.getByTestId("wb-theme-toggle").click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    expect(await canvas()).toBe(light);
  });

  test("from System, the toggle picks the opposite of what shows", async ({
    page,
    ipc,
    hub,
  }) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await openHub(page, ipc, hub, { handy: { theme: "system" } });
    await expect(page.getByTestId("wb-theme-toggle")).toHaveAttribute(
      "data-theme-showing",
      "dark",
    );
    await page.getByTestId("wb-theme-toggle").click();
    await ipc.expectCalled("change_theme_setting", { theme: "light" });
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  });

  test("deep links: URL hash, empty-state actions and hashchange", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub, { hash: "#/hub/settings/history" });
    await expect(main(page)).toHaveAttribute("data-page", "settings");
    await expect(page.getByRole("tab", { name: "History" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    // An empty state's one action is a deep link.
    await page.getByTestId("wb-nav-dictionary").click();
    await page.getByRole("button", { name: "Open custom words" }).click();
    await expect(main(page)).toHaveAttribute("data-page", "settings");
    await expect(page.getByRole("tab", { name: "Advanced" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(page).toHaveURL(/#\/hub\/settings\/advanced$/);

    // A link from outside (tray, menu, test) changes the hash.
    await page.evaluate(() => {
      window.location.hash = "#/hub/insights";
    });
    await expect(main(page)).toHaveAttribute("data-page", "insights");

    // Unknown pages and sections fall back safely.
    await page.evaluate(() => {
      window.location.hash = "#/hub/nope";
    });
    await expect(main(page)).toHaveAttribute("data-page", "insights");
    await page.evaluate(() => {
      window.location.hash = "#/hub/settings/toString";
    });
    await expect(page.getByRole("tab", { name: "General" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  test("file drops go only to the drop zone under the pointer (RE-33)", async ({
    page,
    ipc,
    hub,
  }) => {
    ipc.respond("transcribe_file", 1);
    await openHub(page, ipc, hub);
    await ipc.waitForListener("tauri://drag-drop");
    const files = ["/Users/test/notes.csv"];

    // Dictation has no drop zone: the drop is ignored.
    const where = await centerOf(page, "wb-main");
    await ipc.emit("tauri://drag-enter", { paths: files, position: where });
    await ipc.emit("tauri://drag-drop", { paths: files, position: where });
    await page.waitForTimeout(200);
    expect(await ipc.calls("transcribe_file")).toHaveLength(0);

    // History registers a zone; a drop outside it (the sidebar) is ignored…
    await page.getByTestId("wb-nav-help").click();
    await page.getByTestId("wb-menu-item-history").click();
    const zone = page.getByTestId("wb-drop-zone-history");
    await expect(zone).toBeVisible();
    const sidebar = await centerOf(page, "wb-sidebar");
    await ipc.emit("tauri://drag-drop", { paths: files, position: sidebar });
    await page.waitForTimeout(200);
    expect(await ipc.calls("transcribe_file")).toHaveLength(0);

    // …and a drop on it is handled by the page, with a highlight while over.
    const audio = ["/Users/test/memo.m4a"];
    const inZone = await centerOf(page, "wb-drop-zone-history");
    await ipc.emit("tauri://drag-enter", { paths: audio, position: inZone });
    await expect(zone).toHaveAttribute("data-drop-active", "true");
    await ipc.emit("tauri://drag-leave", {});
    await expect(zone).not.toHaveAttribute("data-drop-active", "true");
    await ipc.emit("tauri://drag-enter", { paths: audio, position: sidebar });
    await ipc.emit("tauri://drag-over", { position: inZone });
    await expect(zone).toHaveAttribute("data-drop-active", "true");
    await ipc.emit("tauri://drag-drop", { paths: audio, position: inZone });
    await ipc.expectCalled("transcribe_file", { path: audio[0] });
    await expect(zone).not.toHaveAttribute("data-drop-active", "true");
  });

  test("collapse shows icons only, keeps names and persists", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    const toggle = page.getByTestId("wb-sidebar-toggle");
    await expect(toggle).toHaveAccessibleName("Collapse sidebar");
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    const width = async () =>
      (await page.getByTestId("wb-sidebar").boundingBox())!.width;
    const wide = await width();

    await toggle.click();
    await expect(page.getByTestId("wb-sidebar")).toHaveAttribute(
      "data-collapsed",
      "true",
    );
    await expect(toggle).toHaveAccessibleName("Expand sidebar");
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect.poll(width).toBeLessThan(wide);
    // Labels are gone visually but every button keeps its name.
    await expect(page.getByTestId("wb-nav-insights")).toHaveAccessibleName(
      "Insights",
    );
    await expect(page.getByTestId("wb-nav-insights")).not.toContainText(
      "Insights",
    );
    await page.getByTestId("wb-nav-style").click();
    await expect(main(page)).toHaveAttribute("data-page", "style");

    await page.reload();
    await expect(page.getByTestId("wb-sidebar")).toHaveAttribute(
      "data-collapsed",
      "true",
    );
    await page.getByTestId("wb-sidebar-toggle").click();
    await expect(page.getByTestId("wb-sidebar")).not.toHaveAttribute(
      "data-collapsed",
      "true",
    );
  });

  test("footer shows background jobs and opens History", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await expect(page.getByTestId("wb-footer-version")).toHaveText(
      "v1.0.0-alpha.1",
    );
    await ipc.waitForListener("transcription-job");
    await ipc.emit("transcription-job", {
      entry_id: 7,
      status: { state: "running", percent: 40 },
    });
    const status = page
      .getByTestId("wb-footer-jobs")
      .getByRole("button", { name: /40/ });
    await expect(status).toBeVisible();
    await status.click();
    await expect(main(page)).toHaveAttribute("data-page", "settings");
    await expect(page.getByRole("tab", { name: "History" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  test("the model selector is not in the footer (B-207)", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await expect(
      page.getByTestId("wb-footer").getByRole("combobox"),
    ).toHaveCount(0);
    await expect(page.getByTestId("wb-footer").locator("select")).toHaveCount(
      0,
    );
  });

  test("RTL: the sidebar moves to the right and still works", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub, { handy: { app_language: "ar" } });
    await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
    const sidebar = await page.getByTestId("wb-sidebar").boundingBox();
    const mainBox = await main(page).boundingBox();
    expect(sidebar!.x).toBeGreaterThan(mainBox!.x);
    await page.getByTestId("wb-nav-dictionary").click();
    await expect(main(page)).toHaveAttribute("data-page", "dictionary");
    await expectNoSeriousAxeFindings(page);
  });

  test("reduced motion: the sidebar resizes without animation", async ({
    page,
    ipc,
    hub,
  }) => {
    const transition = () =>
      page
        .getByTestId("wb-sidebar")
        .evaluate((el) => getComputedStyle(el).transitionProperty);
    await openHub(page, ipc, hub);
    expect(await transition()).toBe("width");
    await page.emulateMedia({ reducedMotion: "reduce" });
    expect(await transition()).toBe("none");
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe: every page has no serious findings (${theme})`, async ({
      page,
      ipc,
      hub,
    }) => {
      await page.emulateMedia({ colorScheme: theme });
      await openHub(page, ipc, hub, { handy: { theme } });
      for (const id of HUB_PAGE_IDS) {
        await page.getByTestId(`wb-nav-${id}`).click();
        await expect(main(page)).toHaveAttribute("data-page", id);
        await expectNoSeriousAxeFindings(page);
      }
      await page.getByTestId("wb-notifications-bell").click();
      await expect(page.getByTestId("wb-notifications-panel")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-nav-help").click();
      await expect(page.getByTestId("wb-help-menu")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-sidebar-toggle").click();
      await expectNoSeriousAxeFindings(page);
    });
  }

  test("token pairs meet WCAG AA in light and dark", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    // [text, background, minimum ratio]
    const pairs: [string, string, number][] = [
      ["--wb-ink", "--wb-canvas", 4.5],
      ["--wb-ink", "--wb-sidebar", 4.5],
      ["--wb-ink", "--wb-card", 4.5],
      ["--wb-ink", "--wb-banner", 4.5],
      ["--wb-ink", "--wb-selected", 4.5],
      ["--wb-ink-2", "--wb-canvas", 4.5],
      ["--wb-ink-2", "--wb-sidebar", 4.5],
      ["--wb-ink-2", "--wb-banner", 4.5],
      ["--wb-ink-2", "--wb-selected", 4.5],
      ["--wb-ink-2", "--wb-card", 4.5],
      ["--wb-ink-3", "--wb-canvas", 4.5],
      ["--wb-ink-3", "--wb-card", 4.5],
      ["--wb-ink-3", "--wb-sidebar", 4.5],
      ["--wb-on-accent", "--wb-accent-fill", 4.5],
      ["--wb-accent", "--wb-canvas", 4.5],
      ["--wb-tag-new-ink", "--wb-tag-new-bg", 4.5],
      ["--wb-tag-beta-ink", "--wb-tag-beta-bg", 4.5],
      ["--wb-line-strong", "--wb-card", 3],
      ["--wb-accent", "--wb-sidebar", 3],
      // Handy's hint grey, retuned (C-19)
      ["--color-ink-3", "--color-background", 4.5],
    ];
    for (const theme of ["light", "dark"]) {
      await page.evaluate((t) => {
        document.documentElement.dataset.theme = t;
      }, theme);
      const results = await page.evaluate((list) => {
        const probe = document.createElement("div");
        document.body.appendChild(probe);
        const rgb = (v: string) => {
          probe.style.color = `var(${v})`;
          const m = getComputedStyle(probe).color.match(/[\d.]+/g)!;
          return m.slice(0, 3).map(Number);
        };
        const lum = ([r, g, b]: number[]) => {
          const f = (c: number) => {
            const s = c / 255;
            return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
          };
          return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
        };
        const out = list.map(([fg, bg, min]) => {
          const a = lum(rgb(fg));
          const b = lum(rgb(bg));
          const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
          return { fg, bg, min, ratio: Math.round(ratio * 100) / 100 };
        });
        probe.remove();
        return out;
      }, pairs);
      const failing = results.filter((r) => r.ratio < r.min);
      expect(failing, `${theme} theme`).toEqual([]);
    }
  });
});
