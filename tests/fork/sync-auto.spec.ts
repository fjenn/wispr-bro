// SPDX-License-Identifier: GPL-3.0-or-later
// Automatic sync (`src/fork/sync/useAutoSync`, `PairingDialog`, the iPhone's
// Sync page and the Mac's Settings › Data and Privacy › Sync): pairing with
// the Mac's code, the status line, Sync now, Unpair, and the Mac's side.
//
// Screenshots (light and dark) are written when WB_SYNC_SHOTS_DIR is set:
//
//   WB_SYNC_SHOTS_DIR=/tmp/sync-shots bun run wb:test:ui -- sync-auto
import path from "node:path";
import type { Page } from "@playwright/test";
import type { AutoStatus, PairedDevice } from "../../src/fork/bindings";
import type { IpcMock } from "./fixtures";
import { test as desktop } from "./fixtures";
import { handySettings } from "./hub-fixture";
import { test, expect, openIos } from "./ios-fixture";
import { openSettingsModal } from "./settings-modal-fixture";

const dir = process.env.WB_SYNC_SHOTS_DIR;
const SERVICE = "ab12cd34 Finns MacBook Pro";

function phone(over: Partial<AutoStatus> = {}): AutoStatus {
  return {
    role: "phone",
    device_name: "iPhone",
    enabled: false,
    devices: [],
    found: [],
    searching: false,
    pairing: null,
    network_denied: false,
    ...over,
  };
}

function mac(over: Partial<AutoStatus> = {}): AutoStatus {
  return { ...phone(), role: "mac", device_name: "Finns MacBook Pro", ...over };
}

function paired(over: Partial<PairedDevice> = {}): PairedDevice {
  return {
    device_id: "ab12cd34ef567890ab12cd34ef567890",
    name: "Finns MacBook Pro",
    platform: "macos",
    paired_at: Date.now() - 86_400_000,
    last_sync_at: Date.now() - 2 * 60_000,
    connected: true,
    found: true,
    needs_update: false,
    ...over,
  };
}

/** A backend whose status a test changes; every change is announced. */
function installAuto(ipc: IpcMock, start: AutoStatus) {
  const state = { auto: start };
  ipc.handle("fork_sync_auto_status", () => state.auto);
  for (const command of [
    "fork_sync_auto_enable",
    "fork_sync_search",
    "fork_sync_pair",
    "fork_sync_pair_code",
    "fork_sync_unpair",
    "fork_sync_now",
  ]) {
    ipc.handle(command, () => null);
  }
  ipc.handle("fork_sync_status", () => ({
    device_name: start.device_name,
    platform: start.role === "mac" ? "macos" : "ios",
    devices: [],
    last_export_at: null,
    pending_file: null,
  }));
  return {
    state,
    set: async (next: AutoStatus) => {
      state.auto = next;
      await ipc.emit("fork-sync-auto-changed", {});
    },
  };
}

async function snap(page: Page, name: string, theme: string) {
  if (!dir) return;
  await page.waitForTimeout(600);
  await page.screenshot({ path: path.join(dir, `${name}-${theme}.png`) });
}

test("the iPhone finds its Mac, takes the code and shows it in sync", async ({
  page,
  ipc,
  hub,
  ios,
}) => {
  let backend: ReturnType<typeof installAuto> | undefined;
  await openIos(page, ipc, hub, ios, {
    hash: "#/hub/settings/sync",
    beforeNavigate: () => {
      backend = installAuto(ipc, phone());
    },
  });
  const auto = backend!;
  const pair = page.getByTestId("wb-ios-sync-pair");
  await expect(pair).toContainText("turn on Sync with your iPhone");
  await page.getByTestId("wb-ios-sync-find").click();
  await ipc.expectCalled("fork_sync_search", { on: true });
  await auto.set(phone({ searching: true }));
  await expect(page.getByTestId("wb-ios-sync-looking")).toBeVisible();
  await auto.set(
    phone({
      searching: true,
      found: [{ service: SERVICE, name: "Finns MacBook Pro", paired: false }],
    }),
  );
  await page.getByTestId("wb-ios-sync-found-mac").click();
  await ipc.expectCalled("fork_sync_pair", { service: SERVICE });

  await auto.set(
    phone({
      searching: true,
      pairing: { stage: "enter_code", device_name: "Finns MacBook Pro" },
    }),
  );
  const dialog = page.getByTestId("wb-pair-dialog");
  await expect(dialog).toContainText("Finns MacBook Pro is showing a 6-digit");
  await expect(page.getByTestId("wb-pair-submit")).toBeDisabled();
  await page.getByTestId("wb-pair-input").fill("48a29 13");
  await expect(page.getByTestId("wb-pair-input")).toHaveValue("482913");
  await page.getByTestId("wb-pair-submit").click();
  await ipc.expectCalled("fork_sync_pair_code", { code: "482913" });

  // A wrong code says so; the right one pairs.
  await auto.set(
    phone({
      pairing: {
        stage: "failed",
        device_name: "Finns MacBook Pro",
        problem: "wrong_code",
      },
    }),
  );
  await expect(page.getByTestId("wb-pair-problem")).toContainText(
    "The code did not match the one on your Mac.",
  );
  await page.getByTestId("wb-pair-close").click();
  await ipc.expectCalled("fork_sync_pair_code", { code: null });
  await auto.set(phone({ enabled: true, devices: [paired()] }));
  await expect(dialog).toBeHidden();
  const state = page.getByTestId("wb-ios-sync-state");
  await expect(state).toContainText("In sync · last change 2 minutes ago");
  await expect(page.getByTestId("wb-ios-sync-mac")).toContainText(
    "syncs while Wispr Bro is open or a dictation session is on",
  );
  await expect(page.getByTestId("wb-ios-sync-pair")).toHaveCount(0);
});

