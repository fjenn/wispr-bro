// SPDX-License-Identifier: GPL-3.0-or-later
// S-28: the Hub footer's update status, the About page's update panel and
// What's New (once per version, never while dictating). Every state, light
// and dark with reduced motion, RTL, keyboard-only and axe.
import type { Page } from "@playwright/test";
import { ipcError } from "./fixtures";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
} from "./hub-fixture";
import type { IpcMock } from "./fixtures";
import { openSettingsModal } from "./settings-modal-fixture";

type Phase =
  | "off"
  | "idle"
  | "checking"
  | "up_to_date"
  | "check_failed"
  | "downloading"
  | "ready"
  | "installing"
  | "installed"
  | "install_failed";

function updateState(phase: Phase, over: Record<string, unknown> = {}) {
  return {
    current_version: "1.0.0-alpha.1",
    channel: "stable",
    phase,
    version: ["off", "idle", "checking", "up_to_date", "check_failed"].includes(
      phase,
    )
      ? null
      : "1.0.0-alpha.2",
    last_checked_ms: phase === "idle" ? null : Date.UTC(2026, 10, 2, 9, 30),
    paused_until_ms: null,
    failures: 0,
    busy: false,
    automatic: true,
    needs_password: false,
    ...over,
  };
}

async function hubWith(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  state: ReturnType<typeof updateState>,
  handy: Record<string, unknown> = {},
) {
  ipc.respond("fork_update_state", state);
  await openHub(page, ipc, hub, { handy });
}

const footer = (page: Page) => page.getByTestId("wb-footer");
const status = (page: Page) => page.getByTestId("wb-update-status");

