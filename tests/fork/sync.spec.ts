// SPDX-License-Identifier: GPL-3.0-or-later
// Sync between the Mac and the iPhone (`src/fork/sync`, `src/fork/ios/IosSync`):
// the iPhone's page, the shared review and result, a file handed over by
// AirDrop, and the Mac's entry in Settings › Data and Privacy.
//
// Screenshots (light and dark) are written when WB_SYNC_SHOTS_DIR is set:
//
//   WB_SYNC_SHOTS_DIR=/tmp/sync-shots bun run wb:test:ui -- sync.spec
import path from "node:path";
import type { Page } from "@playwright/test";
import type {
  MergeReport,
  SyncReview,
  SyncStatus,
} from "../../src/fork/bindings";
import type { IpcMock } from "./fixtures";
import { test as desktop } from "./fixtures";
import { handySettings } from "./hub-fixture";
import { test, expect, openIos } from "./ios-fixture";
import { openSettingsModal } from "./settings-modal-fixture";

const dir = process.env.WB_SYNC_SHOTS_DIR;
const MADE = Date.UTC(2026, 10, 17, 13, 2);
const PICKED =
  "file:///private/var/mobile/tmp/com.wisprbro.ios-Inbox/wispr-bro-data-2026-11-17.zip";

const none = { added: 0, changed: 0, removed: 0, skipped: 0 };

function report(over: Partial<MergeReport> = {}): MergeReport {
  return {
    dictionary: { added: 12, changed: 2, removed: 1, skipped: 0 },
    snippets: { added: 3, changed: 0, removed: 0, skipped: 0 },
    settings: ["spoken_languages", "typing_wpm", "styles"],
    stats_days: 41,
    stats_words: 12_840,
    stats_changed: true,
    ...over,
  };
}

function ready(over: Partial<Extract<SyncReview, { status: "ready" }>> = {}) {
  return {
    status: "ready",
    source: "mac_export",
    device_name: "",
    platform: "macos",
    app_version: "1.0.0-alpha.6",
    created_at: MADE,
    has_settings: true,
    has_dictionary: true,
    has_snippets: true,
    has_stats: true,
    report: report(),
    ...over,
  } as SyncReview;
}

function status(over: Partial<SyncStatus> = {}): SyncStatus {
  return {
    device_name: "iPhone",
    platform: "ios",
    devices: [],
    last_export_at: null,
    pending_file: null,
    ...over,
  };
}

const MAC_DEVICE = {
  device_id: "mac-export",
  name: "",
  platform: "macos",
  app_version: "1.0.0-alpha.6",
  last_import_at: MADE + 60_000,
  words: 12_840,
};

/** A backend whose state a merge changes, as the real one's does. */
function installSync(ipc: IpcMock, review: SyncReview, start = status()) {
  const state = { status: start };
  ipc.handle("fork_sync_status", () => state.status);
  ipc.handle("fork_sync_review", () => review);
  ipc.handle("fork_sync_apply", () => {
    state.status = {
      ...state.status,
      pending_file: null,
      devices: [MAC_DEVICE],
    };
    return review;
  });
  ipc.handle("fork_sync_discard", () => {
    state.status = { ...state.status, pending_file: null };
    return null;
  });
  ipc.handle("fork_sync_export", (args) => {
    state.status = { ...state.status, last_export_at: MADE + 120_000 };
    return {
      paths: [
        (args.path as string | null) ?? "/tmp/Wispr Bro (iPhone).wisprbro",
      ],
      words: 31,
      snippets: 4,
    };
  });
  return state;
}

async function snap(page: Page, name: string, theme: string) {
  if (!dir) return;
  await page.waitForTimeout(600);
  await page.screenshot({ path: path.join(dir, `${name}-${theme}.png`) });
}