test("a paired iPhone says when the Mac is away; Sync now and Unpair", async ({
  page,
  ipc,
  hub,
  ios,
}) => {
  await openIos(page, ipc, hub, ios, {
    hash: "#/hub/settings/sync",
    beforeNavigate: () => {
      installAuto(
        ipc,
        phone({
          enabled: true,
          devices: [
            paired({
              connected: false,
              found: false,
              last_sync_at: Date.now() - 3 * 3_600_000,
            }),
          ],
        }),
      );
    },
  });
  await expect(page.getByTestId("wb-ios-sync-state")).toHaveText(
    "Mac not found. Open Wispr Bro on your Mac. Last synced 3 hours ago.",
  );
  await page.getByTestId("wb-ios-sync-now").click();
  await ipc.expectCalled("fork_sync_now");
  await page.getByTestId("wb-ios-sync-unpair").click();
  await expect(page.getByTestId("wb-unpair-dialog")).toContainText(
    "Nothing is deleted on either device.",
  );
  await page.getByTestId("wb-unpair-confirm").click();
  await ipc.expectCalled("fork_sync_unpair", {
    deviceId: "ab12cd34ef567890ab12cd34ef567890",
  });
});

test("local network access refused: the page says how to allow it", async ({
  page,
  ipc,
  hub,
  ios,
}) => {
  await openIos(page, ipc, hub, ios, {
    hash: "#/hub/settings/sync",
    beforeNavigate: () => {
      installAuto(ipc, phone({ searching: true, network_denied: true }));
    },
  });
  await expect(page.getByTestId("wb-ios-sync-denied")).toContainText(
    "not allowed to find devices on your local network",
  );
  await expect(page.getByTestId("wb-ios-sync-found")).toHaveCount(0);
});

test("a failed pairing says which side and what to do", async ({
  page,
  ipc,
  hub,
  ios,
}) => {
  let backend: ReturnType<typeof installAuto> | undefined;
  await openIos(page, ipc, hub, ios, {
    hash: "#/hub/settings/sync",
    beforeNavigate: () => {
      backend = installAuto(ipc, phone());
    },
  });
  const failed = (problem: string) =>
    backend!.set(
      phone({
        pairing: { stage: "failed", device_name: "Finns MacBook Pro", problem },
      }),
    );
  const problem = page.getByTestId("wb-pair-problem");
  for (const [code, text] of [
    ["denied", "Settings › Privacy & Security › Local Network"],
    ["update_mac", "Update Wispr Bro on your Mac"],
    ["update_this", "Update Wispr Bro on this iPhone"],
    ["no_code", "Timed out"],
    ["unreachable", "could not be reached"],
    ["something_new", "Pairing did not finish"],
  ]) {
    await failed(code);
    await expect(problem).toContainText(text);
  }
});

desktop(
  "the Mac says why a pairing failed, in its own words",
  async ({ page, ipc }) => {
    const auto = installAuto(ipc, mac({ enabled: true }));
    await openSettingsModal(page, ipc, "privacy");
    const problem = page.getByTestId("wb-pair-problem");
    for (const [code, text] of [
      ["wrong_code", "Your iPhone typed a different code"],
      ["update_phone", "Update Wispr Bro on your iPhone"],
      ["update_this", "Update Wispr Bro on this Mac"],
    ]) {
      await auto.set(
        mac({
          enabled: true,
          pairing: { stage: "failed", device_name: "iPhone", problem: code },
        }),
      );
      await expect(problem).toContainText(text);
    }
  },
);

