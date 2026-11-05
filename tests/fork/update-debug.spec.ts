// SPDX-License-Identifier: GPL-3.0-or-later
// F-02: the debug panel's Updates section (status, Check now, Install,
// checks off, errors; keyboard, RTL, axe in light and dark).
import AxeBuilder from "@axe-core/playwright";
import { test, expect, ipcError } from "./fixtures";
import type { IpcMock } from "./fixtures";
import type { Page } from "@playwright/test";

const status = (over: Record<string, unknown> = {}) => ({
  current_version: "1.0.0-alpha.1",
  channel: "stable",
  endpoints: [
    "https://github.com/fjenn/wispr-bro/releases/latest/download/latest.json",
  ],
  checks_enabled: true,
  pubkey_id: "89E085217ACC46CF",
  ...over,
});

const available = {
  current_version: "1.0.0-alpha.1",
  channel: "stable",
  available: {
    version: "1.0.0-alpha.2",
    date: "2026-11-02T12:00:00Z",
    notes: "Wispr Bro 1.0.0-alpha.2",
    endpoint:
      "https://github.com/fjenn/wispr-bro/releases/latest/download/latest.json",
  },
};

async function open(page: Page, ipc: IpcMock, s = status()) {
  ipc.respond({
    fork_ping: {
      message: "pong",
      seq: 0,
      ready: true,
      debug_build: true,
      cli_debug: false,
    },
    fork_model_status: { known_models: 1, downloaded_model_ids: [] },
    fork_notifications_permission: "granted",
    fork_update_status: s,
  });
  await page.goto("/#/fork/debug");
  const section = page.getByTestId("fork-debug-section-update");
  await expect(section).toBeVisible();
  await expect(page.getByTestId("fork-debug-update-version")).toHaveText(
    String(s.current_version),
  );
  return section;
}

test.describe("fork debug panel: updates", () => {
  test("shows version, channel, key and feed", async ({ page, ipc }) => {
    const section = await open(page, ipc);
    await expect(page.getByTestId("fork-debug-update-channel")).toHaveText(
      "Stable",
    );
    await expect(section).toContainText("89E085217ACC46CF");
    await expect(section).toContainText(
      "github.com/fjenn/wispr-bro/releases/latest/download/latest.json",
    );
    await expect(page.getByTestId("fork-debug-update-off")).toHaveCount(0);
  });

  test("Check now finds an update and Install calls the backend", async ({
    page,
    ipc,
  }) => {
    const section = await open(page, ipc);
    ipc.respond({ fork_update_check: available, fork_update_install: null });
    await section.getByRole("button", { name: "Check now" }).click();
    await expect(page.getByTestId("fork-debug-update-message")).toHaveText(
      "1.0.0-alpha.2 is available.",
    );
    await section
      .getByRole("button", { name: "Install 1.0.0-alpha.2 and relaunch" })
      .click();
    await ipc.expectCalled("fork_update_install");
    await expect(page.getByTestId("fork-debug-update-message")).toHaveText(
      "Up to date.",
    );
  });

  test("up to date and failures are reported", async ({ page, ipc }) => {
    const section = await open(page, ipc);
    ipc.respond({
      fork_update_check: { ...available, available: null },
    });
    await section.getByRole("button", { name: "Check now" }).click();
    await expect(page.getByTestId("fork-debug-update-message")).toHaveText(
      "Up to date.",
    );
    await expect(section.getByRole("button", { name: /^Install/ })).toHaveCount(
      0,
    );
    ipc.respond({
      fork_update_check: ipcError({ kind: "Internal", message: "offline" }),
    });
    await section.getByRole("button", { name: "Check now" }).click();
    await expect(page.getByTestId("fork-debug-update-message")).toHaveText(
      "Couldn't check: offline",
    );
  });

  test("checks off: the button is disabled and says why", async ({
    page,
    ipc,
  }) => {
    const section = await open(
      page,
      ipc,
      status({ checks_enabled: false, channel: "beta" }),
    );
    await expect(page.getByTestId("fork-debug-update-off")).toBeVisible();
    await expect(page.getByTestId("fork-debug-update-channel")).toHaveText(
      "Beta",
    );
    await expect(
      section.getByRole("button", { name: "Check now" }),
    ).toBeDisabled();
  });

  test("keyboard only: Check now and Install", async ({ page, ipc }) => {
    const section = await open(page, ipc);
    ipc.respond({ fork_update_check: available, fork_update_install: null });
    const check = section.getByRole("button", { name: "Check now" });
    await check.focus();
    await page.keyboard.press("Enter");
    const install = section.getByRole("button", {
      name: "Install 1.0.0-alpha.2 and relaunch",
    });
    await expect(install).toBeVisible();
    await page.keyboard.press("Tab");
    await expect(install).toBeFocused();
    await page.keyboard.press("Space");
    await ipc.expectCalled("fork_update_install");
  });

  test("RTL keeps feeds left-to-right and inside the section", async ({
    page,
    ipc,
  }) => {
    const section = await open(page, ipc);
    await page.evaluate(() => {
      document.documentElement.dir = "rtl";
    });
    const box = await section.boundingBox();
    const feed = await section.locator("ul[dir=ltr] li").first().boundingBox();
    expect(box && feed).toBeTruthy();
    expect(feed!.x).toBeGreaterThanOrEqual(box!.x);
    expect(feed!.x + feed!.width).toBeLessThanOrEqual(box!.x + box!.width + 1);
  });

  for (const theme of ["light", "dark"] as const) {
    test(`no serious axe findings, ${theme}`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await open(page, ipc);
      ipc.respond({ fork_update_check: available });
      await page
        .getByTestId("fork-debug-section-update")
        .getByRole("button", { name: "Check now" })
        .click();
      await expect(page.getByTestId("fork-debug-update-message")).toHaveText(
        "1.0.0-alpha.2 is available.",
      );
      const results = await new AxeBuilder({ page })
        .include('[data-testid="fork-debug-section-update"]')
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .analyze();
      const serious = results.violations.filter(
        (v) => v.impact === "serious" || v.impact === "critical",
      );
      expect(serious.map((v) => v.id)).toEqual([]);
    });
  }
});
