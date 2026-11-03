// SPDX-License-Identifier: GPL-3.0-or-later
// Design Phase 0: before/after screenshots of every Hub page, every Settings
// section and every onboarding step, light and dark, at the Hub's default
// size. Skipped unless WB_DESIGN_SHOTS_DIR is set:
//
//   WB_DESIGN_SHOTS_DIR=/tmp/shots bun run wb:test:ui -- design-shots
//
// The images show mocked data only (no personal data).
import path from "node:path";
import { HUB_PAGE_IDS } from "../../src/fork/hub/nav";
import { test, openHub, handySettings } from "./hub-fixture";
import { openSettingsModal } from "./settings-modal-fixture";
import { openOnboarding, STEPS } from "./onboarding-fixture";

const dir = process.env.WB_DESIGN_SHOTS_DIR;
const SETTINGS = [
  "general",
  "system",
  "vibe",
  "privacy",
  "advanced",
  "about",
  "hotkeys",
];

test.describe("design shots", () => {
  test.skip(!dir, "set WB_DESIGN_SHOTS_DIR to write screenshots");
  test.setTimeout(180_000);

  for (const theme of ["light", "dark"] as const) {
    test(`hub ${theme}`, async ({ page, ipc, hub }) => {
      await page.emulateMedia({ colorScheme: theme });
      Object.assign(hub.stats, {
        total_words: 1_284,
        total_dictations: 57,
        words_per_minute: 142.6,
        current_streak_days: 4,
      });
      await openHub(page, ipc, hub, {
        handy: { theme },
        size: { width: 1040, height: 720 },
      });
      for (const id of HUB_PAGE_IDS) {
        await page.getByTestId(`wb-nav-${id}`).click();
        await page.waitForTimeout(450);
        await page.screenshot({
          path: path.join(dir!, `hub-${id}-${theme}.png`),
        });
      }
    });

    test(`settings ${theme}`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme });
      for (const section of SETTINGS) {
        const handy = handySettings({
          theme,
          post_process_providers: [],
          post_process_provider_id: "local",
          post_process_api_keys: {},
          post_process_models: {},
          post_process_prompts: [],
        });
        await openSettingsModal(page, ipc, section, {
          get_app_settings: handy,
          get_default_settings: handy,
        });
        await page.waitForTimeout(450);
        await page.screenshot({
          path: path.join(dir!, `settings-${section}-${theme}.png`),
        });
      }
    });

    for (const step of STEPS) {
      test(`onboarding ${step} ${theme}`, async ({ page, ipc }) => {
        await page.emulateMedia({ colorScheme: theme });
        await openOnboarding(page, ipc, {
          step,
          theme,
          location: step === "move_to_applications" ? "move_needed" : undefined,
        });
        await page.setViewportSize({ width: 1040, height: 720 });
        await page.waitForTimeout(600);
        await page.screenshot({
          path: path.join(dir!, `onboarding-${step}-${theme}.png`),
        });
      });
    }
  }
});
