// SPDX-License-Identifier: GPL-3.0-or-later
// S-13: screenshots of every Hub page in light and dark at the window sizes
// the slice is checked at, for a person (or an agent) to look at. Skipped
// unless WB_SHOTS_DIR is set:
//
//   WB_SHOTS_DIR=/tmp/hub-shots bun run wb:test:ui -- hub-screens
//
// The images show mocked data only (no personal data).
import path from "node:path";
import { HUB_PAGE_IDS } from "../../src/fork/hub/nav";
import { test, openHub } from "./hub-fixture";

const dir = process.env.WB_SHOTS_DIR;

const SIZES = [
  { name: "min", width: 720, height: 600 },
  { name: "default", width: 1040, height: 720 },
  { name: "large", width: 1440, height: 900 },
];

test.describe("hub screenshots", () => {
  test.skip(!dir, "set WB_SHOTS_DIR to write screenshots");

  for (const theme of ["light", "dark"] as const) {
    for (const size of SIZES) {
      test(`${theme} ${size.name}`, async ({ page, ipc, hub }) => {
        await page.emulateMedia({ colorScheme: theme });
        await openHub(page, ipc, hub, {
          handy: { theme },
          size: { width: size.width, height: size.height },
        });
        for (const id of HUB_PAGE_IDS) {
          await page.getByTestId(`wb-nav-${id}`).click();
          await page.waitForTimeout(150);
          await page.screenshot({
            path: path.join(dir!, `${theme}-${size.name}-${id}.png`),
          });
        }
        await page.getByTestId("wb-nav-dictation").click();
        await page.getByTestId("wb-sidebar-toggle").click();
        await page.waitForTimeout(250);
        await page.screenshot({
          path: path.join(dir!, `${theme}-${size.name}-collapsed.png`),
        });
        await page.getByTestId("wb-sidebar-toggle").click();
        await page.getByTestId("wb-notifications-bell").click();
        await page.waitForTimeout(200);
        await page.screenshot({
          path: path.join(dir!, `${theme}-${size.name}-notifications.png`),
        });
        await page.keyboard.press("Escape");
        await page.getByTestId("wb-nav-help").click();
        await page.waitForTimeout(200);
        await page.screenshot({
          path: path.join(dir!, `${theme}-${size.name}-help.png`),
        });
      });
    }
  }
});
