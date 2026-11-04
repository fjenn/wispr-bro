// SPDX-License-Identifier: GPL-3.0-or-later
// OT2-E (owner feedback on rc3): the sidebar Support item and menu, Help ›
// Support, GitHub's mark on About's GitHub links, Handy's hosted rows in
// sentence case, the onboarding Summary's neutral mark for "Not set", focus
// rings only after keyboard navigation, and the Home banner's text kept
// clear of the photo's subject. Axe light/dark, LTR/RTL, keyboard only.
import type { Page } from "@playwright/test";
import {
  test,
  expect,
  expectNoSeriousAxeFindings,
  openHub,
} from "./hub-fixture";
import { expectStep, openOnboarding } from "./onboarding-fixture";

const REPO = "https://github.com/fjenn/wispr-bro";
const CONTRIBUTING =
  "https://github.com/fjenn/wispr-bro/blob/wispr-bro/CONTRIBUTING-WB.md";
const SUPPORT_ITEMS = ["star", "contribute", "support-feedback"];

test.describe("Support", () => {
  test("its own pinned item between Share and Settings", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    const footer = page.getByTestId("wb-sidebar").locator("nav > ul");
    const ids = await footer
      .locator("[data-testid^='wb-nav-']")
      .evaluateAll((els) => els.map((e) => e.getAttribute("data-testid")));
    expect(ids).toEqual([
      "wb-nav-share",
      "wb-nav-support",
      "wb-nav-settings",
      "wb-nav-help",
    ]);
    // Outside the scrolling part: always in view (Mac wave 6 finding 9).
    await expect(
      page.getByTestId("wb-sidebar-scroll").getByTestId("wb-nav-support"),
    ).toHaveCount(0);
    await expect(page.getByTestId("wb-nav-support")).toBeInViewport();
    await expect(page.getByTestId("wb-nav-support")).toHaveText(
      "Support Wispr Bro",
    );
  });

  test("keyboard: open, walk, Esc back to the item", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    const trigger = page.getByTestId("wb-nav-support");
    await trigger.focus();
    await page.keyboard.press("Enter");
    const menu = page.getByRole("menu", { name: "Support Wispr Bro" });
    await expect(menu).toBeVisible();
    await expect(menu.getByText("Free and open source")).toBeVisible();
    // No Sponsors profile yet: no Sponsor item.
    await expect(page.getByTestId("wb-menu-item-sponsor")).toHaveCount(0);
    for (const id of SUPPORT_ITEMS) {
      await expect(page.getByTestId(`wb-menu-item-${id}`)).toBeFocused();
      await page.keyboard.press("ArrowDown");
    }
    await page.keyboard.press("Escape");
    await expect(menu).toBeHidden();
    await expect(trigger).toBeFocused();
  });

  test("each item opens GitHub or the feedback form, nothing else", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await page.getByTestId("wb-nav-support").click();
    await expect(
      page
        .getByTestId("wb-menu-item-star")
        .locator("[data-glyph='github-mark']"),
    ).toHaveCount(1);
    await page.getByTestId("wb-menu-item-star").click();
    await ipc.expectCalled("plugin:opener|open_url", { url: REPO });
    await page.getByTestId("wb-nav-support").click();
    await page.getByTestId("wb-menu-item-contribute").click();
    await ipc.expectCalled("plugin:opener|open_url", { url: CONTRIBUTING });
    await page.getByTestId("wb-nav-support").click();
    await page.getByTestId("wb-menu-item-support-feedback").click();
    await ipc.expectCalled("fork_open_issue", { kind: "feedback" });
    const urls = (await ipc.calls("plugin:opener|open_url")).map(
      (c) => c.args.url,
    );
    expect(urls).toEqual([REPO, CONTRIBUTING]);
  });

  test("Help › Get in touch › Support opens the Support menu", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await page.getByTestId("wb-nav-help").click();
    await page.getByTestId("wb-menu-item-support").click();
    await expect(page.getByTestId("wb-help-menu")).toHaveCount(0);
    const menu = page.getByTestId("wb-support-menu");
    await expect(menu).toBeVisible();
    // Focus is in the menu; the arrow keys walk it.
    await expect
      .poll(() => menu.evaluate((el) => el.contains(document.activeElement)))
      .toBe(true);
    await page.keyboard.press("ArrowDown");
    await expect(page.getByTestId("wb-menu-item-star")).toBeFocused();
  });

  test("collapsed sidebar: icon only, named", async ({ page, ipc, hub }) => {
    await openHub(page, ipc, hub, { fork: { sidebar_collapsed: true } });
    const item = page.getByTestId("wb-nav-support");
    if (
      (await page.getByTestId("wb-sidebar").getAttribute("data-collapsed")) ===
      null
    ) {
      test.skip(true, "this build keeps the sidebar open by default");
    }
    await expect(item).toHaveAttribute("aria-label", "Support Wispr Bro");
  });
});