test("the iPhone merges a file from the Mac after a review", async ({
  page,
  ipc,
  hub,
  ios,
}) => {
  await openIos(page, ipc, hub, ios, {
    hash: "#/hub/settings",
    beforeNavigate: () => installSync(ipc, ready()),
  });
  await page.getByTestId("wb-ios-settings-sync").click();
  const sync = page.getByTestId("wb-ios-sync");
  await expect(sync).toContainText("There is no account and no server");
  await expect(sync).toContainText("Export my data");
  await expect(page.getByTestId("wb-ios-sync-devices")).toContainText(
    "Not synced yet",
  );

  ipc.respond("plugin:dialog|open", PICKED);
  await page.getByTestId("wb-ios-sync-import").click();
  const review = page.getByTestId("wb-sync-review");
  await expect(review).toContainText("Merge from your Mac");
  await expect(review).toContainText("with Wispr Bro 1.0.0-alpha.6");
  await expect(page.getByTestId("wb-sync-part-dictionary")).toContainText(
    "12 new, 2 changed, 1 removed",
  );
  await expect(page.getByTestId("wb-sync-part-snippets")).toContainText(
    "3 new",
  );
  await expect(page.getByTestId("wb-sync-part-settings")).toContainText(
    "Languages, Typing speed, Style",
  );
  await expect(page.getByTestId("wb-sync-part-stats")).toContainText(
    "12,840 words on 41 days",
  );
  await expect(review).toContainText("Its history and recordings are not");
  await ipc.expectCalled("fork_sync_review", {
    path: PICKED,
    choices: { settings: true, dictionary: true, snippets: true, stats: true },
  });

  // Leave the settings as they are on this phone.
  await page.getByTestId("wb-sync-switch-settings").click();
  await page.getByTestId("wb-sync-merge").click();
  await ipc.expectCalled("fork_sync_apply", {
    path: PICKED,
    choices: { settings: false, dictionary: true, snippets: true, stats: true },
  });
  await expect(page.getByTestId("wb-sync-result")).toContainText(
    "Both are in step now.",
  );
  await page.getByTestId("wb-sync-done").click();
  await expect(review).toBeHidden();
  await expect(page.getByTestId("wb-ios-sync-device")).toContainText(
    "Your Mac",
  );
  await expect(page.getByTestId("wb-ios-sync-device")).toContainText(
    "12,840 words",
  );
});

test("a file opened from AirDrop lands on its review; Cancel forgets it", async ({
  page,
  ipc,
  hub,
  ios,
}) => {
  const file =
    "/var/mobile/Documents/Inbox/Wispr Bro (Finns MacBook Pro).wisprbro";
  let backend: ReturnType<typeof installSync> | undefined;
  await openIos(page, ipc, hub, ios, {
    beforeNavigate: () => {
      backend = installSync(
        ipc,
        ready({ source: "bundle", device_name: "Finn's MacBook Pro" }),
        status({ pending_file: file }),
      );
    },
  });
  // Cold start: the app opens the Sync page with the review up.
  const review = page.getByTestId("wb-sync-review");
  await expect(review).toContainText("Merge from Finn's MacBook Pro");
  await expect(page.getByTestId("wb-ios-sync")).toBeVisible();
  await expect(review).toContainText("History and recordings are not synced");
  await page.getByTestId("wb-sync-cancel").click();
  await ipc.expectCalled("fork_sync_discard", { path: file });
  await expect(review).toBeHidden();

  // While the app runs: the event opens it again, from any tab.
  await page.getByTestId("wb-ios-tab-home").click();
  // As the backend does: remember the file, then announce it.
  backend!.status = { ...backend!.status, pending_file: file };
  await ipc.emit("fork-sync-file-opened", { path: file });
  await expect(review).toContainText("Merge from Finn's MacBook Pro");
});