test.describe("Hub footer: update status", () => {
  const cases: [Phase, string, Record<string, unknown>?][] = [
    ["off", "Update checks off"],
    ["checking", "Checking for updates…"],
    ["up_to_date", "Up to date"],
    ["check_failed", "Couldn't check"],
    ["downloading", "Downloading 1.0.0-alpha.2…"],
    ["ready", "Update ready"],
    ["installing", "Installing 1.0.0-alpha.2…"],
    ["installed", "Restarting…"],
    [
      "installed",
      "1.0.0-alpha.2 installed · restarts after this dictation",
      { busy: true },
    ],
    ["install_failed", "Update didn't install"],
  ];
  for (const [phase, text, over] of cases) {
    test(`${phase}${over ? " (busy)" : ""}: "${text}" next to the version`, async ({
      page,
      ipc,
      hub,
    }) => {
      await hubWith(page, ipc, hub, updateState(phase, over));
      await expect(status(page).getByTestId("wb-update-text")).toHaveText(text);
      await expect(page.getByTestId("wb-footer-version")).toHaveText(
        "v1.0.0-alpha.1",
      );
      // Handy's update checker is gone (it called the plugin directly).
      await expect(footer(page)).not.toContainText("Check for updates?");
    });
  }

  test("never checked: a Check for updates link that checks now", async ({
    page,
    ipc,
    hub,
  }) => {
    await hubWith(page, ipc, hub, updateState("idle"));
    ipc.respond("fork_update_check_now", updateState("checking"));
    await status(page)
      .getByRole("button", { name: "Check for updates" })
      .click();
    await ipc.expectCalled("fork_update_check_now");
    await expect(status(page).getByTestId("wb-update-text")).toHaveText(
      "Checking for updates…",
    );
  });

  test("ready: Restart now restarts, and says so when a take runs", async ({
    page,
    ipc,
    hub,
  }) => {
    await hubWith(page, ipc, hub, updateState("ready"));
    ipc.respondSequence("fork_update_restart_now", [
      ipcError({ kind: "Internal", message: "a dictation is in progress" }),
      null,
    ]);
    const restart = status(page).getByRole("button", { name: "Restart now" });
    await restart.click();
    await expect(page.getByTestId("wb-update-problem")).toHaveText(
      "Finish dictating first.",
    );
    await restart.click();
    await expect(page.getByTestId("wb-update-problem")).toHaveCount(0);
    expect(await ipc.calls("fork_update_restart_now")).toHaveLength(2);
  });

  test("couldn't check: Retry, then live progress from the backend", async ({
    page,
    ipc,
    hub,
  }) => {
    await hubWith(page, ipc, hub, updateState("check_failed"));
    ipc.respond("fork_update_check_now", updateState("checking"));
    await status(page).getByRole("button", { name: "Retry" }).click();
    await ipc.waitForListener("fork-update-state");
    await ipc.emit("fork-update-state", updateState("downloading"));
    await expect(status(page).getByTestId("wb-update-text")).toHaveText(
      "Downloading 1.0.0-alpha.2…",
    );
    await ipc.emit("fork-update-state", updateState("ready"));
    await expect(
      status(page).getByRole("button", { name: "Restart now" }),
    ).toBeVisible();
  });

  test("a failed manual check is reported", async ({ page, ipc, hub }) => {
    await hubWith(page, ipc, hub, updateState("install_failed"));
    ipc.respond(
      "fork_update_check_now",
      ipcError({ kind: "Internal", message: "update checks are off" }),
    );
    await status(page).getByRole("button", { name: "Retry" }).click();
    await expect(page.getByTestId("wb-update-problem")).toHaveText(
      "That didn't work. Try again later.",
    );
  });

  test("checks off: Turn on opens Data and Privacy", async ({
    page,
    ipc,
    hub,
  }) => {
    await hubWith(page, ipc, hub, updateState("off"));
    await status(page).getByRole("button", { name: "Turn on" }).click();
    await expect(page.getByTestId("wb-settings-privacy")).toBeVisible();
  });

  test("keyboard only: the footer action is reachable and works", async ({
    page,
    ipc,
    hub,
  }) => {
    await hubWith(page, ipc, hub, updateState("ready"));
    ipc.respond("fork_update_restart_now", null);
    const restart = status(page).getByRole("button", { name: "Restart now" });
    for (let i = 0; i < 60; i += 1) {
      if (await restart.evaluate((el) => el === document.activeElement)) break;
      await page.keyboard.press("Tab");
    }
    await expect(restart).toBeFocused();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_update_restart_now");
  });

  test("RTL: the status stays in the footer and keeps its order", async ({
    page,
    ipc,
    hub,
  }) => {
    await hubWith(page, ipc, hub, updateState("ready"), { app_language: "ar" });
    await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
    const box = await status(page).boundingBox();
    const bar = await footer(page).boundingBox();
    expect(box && bar && box.y >= bar.y && box.y < bar.y + bar.height).toBe(
      true,
    );
    // In RTL the version/status group sits at the inline end (left).
    expect(box && bar && box.x - bar.x < bar.width / 2).toBe(true);
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe and reduced motion: footer states (${theme})`, async ({
      page,
      ipc,
      hub,
    }) => {
      test.setTimeout(180_000);
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await hubWith(page, ipc, hub, updateState("ready"), { theme });
      await expect(
        status(page).getByRole("button", { name: "Restart now" }),
      ).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await ipc.emit("fork-update-state", updateState("check_failed"));
      await expect(
        status(page).getByRole("button", { name: "Retry" }),
      ).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }
});

test.describe("About: updates", () => {
  test("shows status, last check, policy and Check again", async ({
    page,
    ipc,
  }) => {
    await openSettingsModal(page, ipc, "about", {
      fork_update_state: updateState("up_to_date"),
      fork_update_check_now: updateState("checking"),
    });
    const panel = page.getByTestId("wb-update-panel");
    await expect(panel).toContainText("Up to date");
    await expect(panel).toContainText("Last checked");
    await expect(panel).toContainText(
      "Updates download in the background and install only while you're not dictating, or when you quit.",
    );
    await panel.getByRole("button", { name: "Check again" }).click();
    await ipc.expectCalled("fork_update_check_now");
  });

  test("ready: the hint explains when it installs", async ({ page, ipc }) => {
    await openSettingsModal(page, ipc, "about", {
      fork_update_state: updateState("ready"),
    });
    const panel = page.getByTestId("wb-update-panel");
    await expect(panel).toContainText(
      "It installs by itself once you haven't dictated for 20 minutes, or when you quit.",
    );
    await expect(
      panel.getByRole("button", { name: "Restart now" }),
    ).toBeVisible();
  });

  test("paused after three failures and development builds say so", async ({
    page,
    ipc,
  }) => {
    await openSettingsModal(page, ipc, "about", {
      fork_update_state: updateState("install_failed", {
        failures: 3,
        paused_until_ms: Date.UTC(2026, 10, 3, 9, 30),
        automatic: false,
        needs_password: true,
      }),
    });
    const panel = page.getByTestId("wb-update-panel");
    await expect(panel).toContainText("Update didn't install");
    await expect(panel).toContainText(
      "After three failed attempts, automatic updates are paused until",
    );
    await expect(panel).toContainText("updates are checked only when you ask");
    await expect(panel).toContainText("needs an administrator password");
    await expect(panel.getByRole("button", { name: "Retry" })).toBeVisible();
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe: About with the update panel (${theme})`, async ({
      page,
      ipc,
    }) => {
      test.setTimeout(180_000);
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openSettingsModal(page, ipc, "about", {
        fork_update_state: updateState("ready"),
      });
      await expect(page.getByTestId("wb-update-panel")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }
});

test.describe("What's New", () => {
  const whatsNew = (page: Page) => page.getByTestId("wb-whats-new");
  const seen = { show_whats_new_on_update: true };

  test("after an update: our note shows once and is marked seen", async ({
    page,
    ipc,
    hub,
  }) => {
    ipc.handle("change_whats_new_last_seen_version_setting", (args) => {
      hub.handy.whats_new_last_seen_version = args.version;
      return null;
    });
    await hubWith(page, ipc, hub, updateState("up_to_date"), {
      ...seen,
      whats_new_last_seen_version: "0.9.7",
    });
    await expect(whatsNew(page)).toBeVisible();
    await expect(page.getByRole("dialog")).toContainText(
      "What's new in 1.0.0-alpha.1",
    );
    await expect(whatsNew(page)).toContainText("Updates that never interrupt");
    // Never Handy's notes.
    await expect(whatsNew(page)).not.toContainText("Handy");
    await page.getByTestId("wb-whats-new-close").click();
    await expect(whatsNew(page)).toHaveCount(0);
    await ipc.expectCalled("change_whats_new_last_seen_version_setting", {
      version: "1.0.0-alpha.1",
    });
    await page.reload();
    await expect(page.getByTestId("wb-sidebar")).toBeVisible();
    await expect(whatsNew(page)).toHaveCount(0);
  });

  test("never while dictating: it waits for the take to end", async ({
    page,
    ipc,
    hub,
  }) => {
    await hubWith(page, ipc, hub, updateState("up_to_date", { busy: true }), {
      ...seen,
      whats_new_last_seen_version: "0.9.7",
    });
    await ipc.waitForListener("fork-update-state");
    await expect(whatsNew(page)).toHaveCount(0);
    await ipc.emit("fork-update-state", updateState("up_to_date"));
    await expect(whatsNew(page)).toBeVisible();
    // The dialog takes focus a frame after its key handler is attached; an
    // Esc before that went nowhere under the full parallel suite.
    await expect(page.getByRole("dialog")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(whatsNew(page)).toHaveCount(0);
  });

  test("a fresh install or the setting off shows nothing", async ({
    page,
    ipc,
    hub,
  }) => {
    await hubWith(page, ipc, hub, updateState("up_to_date"), {
      ...seen,
      whats_new_last_seen_version: "1.0.0-alpha.1",
    });
    await ipc.waitForListener("fork-update-state");
    await expect(whatsNew(page)).toHaveCount(0);
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe and RTL: the What's New dialog (${theme})`, async ({
      page,
      ipc,
      hub,
    }) => {
      test.setTimeout(180_000);
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await hubWith(page, ipc, hub, updateState("up_to_date"), {
        ...seen,
        theme,
        whats_new_last_seen_version: "",
        app_language: theme === "dark" ? "ar" : "en",
      });
      await expect(whatsNew(page)).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }
});
