// SPDX-License-Identifier: GPL-3.0-or-later
// S-02: hotkey self-healing and Secure Input in the UI, with a mocked
// backend. The Hub banner and the Fix sheet (open from the banner, the
// bar's Fix event and the notification's section; Open Settings, Check
// again, Reset and re-grant with its failure text, Relaunch; live recovery),
// and the bar's two new states: "Your shortcut stopped working" with Fix
// and the Secure Input notice with and without the holder. Keyboard only,
// RTL, light and dark with axe. The health check itself (probe tap,
// notifications, rebuilds) is Rust (`fork::shortcut::health`) and the Mac
// wave (RS-02/RS-14 drivers in scripts/wb-s02/).
import { readFileSync } from "node:fs";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import type { ForkPillState, HealthStatus } from "../../src/fork/bindings";
import { ipcError, type IpcMock } from "./fixtures";
import { expectRtl, RTL_APP_SETTINGS } from "./pill-page";
import {
  test,
  expect,
  expectNoSeriousAxeFindings,
  openHub,
  type HubState,
  type OpenHubOptions,
} from "./hub-fixture";

const en = JSON.parse(
  readFileSync(
    new URL("../../src/i18n/locales/en/fork.json", import.meta.url),
    "utf8",
  ),
);
const geometry = JSON.parse(
  readFileSync(
    new URL("../../src/fork/pill/geometry.gen.json", import.meta.url),
    "utf8",
  ),
) as { shapes: Record<ForkPillState["shape"], { w: number; h: number }> };

const HEALTHY: HealthStatus = {
  state: "healthy",
  reason: null,
  fork_tap: "healthy",
  handy_tap: "healthy",
  needs_relaunch: false,
};
const LOST: HealthStatus = {
  state: "lost",
  reason: "permission",
  fork_tap: "lost",
  handy_tap: "lost",
  needs_relaunch: false,
};

async function open(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  status: HealthStatus,
  options: OpenHubOptions = {},
): Promise<void> {
  ipc.respond({
    fork_shortcut_health: status,
    fork_shortcut_recheck: status,
    fork_shortcut_open_accessibility: null,
    fork_shortcut_reset_permission: null,
    fork_shortcut_relaunch: null,
  });
  await openHub(page, ipc, hub, options);
}