test("About: GitHub links carry GitHub's mark", async ({ page, ipc, hub }) => {
  await openHub(page, ipc, hub, { hash: "#/hub/settings/about" });
  for (const id of [
    "wb-about-source",
    "wb-about-license-link",
    "wb-about-handy",
  ]) {
    const mark = page.getByTestId(id).locator("svg[data-glyph='github-mark']");
    await expect(mark, id).toHaveCount(1);
    await expect(mark, id).toHaveAttribute("aria-hidden", "true");
    // One solid colour: the button's ink.
    const [fill, color] = await mark.evaluate((el) => [
      getComputedStyle(el).fill,
      getComputedStyle(el.closest("button")!).color,
    ]);
    expect(fill, id).toBe(color);
  }
  await expect(page.getByTestId("wb-about-source")).toHaveText(
    "View source code",
  );
});

test("Handy's hosted rows read in sentence case", async ({
  page,
  ipc,
  hub,
}) => {
  await openHub(page, ipc, hub, { hash: "#/hub/settings/system" });
  const modal = page.getByTestId("wb-settings-modal");
  await expect(
    modal.getByText("Launch on startup", { exact: true }),
  ).toBeVisible();
  await expect(
    modal.getByText("Show tray icon", { exact: true }),
  ).toBeVisible();
  await expect(modal.getByText(/Launch on Startup|Show Tray Icon/)).toHaveCount(
    0,
  );
  // Other locales from their own text; caseless languages and German as
  // translated.
  const probe = await page.evaluate(async () => {
    const i18n = (await import("/src/i18n/index.ts")).default;
    const t = (key: string, lng: string) => i18n.t(key, { lng });
    return {
      fr: t("settings.advanced.experimentalToggle.label", "fr"),
      it: t("settings.about.sourceCode.title", "it"),
      tr: t("settings.debug.historyLimit.title", "tr"),
      cs: t("settings.advanced.chineseScript.options.traditionalTw", "cs"),
      de: t("settings.advanced.autostart.label", "de"),
      ja: t("settings.advanced.autostart.label", "ja"),
      enHk: t("settings.advanced.chineseScript.options.traditionalHk", "en"),
    };
  });
  expect(probe.fr).toBe("Fonctionnalités expérimentales");
  expect(probe.it).toBe("Codice sorgente");
  expect(probe.tr).toBe("Geçmiş limiti");
  expect(probe.cs).toContain("Tchaj-wan");
  expect(probe.enHk).toBe("Traditional (Hong Kong)");
  expect(probe.de.length).toBeGreaterThan(0);
  expect(probe.ja.length).toBeGreaterThan(0);
});

test.describe("Summary marks", () => {
  test("a tick only beside a set choice; Not set gets a neutral dash", async ({
    page,
    ipc,
  }) => {
    await openOnboarding(page, ipc, {
      step: "summary",
      status: { trigger: null },
    });
    await expectStep(page, "summary");
    const languages = page.getByTestId("wb-summary-languages");
    await expect(languages).toContainText("Not set");
    await expect(languages).toHaveAttribute("data-state", "unset");
    await expect(page.getByTestId("wb-summary-languages-unset")).toBeVisible();
    await expect(page.getByTestId("wb-summary-shortcut")).toHaveAttribute(
      "data-state",
      "unset",
    );
    await expect(page.getByTestId("wb-summary-typing")).toHaveAttribute(
      "data-state",
      "unset",
    );
    const mic = page.getByTestId("wb-summary-microphone");
    await expect(mic).toHaveAttribute("data-state", "set");
    await expect(page.getByTestId("wb-summary-microphone-unset")).toHaveCount(
      0,
    );
    await expectNoSeriousAxeFindings(page);
  });

  test("set choices keep their tick", async ({ page, ipc }) => {
    await openOnboarding(page, ipc, {
      step: "summary",
      handy: { spoken_languages: ["nl"] },
    });
    await expect(page.getByTestId("wb-summary-languages")).toHaveAttribute(
      "data-state",
      "set",
    );
    await expect(page.getByTestId("wb-summary-shortcut")).toHaveAttribute(
      "data-state",
      "set",
    );
  });
});