test("a file with nothing new and a file that is not ours say so", async ({
  page,
  ipc,
  hub,
  ios,
}) => {
  const same = ready({
    report: report({
      dictionary: none,
      snippets: none,
      settings: [],
      stats_changed: false,
    }),
  });
  await openIos(page, ipc, hub, ios, {
    hash: "#/hub/settings/sync",
    beforeNavigate: () => installSync(ipc, same),
  });
  ipc.respond("plugin:dialog|open", PICKED);
  await page.getByTestId("wb-ios-sync-import").click();
  await expect(page.getByTestId("wb-sync-up-to-date")).toBeVisible();
  await expect(page.getByTestId("wb-sync-merge")).toBeDisabled();
  await expect(page.getByTestId("wb-sync-switch-dictionary")).toHaveCount(0);
  await page.getByTestId("wb-sync-cancel").click();

  ipc.handle("fork_sync_review", () => ({
    status: "problem",
    problem: { problem: "not_wispr_bro" },
  }));
  await page.getByTestId("wb-ios-sync-import").click();
  await expect(page.getByTestId("wb-sync-problem")).toContainText(
    "This is not a Wispr Bro file.",
  );
  await page.getByTestId("wb-sync-done").click();
  await expect(page.getByTestId("wb-sync-review")).toBeHidden();
});

test("Send to Mac makes the file, with CSVs when asked", async ({
  page,
  ipc,
  hub,
  ios,
}) => {
  await openIos(page, ipc, hub, ios, {
    hash: "#/hub/settings/sync",
    beforeNavigate: () => installSync(ipc, ready()),
  });
  await page.getByTestId("wb-ios-sync-send").click();
  await ipc.expectCalled("fork_sync_export", { path: null, withCsv: false });
  await page.getByTestId("wb-ios-sync-csv").click();
  await page.getByTestId("wb-ios-sync-send").click();
  await ipc.expectCalled("fork_sync_export", { path: null, withCsv: true });
});

test("Home says when its numbers include another device", async ({
  page,
  ipc,
  hub,
  ios,
}) => {
  Object.assign(hub.stats, { total_words: 12_965, other_devices: 1 });
  await openIos(page, ipc, hub, ios);
  await expect(page.getByTestId("wb-stats-devices")).toHaveText(
    "Includes 1 other device",
  );
});

desktop(
  "the Mac exports its file and merges the iPhone's",
  async ({ page, ipc }) => {
    const fromPhone = ready({
      source: "bundle",
      device_name: "iPhone",
      platform: "ios",
      app_version: "1.0.0-alpha.7",
      report: report({
        dictionary: { added: 4, changed: 0, removed: 0, skipped: 0 },
        snippets: none,
        settings: ["typing_wpm"],
        stats_days: 9,
        stats_words: 1_420,
      }),
    });
    installSync(
      ipc,
      fromPhone,
      status({ device_name: "Finn's MacBook Pro", platform: "macos" }),
    );
    await openSettingsModal(page, ipc, "privacy");
    const row = page.getByTestId("wb-privacy-sync");
    await expect(row).toContainText("Sync with your iPhone");
    await expect(page.getByTestId("wb-privacy-sync-last")).toContainText(
      "Not synced yet",
    );

    ipc.respond("plugin:dialog|save", "/Users/test/Desktop/To iPhone.wisprbro");
    await page.getByTestId("wb-privacy-sync-export").click();
    await ipc.expectCalled("fork_sync_export", {
      path: "/Users/test/Desktop/To iPhone.wisprbro",
      withCsv: false,
    });
    const [dialog] = await ipc.calls("plugin:dialog|save");
    expect(JSON.stringify(dialog)).toContain(
      "Wispr Bro (Finns MacBook Pro).wisprbro",
    );
    await expect(page.getByText("AirDrop it to your iPhone.")).toBeVisible();

    ipc.respond(
      "plugin:dialog|open",
      "/Users/test/Downloads/Wispr Bro (iPhone).wisprbro",
    );
    await page.getByTestId("wb-privacy-sync-import").click();
    const review = page.getByTestId("wb-sync-review");
    await expect(review).toContainText("Merge from iPhone");
    await expect(page.getByTestId("wb-sync-part-snippets")).toContainText(
      "Up to date",
    );
    await page.getByTestId("wb-sync-merge").click();
    await ipc.expectCalled("fork_sync_apply");
    await expect(page.getByTestId("wb-sync-result")).toBeVisible();
    await page.getByTestId("wb-sync-done").click();
    await expect(review).toBeHidden();
    // The Settings modal is still up behind it.
    await expect(page.getByTestId("wb-privacy-sync")).toBeVisible();
  },
);

