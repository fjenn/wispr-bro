// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Hub's one notice slot (`src/fork/hub/HubNotices.tsx`): shell notices
 * never stack. With a lost shortcut, Secure Input and the Apple Dictation
 * note all due, the shortcut notice shows alone with "2 more", which steps
 * through the others; dismissing one brings up the next. Axe in light and
 * dark, RTL, keyboard only.
 */
import type { Page } from "@playwright/test";
import type { HealthStatus, HotkeyStatus } from "../../src/fork/bindings";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
  type OpenHubOptions,
} from "./hub-fixture";
import type { IpcMock } from "./fixtures";

const LOST: HealthStatus = {
  state: "lost",
  reason: "permission",
  fork_tap: "lost",
  handy_tap: "lost",
  needs_relaunch: false,
};
const HEALTHY: HealthStatus = {
  state: "healthy",
  reason: null,
  fork_tap: "healthy",
  handy_tap: "healthy",
  needs_relaunch: false,
};
const SECURE = {
  active: true,
  sustained: true,
  culprit_name: "Terminal",
  degraded_bindings: [],
  uncovered_bindings: ["transcribe"],
  recorder_blocked: false,
};
const APPLE_OFF: HotkeyStatus = {
  running: true,
  error: null,
  trigger: "fn",
  globe: "do_nothing",
  fn_blocked: false,
  apple_dictation: "turned_off",
  issues: [],
  bindings: [],
};

async function open(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  due: { shortcut?: boolean; secure?: boolean; apple?: boolean },
  options: OpenHubOptions = {},
) {
  const health = due.shortcut ? LOST : HEALTHY;
  await openHub(page, ipc, hub, {
    ...options,
    beforeNavigate: () => {
      ipc.respond({
        fork_shortcut_health: health,
        fork_shortcut_recheck: health,
        get_secure_input_status: due.secure
          ? SECURE
          : { active: false, sustained: false },
        "plugin:opener|open_url": null,
      });
      ipc.respond(
        "fork_hotkey_status",
        due.apple ? APPLE_OFF : { ...APPLE_OFF, apple_dictation: "none" },
      );
    },
  });
}

const notices = (page: Page) =>
  page.locator("[data-notice]", { has: page.locator("p") });

test("one notice at a time, highest priority first; N more steps through", async ({
  page,
  ipc,
  hub,
}) => {
  await open(page, ipc, hub, { shortcut: true, secure: true, apple: true });
  const next = page.getByTestId("wb-hub-notice-next");
  await expect(page.getByTestId("wb-shortcut-banner")).toBeVisible({
    timeout: 30_000,
  });
  await expect(next).toHaveText(/2 more$/);
  await expect(notices(page)).toHaveCount(1);
  await next.click();
  await expect(page.getByTestId("wb-secure-input-notice")).toContainText(
    "Terminal may be blocking 1 shortcut",
  );
  await expect(notices(page)).toHaveCount(1);
  await next.click();
  await expect(page.getByTestId("wb-apple-dictation-note")).toBeVisible();
  await next.click();
  await expect(page.getByTestId("wb-shortcut-banner")).toBeVisible();
  // The actions are the notices' own.
  await next.click();
  await page.getByTestId("wb-secure-input-help").click();
  await ipc.expectCalled("plugin:opener|open_url");
  await page.getByTestId("wb-secure-input-dismiss").click();
  // Dismissed: the first remaining one shows, with the other behind it.
  await expect(page.getByTestId("wb-shortcut-banner")).toBeVisible();
  await expect(next).toHaveText(/1 more$/);
  await expect(page.getByTestId("wb-secure-input-notice")).toHaveCount(0);
});

test("dismissing the only other notice leaves no 'more'", async ({
  page,
  ipc,
  hub,
}) => {
  await open(page, ipc, hub, { secure: true, apple: true });
  await expect(page.getByTestId("wb-secure-input-notice")).toBeVisible({
    timeout: 30_000,
  });
  await page.getByTestId("wb-secure-input-dismiss").click();
  await expect(page.getByTestId("wb-apple-dictation-note")).toBeVisible();
  await expect(page.getByTestId("wb-hub-notice-next")).toHaveCount(0);
  await page.getByTestId("wb-apple-dictation-note-dismiss").click();
  await expect(notices(page)).toHaveCount(0);
  expect(hub.forkSettings.dismissed_banners).toContain(
    "hub.apple-dictation-off",
  );
});

test("keyboard only: More and the notice's actions", async ({
  page,
  ipc,
  hub,
}) => {
  await open(page, ipc, hub, { shortcut: true, apple: true });
  const next = page.getByTestId("wb-hub-notice-next");
  await expect(next).toBeVisible({ timeout: 30_000 });
  await next.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("wb-apple-dictation-note")).toBeVisible();
  // The slot is reused, so focus stays on More.
  await expect(next).toBeFocused();
  await page.keyboard.press("Space");
  await expect(page.getByTestId("wb-shortcut-banner")).toBeVisible();
  await page.keyboard.press("Shift+Tab");
  await expect(page.getByTestId("wb-shortcut-banner-fix")).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("wb-fix-sheet")).toBeVisible();
});

test("RTL: the slot mirrors", async ({ page, ipc, hub }) => {
  test.setTimeout(180_000);
  await open(
    page,
    ipc,
    hub,
    { shortcut: true, secure: true },
    { handy: { app_language: "ar" } },
  );
  await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
  const slot = page.getByTestId("wb-shortcut-banner");
  await expect(slot).toBeVisible({ timeout: 30_000 });
  const box = (await slot.boundingBox())!;
  const next = (await page.getByTestId("wb-hub-notice-next").boundingBox())!;
  expect(next.x).toBeLessThan(box.x + box.width / 2);
  await expectNoSeriousAxeFindings(page);
});

for (const theme of ["light", "dark"] as const) {
  test(`axe: each notice in the slot (${theme})`, async ({
    page,
    ipc,
    hub,
  }) => {
    test.setTimeout(180_000);
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await open(
      page,
      ipc,
      hub,
      { shortcut: true, secure: true, apple: true },
      { handy: { theme } },
    );
    await expect(page.getByTestId("wb-shortcut-banner")).toBeVisible({
      timeout: 30_000,
    });
    for (let i = 0; i < 3; i++) {
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-hub-notice-next").click();
    }
  });
}