test.describe("Hub: banner and Fix sheet", () => {
  test("no banner while the shortcut works", async ({ page, ipc, hub }) => {
    await open(page, ipc, hub, HEALTHY);
    await ipc.expectCalled("fork_shortcut_health");
    await expect(page.getByTestId("wb-shortcut-banner")).toHaveCount(0);
  });

  test("a lost shortcut shows the banner; Fix opens the sheet", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, LOST);
    const banner = page.getByTestId("wb-shortcut-banner");
    await expect(banner).toHaveText(
      new RegExp(en.shortcutHealth.banner.lost.slice(0, 30)),
    );
    await page.getByTestId("wb-shortcut-banner-fix").click();
    const sheet = page.getByTestId("wb-fix-sheet");
    await expect(sheet).toBeVisible();
    await expect(
      page.getByRole("dialog", { name: en.shortcutHealth.sheet.title }),
    ).toBeVisible();
    await expect(page.getByTestId("wb-fix-status")).toHaveAttribute(
      "data-state",
      "lost",
    );
    await expect(
      page.getByRole("img", { name: en.shortcutHealth.sheet.pictureLabel }),
    ).toBeVisible();
    await page.getByTestId("wb-fix-open-settings").click();
    await ipc.expectCalled("fork_shortcut_open_accessibility");
    await page.getByTestId("wb-fix-check").click();
    await ipc.expectCalled("fork_shortcut_recheck");
  });

  test("the shortcut comes back on its own: status and banner follow", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, LOST);
    await page.getByTestId("wb-shortcut-banner-fix").click();
    await ipc.waitForListener("fork-shortcut-health-changed");
    await ipc.emit("fork-shortcut-health-changed", {
      status: {
        ...LOST,
        state: "recovering",
        fork_tap: "recovering",
        handy_tap: "recovering",
      },
    });
    await expect(page.getByTestId("wb-fix-status")).toHaveAttribute(
      "data-state",
      "recovering",
    );
    await ipc.emit("fork-shortcut-health-changed", { status: HEALTHY });
    await expect(page.getByTestId("wb-fix-status")).toHaveText(
      en.shortcutHealth.sheet.status.healthy,
    );
    await page.getByTestId("wb-fix-close").click();
    await expect(page.getByTestId("wb-shortcut-banner")).toHaveCount(0);
  });

  test("a stopped listener says so and offers Relaunch when needed", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, {
      ...LOST,
      reason: "tap_stopped",
      handy_tap: "healthy",
      needs_relaunch: true,
    });
    await expect(page.getByTestId("wb-shortcut-banner")).toHaveText(
      new RegExp(en.shortcutHealth.banner.stopped.slice(0, 30)),
    );
    await page.getByTestId("wb-shortcut-banner-fix").click();
    await expect(page.getByTestId("wb-fix-status")).toHaveText(
      en.shortcutHealth.sheet.status.needsRelaunch,
    );
    await page.getByTestId("wb-fix-relaunch").click();
    await ipc.expectCalled("fork_shortcut_relaunch");
  });

  test("Reset and re-grant; a failed reset explains the manual way", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, LOST);
    await page.getByTestId("wb-shortcut-banner-fix").click();
    await page.getByTestId("wb-fix-reset").click();
    await ipc.expectCalled("fork_shortcut_reset_permission");
    await expect(page.getByTestId("wb-fix-reset-failed")).toHaveCount(0);
    ipc.respond(
      "fork_shortcut_reset_permission",
      ipcError({ kind: "Internal", message: "tccutil exited with 1" }),
    );
    await page.getByTestId("wb-fix-reset").click();
    await expect(page.getByTestId("wb-fix-reset-failed")).toHaveText(
      en.shortcutHealth.sheet.resetFailed,
    );
  });

  test("the bar's Fix and the notification open the sheet", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, LOST);
    await ipc.waitForListener("fork-shortcut-fix-requested");
    await ipc.emit("fork-shortcut-fix-requested", { source: "pill" });
    await expect(page.getByTestId("wb-fix-sheet")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("wb-fix-sheet")).toHaveCount(0);

    await open(page, ipc, hub, LOST, { hash: "#/hub/dictation/fix-shortcut" });
    await expect(page.getByTestId("wb-fix-sheet")).toBeVisible();
  });

  test("keyboard only: Fix, every sheet button, Esc closes", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, LOST);
    await page.getByTestId("wb-shortcut-banner-fix").focus();
    await page.keyboard.press("Enter");
    // Focus starts on the main action.
    await expect(page.getByTestId("wb-fix-open-settings")).toBeFocused();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_shortcut_open_accessibility");
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("wb-fix-reset")).toBeFocused();
    await page.keyboard.press("Space");
    await ipc.expectCalled("fork_shortcut_reset_permission");
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("wb-fix-sheet")).toHaveCount(0);
  });

  test("RTL: banner and sheet mirror and stay accessible", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, LOST, { handy: { app_language: "ar" } });
    await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
    const banner = (await page
      .getByTestId("wb-shortcut-banner")
      .boundingBox())!;
    const fix = (await page
      .getByTestId("wb-shortcut-banner-fix")
      .boundingBox())!;
    expect(fix.x).toBeLessThan(banner.x + banner.width / 2);
    await page.getByTestId("wb-shortcut-banner-fix").click();
    await expect(page.getByTestId("wb-fix-sheet")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe: banner and sheet (${theme})`, async ({ page, ipc, hub }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await open(page, ipc, hub, LOST, { handy: { theme } });
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-shortcut-banner-fix").click();
      await expect(page.getByTestId("wb-fix-sheet")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }
});

// ------------------------------------------------------------------ the bar

const PILL = "/src/fork/pill/index.html";

function pillState(partial: Partial<ForkPillState>): ForkPillState {
  return {
    seq: 1,
    phase: "message",
    shape: "message",
    mic: null,
    message: null,
    tone: null,
    fix: null,
    warning: null,
    progress: null,
    text: null,
    card_reason: null,
    more: 0,
    copied: false,
    sends_automatically: false,
    context: false,
    diff: null,
    holder: null,
    ...partial,
  };
}

const LOST_PILL = pillState({
  message: "shortcut_lost",
  tone: "error",
  fix: "fix_shortcut",
});

function secureState(locked: boolean, holder: string | null): ForkPillState {
  return pillState({
    phase: locked ? "locked" : "held",
    shape: locked ? "locked_notice" : "held_notice",
    warning: "secure_input",
    holder,
  });
}

/**
 * Opens the bar on `state` and waits until it is settled: the page loaded,
 * the state painted (the page reports it with `fork_pill_ready`, as it does
 * to the backend) and, for an RTL language, the direction applied. Every
 * wait is on an event or state, bounded only by the test's timeout, so a
 * slow first compile of the page under load is waited out (the bar tests
 * timed out under load before). Reduce Motion keeps the sizes still, so
 * boxes are measured at rest, not mid-morph.
 */
async function openPill(
  page: Page,
  ipc: IpcMock,
  state: ForkPillState,
  appSettings: unknown = null,
): Promise<void> {
  ipc.respond({ fork_pill_snapshot: state, get_app_settings: appSettings });
  const size = geometry.shapes[state.shape];
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: size.w, height: size.h });
  await page.goto(PILL, { waitUntil: "load" });
  await page.waitForSelector(
    `[data-testid="wb-pill"][data-phase="${state.phase}"]`,
    {
      state: "visible",
      timeout: 0,
    },
  );
  await expect
    .poll(async () => (await ipc.calls("fork_pill_ready")).length, {
      timeout: 0,
    })
    .toBeGreaterThan(0); // the call log is the page's: a new load starts at 0
}

test.describe("the bar", () => {
  test.use({ ipcSetup: { windowLabel: "fork-pill" } });

  test("a lost shortcut: the message, its reason, Fix and Dismiss", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, LOST_PILL);
    const msg = page.getByTestId("wb-pill-shortcut-lost");
    await expect(msg).toContainText(en.rust.pill.message.shortcut_lost);
    await expect(msg).toHaveAttribute(
      "title",
      en.pill.shortcutDetail.permission,
    );
    await page.getByTestId("wb-pill-fix").click();
    await ipc.expectCalled("fork_pill_action", { action: "fix" });
    await page.getByTestId("wb-pill-dismiss").click();
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
    const box = (await page.getByTestId("wb-pill").boundingBox())!;
    for (const id of ["wb-pill-fix", "wb-pill-dismiss"]) {
      const b = (await page.getByTestId(id).boundingBox())!;
      expect(b.x + b.width).toBeLessThanOrEqual(box.width);
    }
  });

  test("a stopped listener and the recovery message", async ({ page, ipc }) => {
    await openPill(
      page,
      ipc,
      pillState({
        message: "shortcut_stopped",
        tone: "error",
        fix: "fix_shortcut",
      }),
    );
    await expect(page.getByTestId("wb-pill-shortcut-lost")).toHaveAttribute(
      "title",
      en.pill.shortcutDetail.stopped,
    );
    await ipc.waitForListener("fork-pill-state");
    await ipc.emit(
      "fork-pill-state",
      pillState({ seq: 2, message: "shortcut_back", tone: "info" }),
    );
    await expect(page.getByTestId("wb-pill-message")).toHaveText(
      en.rust.pill.message.shortcut_back,
    );
  });

  test("Secure Input: the notice names the holder and fits the bar", async ({
    page,
    ipc,
  }) => {
    for (const locked of [false, true]) {
      await openPill(page, ipc, secureState(locked, "Terminal"));
      const notice = page.getByTestId("wb-pill-secure-input");
      const text = en.pill.warning.secure_input_app.replace(
        "{{app}}",
        "Terminal",
      );
      await expect(notice).toHaveAttribute("title", text);
      const pill = (await page.getByTestId("wb-pill").boundingBox())!;
      expect(pill.width).toBe(
        geometry.shapes[locked ? "locked_notice" : "held_notice"].w,
      );
      const b = (await notice.boundingBox())!;
      expect(b.x + b.width).toBeLessThanOrEqual(pill.width);
      if (locked) {
        await expect(page.getByTestId("wb-pill-cancel")).toBeVisible();
      }
    }
    await openPill(page, ipc, secureState(false, null));
    await expect(page.getByTestId("wb-pill-secure-input")).toHaveAttribute(
      "title",
      en.pill.warning.secure_input,
    );
  });

  test("keyboard only: Fix and Dismiss on the lost message", async ({
    page,
    ipc,
  }) => {
    await openPill(page, ipc, LOST_PILL);
    await page.getByTestId("wb-pill-fix").focus();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_pill_action", { action: "fix" });
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("wb-pill-dismiss")).toBeFocused();
    await page.keyboard.press("Space");
    await ipc.expectCalled("fork_pill_action", { action: "dismiss" });
  });

  test("RTL mirrors the notice and the message", async ({ page, ipc }) => {
    // The page sets its own direction from the app language (pill-page.ts).
    await openPill(page, ipc, LOST_PILL, RTL_APP_SETTINGS);
    await expectRtl(page);
    await expect
      .poll(async () => {
        const msg = (await page
          .getByTestId("wb-pill-shortcut-lost")
          .boundingBox())!;
        const close = (await page
          .getByTestId("wb-pill-dismiss")
          .boundingBox())!;
        return msg.x > close.x;
      })
      .toBe(true);
  });

  test("light and dark: no serious or critical axe findings", async ({
    page,
    ipc,
  }) => {
    test.setTimeout(120_000);
    for (const theme of ["light", "dark"] as const) {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      for (const s of [
        LOST_PILL,
        secureState(false, "Terminal"),
        secureState(true, null),
      ]) {
        await openPill(page, ipc, s);
        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          .disableRules(["region", "landmark-one-main", "page-has-heading-one"])
          .analyze();
        const serious = results.violations.filter(
          (v) => v.impact === "serious" || v.impact === "critical",
        );
        expect(serious.map((v) => `${theme} ${s.shape}: ${v.id}`)).toEqual([]);
      }
    }
  });
});
