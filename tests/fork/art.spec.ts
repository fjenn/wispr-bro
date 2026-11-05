// SPDX-License-Identifier: GPL-3.0-or-later
// Design W7 art: the banner photo and the empty-state character load in
// the real pages, the character's ink follows the app theme (light, dark,
// RTL), the art stays out of the accessibility tree and keyboard order,
// and axe finds nothing serious (docs/fork/design/SYSTEM-PLAN.md §6).
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
} from "./hub-fixture";

const BANNER = "wb-banner-hub.dictation.start";

for (const theme of ["light", "dark"] as const) {
  test(`home banner photo and history art load (${theme})`, async ({
    page,
    ipc,
    hub,
  }) => {
    await page.emulateMedia({ colorScheme: theme });
    await openHub(page, ipc, hub, { handy: { theme } });

    const photo = page.getByTestId(BANNER).locator("img");
    await expect(photo).toHaveAttribute("src", /banner-home@2x.*\.webp/);
    await expect(photo).toHaveAttribute("alt", "");
    await expect
      .poll(() => photo.evaluate((img: HTMLImageElement) => img.naturalWidth))
      .toBe(1920);

    const art = page
      .getByTestId("wb-empty-dictation")
      .locator('[data-art="empty-history"]');
    await expect(art.locator("svg")).toBeVisible();
    // Decorative: no role, name or tab stop.
    await expect(art).toHaveAttribute("aria-hidden", "true");
    await expect(art.locator("[tabindex]")).toHaveCount(0);
    const ink = await art
      .locator(".wbd-i")
      .first()
      .evaluate((el) => getComputedStyle(el).fill);
    expect(ink).toBe(
      theme === "light" ? "rgb(26, 26, 26)" : "rgb(243, 242, 238)",
    );
    await expectNoSeriousAxeFindings(page);
  });
}

test("art and photo keep their place in RTL", async ({ page, ipc, hub }) => {
  await openHub(page, ipc, hub, { handy: { app_language: "ar" } });
  await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
  const banner = page.getByTestId(BANNER);
  const photo = (await banner.locator("img").boundingBox())!;
  const box = (await banner.boundingBox())!;
  // The photo covers the banner; the grade mirrors, the photo does not.
  expect(Math.abs(photo.width - box.width)).toBeLessThan(2);
  await expect(page.locator('[data-art="empty-history"] svg')).toBeVisible();
  await expectNoSeriousAxeFindings(page);
});

test("keyboard: the art adds no tab stops", async ({ page, ipc, hub }) => {
  await openHub(page, ipc, hub);
  await expect(page.locator('[data-art="empty-history"] svg')).toBeVisible();
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press("Tab");
    const inArt = await page.evaluate(
      () => !!document.activeElement?.closest("[data-art]"),
    );
    expect(inArt).toBe(false);
  }
});