// ── Screenshots ─────────────────────────────────────────────────────────

test.describe("sync shots", () => {
  test.skip(!dir, "set WB_SYNC_SHOTS_DIR to write screenshots");
  test.setTimeout(120_000);

  for (const theme of ["light", "dark"] as const) {
    test(`iphone ${theme}`, async ({ page, ipc, hub, ios }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      Object.assign(hub.stats, {
        total_words: 12_965,
        total_dictations: 431,
        words_per_minute: 142.6,
        current_streak_days: 4,
        other_devices: 1,
      });
      await openIos(page, ipc, hub, ios, {
        hash: "#/hub/settings",
        handy: { theme, spoken_languages: ["en", "nl"] },
        beforeNavigate: () => installSync(ipc, ready()),
      });
      await snap(page, "iphone-settings", theme);
      await page.getByTestId("wb-ios-settings-sync").click();
      await expect(page.getByTestId("wb-ios-sync")).toBeVisible();
      await snap(page, "iphone-sync", theme);
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 9999));
      await snap(page, "iphone-sync-scrolled", theme);

      ipc.respond("plugin:dialog|open", PICKED);
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 0));
      await page.getByTestId("wb-ios-sync-import").click();
      await expect(page.getByTestId("wb-sync-review")).toBeVisible();
      await snap(page, "iphone-review", theme);
      await page.getByTestId("wb-sync-merge").click();
      await expect(page.getByTestId("wb-sync-result")).toBeVisible();
      await snap(page, "iphone-result", theme);
      await page.getByTestId("wb-sync-done").click();
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 9999));
      await snap(page, "iphone-synced", theme);

      ipc.handle("fork_sync_review", () => ({
        status: "problem",
        problem: { problem: "not_wispr_bro" },
      }));
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 0));
      await page.getByTestId("wb-ios-sync-import").click();
      await expect(page.getByTestId("wb-sync-problem")).toBeVisible();
      await snap(page, "iphone-problem", theme);
      await page.getByTestId("wb-sync-done").click();

      await page.getByTestId("wb-ios-tab-home").click();
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 330));
      await snap(page, "iphone-home-stats", theme);
    });
  }
});

desktop.describe("sync shots (mac)", () => {
  desktop.skip(!dir, "set WB_SYNC_SHOTS_DIR to write screenshots");

  for (const theme of ["light", "dark"] as const) {
    desktop(`mac ${theme}`, async ({ page, ipc }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      installSync(
        ipc,
        ready({
          source: "bundle",
          device_name: "iPhone",
          platform: "ios",
          app_version: "1.0.0-alpha.7",
          report: report({
            dictionary: { added: 4, changed: 1, removed: 0, skipped: 0 },
            snippets: none,
            settings: ["typing_wpm"],
            stats_days: 9,
            stats_words: 1_420,
          }),
        }),
        status({
          device_name: "Finn's MacBook Pro",
          platform: "macos",
          devices: [
            {
              ...MAC_DEVICE,
              device_id: "abc",
              name: "iPhone",
              platform: "ios",
              words: 1_300,
            },
          ],
        }),
      );
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
      await snap(page, "mac-settings", theme);
      ipc.respond(
        "plugin:dialog|open",
        "/Users/test/Downloads/Wispr Bro (iPhone).wisprbro",
      );
      await page.getByTestId("wb-privacy-sync-import").click();
      await expect(page.getByTestId("wb-sync-review")).toBeVisible();
      await snap(page, "mac-review", theme);
    });
  }
});