desktop(
  "the Mac turns sync on, shows the code and lists its iPhone",
  async ({ page, ipc }) => {
    const auto = installAuto(ipc, mac());
    await openSettingsModal(page, ipc, "privacy");
    const toggle = page.getByTestId("wb-privacy-sync-auto");
    await expect(toggle).not.toBeChecked();
    await toggle.click();
    await ipc.expectCalled("fork_sync_auto_enable", { on: true });
    await auto.set(mac({ enabled: true }));
    await expect(page.getByTestId("wb-privacy-sync-howto")).toContainText(
      "Find my Mac",
    );

    // A phone asks to pair: the code is up, inside the Settings modal.
    await auto.set(
      mac({
        enabled: true,
        pairing: { stage: "show_code", device_name: "iPhone", code: "482913" },
      }),
    );
    const code = page.getByTestId("wb-pair-code");
    await expect(code).toBeVisible();
    await expect(code).toHaveAttribute("aria-label", "4 8 2 9 1 3");
    await expect(page.getByTestId("wb-pair-dialog")).toContainText(
      "Type this code on your iPhone.",
    );

    await auto.set(
      mac({
        enabled: true,
        devices: [
          paired({
            device_id: "ff00ff00ff00ff00",
            name: "iPhone",
            platform: "ios",
            last_sync_at: Date.now() - 30_000,
          }),
        ],
      }),
    );
    await expect(page.getByTestId("wb-pair-dialog")).toBeHidden();
    const device = page.getByTestId("wb-privacy-sync-device");
    await expect(device).toContainText("iPhone");
    await expect(device).toContainText("In sync · last change just now");
    await page.getByTestId("wb-privacy-sync-unpair").click();
    await page.getByTestId("wb-unpair-confirm").click();
    await ipc.expectCalled("fork_sync_unpair", {
      deviceId: "ff00ff00ff00ff00",
    });
    // The privacy text names the third use of the network.
    await page.getByTestId("wb-privacy-network").getByText("What uses").click();
    await expect(page.getByTestId("wb-privacy-network")).toContainText(
      "talk directly to each other over your local network. No server.",
    );
  },
);

// ── Screenshots ─────────────────────────────────────────────────────────

test.describe("auto sync shots", () => {
  test.skip(!dir, "set WB_SYNC_SHOTS_DIR to write screenshots");
  test.setTimeout(120_000);

  for (const theme of ["light", "dark"] as const) {
    test(`iphone ${theme}`, async ({ page, ipc, hub, ios }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      let backend: ReturnType<typeof installAuto> | undefined;
      await openIos(page, ipc, hub, ios, {
        hash: "#/hub/settings/sync",
        handy: { theme },
        beforeNavigate: () => {
          backend = installAuto(ipc, phone());
        },
      });
      const auto = backend!;
      await expect(page.getByTestId("wb-ios-sync-pair")).toBeVisible();
      await snap(page, "iphone-unpaired", theme);
      await auto.set(
        phone({
          searching: true,
          found: [
            { service: SERVICE, name: "Finns MacBook Pro", paired: false },
          ],
        }),
      );
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 9999));
      await snap(page, "iphone-searching", theme);
      await auto.set(
        phone({
          searching: true,
          pairing: { stage: "enter_code", device_name: "Finns MacBook Pro" },
        }),
      );
      await page.getByTestId("wb-pair-input").fill("482913");
      await snap(page, "iphone-enter-code", theme);
      await auto.set(
        phone({
          pairing: {
            stage: "failed",
            device_name: "Finns MacBook Pro",
            problem: "wrong_code",
          },
        }),
      );
      await snap(page, "iphone-wrong-code", theme);
      await auto.set(phone({ enabled: true, devices: [paired()] }));
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 0));
      await expect(page.getByTestId("wb-ios-sync-mac")).toBeVisible();
      await snap(page, "iphone-in-sync", theme);
      await auto.set(
        phone({
          enabled: true,
          devices: [
            paired({
              connected: false,
              found: false,
              last_sync_at: Date.now() - 3 * 3_600_000,
            }),
          ],
        }),
      );
      await snap(page, "iphone-mac-away", theme);
      await page.getByTestId("wb-ios-sync-file").click();
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 420));
      await snap(page, "iphone-file-route", theme);
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 0));
      await page.getByTestId("wb-ios-sync-unpair").click();
      await snap(page, "iphone-unpair", theme);
    });
  }
});

desktop.describe("auto sync shots (mac)", () => {
  desktop.skip(!dir, "set WB_SYNC_SHOTS_DIR to write screenshots");

  for (const theme of ["light", "dark"] as const) {
    desktop(`mac ${theme}`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      const auto = installAuto(ipc, mac({ enabled: true }));
      await openSettingsModal(page, ipc, "privacy", {
        get_app_settings: handySettings({
          theme,
          post_process_providers: [],
          post_process_provider_id: "local",
          post_process_api_keys: {},
          post_process_models: {},
          post_process_prompts: [],
        }),
      });
      await page.getByTestId("wb-privacy-sync").scrollIntoViewIfNeeded();
      await snap(page, "mac-on-unpaired", theme);
      await auto.set(
        mac({
          enabled: true,
          pairing: {
            stage: "show_code",
            device_name: "iPhone",
            code: "482913",
          },
        }),
      );
      await expect(page.getByTestId("wb-pair-code")).toBeVisible();
      await snap(page, "mac-code", theme);
      await auto.set(
        mac({
          enabled: true,
          devices: [
            paired({
              device_id: "ff00ff00ff00ff00",
              name: "iPhone",
              platform: "ios",
            }),
          ],
        }),
      );
      await expect(page.getByTestId("wb-privacy-sync-device")).toBeVisible();
      await snap(page, "mac-paired", theme);
    });
  }
});