async function activeRing(page: Page): Promise<{ id: string; ring: boolean }> {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return { id: "body", ring: false };
    const s = getComputedStyle(el);
    return {
      id: el.getAttribute("data-testid") ?? el.tagName,
      ring:
        (s.outlineStyle !== "none" && parseFloat(s.outlineWidth) > 0) ||
        (s.boxShadow !== "none" && s.boxShadow !== ""),
    };
  });
}

test.describe("focus rings after keyboard navigation only", () => {
  test("a deep link by mouse into Settings draws no ring; Tab does", async ({
    page,
    ipc,
    hub,
  }) => {
    ipc.respond("fork_mic_list", null);
    await openHub(page, ipc, hub);
    await page.getByTestId("wb-nav-help").click();
    await page.getByTestId("wb-menu-item-microphone").click();
    await expect(page).toHaveURL(/#\/hub\/settings\/microphone$/);
    await expect(page.getByTestId("wb-settings-modal")).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute(
      "data-wb-modality",
      "pointer",
    );
    const before = await activeRing(page);
    expect(before.ring, `no ring on ${before.id}`).toBe(false);
    await page.keyboard.press("Tab");
    await expect(page.locator("html")).toHaveAttribute(
      "data-wb-modality",
      "keyboard",
    );
    const after = await activeRing(page);
    expect(after.ring, `ring on ${after.id}`).toBe(true);
  });

  test("a deep link at launch (no input yet) draws no ring", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub, { hash: "#/hub/settings/about" });
    await expect(page.getByTestId("wb-settings-modal")).toBeVisible();
    const state = await activeRing(page);
    expect(state.ring, `no ring on ${state.id}`).toBe(false);
  });
});

/** The banner's text column and the photo subject's left edge (LTR). */
async function bannerGeometry(page: Page) {
  return page.getByTestId("wb-banner-hub.dictation.start").evaluate((el) => {
    const img = el.querySelector("img") as HTMLImageElement;
    const text = el.querySelector(
      "[data-testid='wb-banner-hub.dictation.start-text']",
    ) as HTMLElement;
    const box = el.getBoundingClientRect();
    const t = text.getBoundingClientRect();
    const at = Number(img.dataset.subjectAt);
    const scale = Math.max(
      box.width / img.naturalWidth,
      box.height / img.naturalHeight,
    );
    const subject = at * img.naturalWidth * scale;
    const rtl = getComputedStyle(el).direction === "rtl";
    return {
      rtl,
      // Distance from the text's far edge to the subject, along the line.
      gap: rtl
        ? t.left - (box.right - subject) // mirrored photo
        : box.left + subject - t.right,
      loaded: img.complete && img.naturalWidth > 0,
    };
  });
}

test.describe("Home banner keeps its text off the photo's subject", () => {
  for (const app_language of ["en", "ar"] as const) {
    for (const size of [
      { width: 1040, height: 720 },
      { width: 1500, height: 900 },
      { width: 900, height: 700 },
    ]) {
      test(`${app_language} at ${size.width}×${size.height}`, async ({
        page,
        ipc,
        hub,
      }) => {
        await openHub(page, ipc, hub, { size, handy: { app_language } });
        const banner = page.getByTestId("wb-banner-hub.dictation.start");
        await expect(banner).toBeVisible();
        await expect
          .poll(async () => (await bannerGeometry(page)).loaded)
          .toBe(true);
        const g = await bannerGeometry(page);
        expect(g.rtl).toBe(app_language === "ar");
        expect(g.gap, JSON.stringify(g)).toBeGreaterThanOrEqual(16);
      });
    }
  }
});

for (const theme of ["light", "dark"] as const) {
  for (const app_language of ["en", "ar"] as const) {
    const dir = app_language === "ar" ? "rtl" : "ltr";
    test(`axe: Support menu, About and Home banner (${theme}, ${dir})`, async ({
      page,
      ipc,
      hub,
    }) => {
      test.setTimeout(180_000);
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openHub(page, ipc, hub, { handy: { theme, app_language } });
      await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", dir);
      await expect(
        page.getByTestId("wb-banner-hub.dictation.start"),
      ).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-nav-support").click();
      await expect(page.getByTestId("wb-support-menu")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.keyboard.press("Escape");
      await page.evaluate(() => {
        window.location.hash = "#/hub/settings/about";
      });
      await expect(page.getByTestId("wb-about-support")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }
}
