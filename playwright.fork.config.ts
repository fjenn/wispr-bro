// SPDX-License-Identifier: GPL-3.0-or-later
// Playwright config for Wispr Bro fork UI tests (R5 §4.2: Handy's
// playwright.config.ts is not edited). Run: bunx playwright test -c playwright.fork.config.ts
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/fork",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://localhost:1420",
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        // Optional: run on an installed browser instead of Playwright's
        // download, e.g. WB_PW_CHANNEL=chrome.
        ...(process.env.WB_PW_CHANNEL
          ? { channel: process.env.WB_PW_CHANNEL }
          : {}),
      },
    },
  ],
  webServer: {
    command: "bunx vite dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 30000,
  },
});
