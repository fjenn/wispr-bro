// SPDX-License-Identifier: GPL-3.0-or-later
// Wispr Bro for iPhone: the phone shell, Home's session and setup, Settings
// and onboarding, on the mocked backend (`ios-fixture.ts`).
import { test, expect, openIos, openIosOnboarding } from "./ios-fixture";

test.describe("ios", () => {
  test("shows the phone shell, not the Hub", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    await openIos(page, ipc, hub, ios);
    await expect(page.locator("html")).toHaveAttribute("data-wb-ios", "");
    await expect(page.getByTestId("wb-sidebar")).toHaveCount(0);
    await expect(page.getByTestId("wb-header")).toHaveCount(0);
    for (const tab of [
      "home",
      "history",
      "dictionary",
      "snippets",
      "settings",
    ]) {
      await expect(page.getByTestId(`wb-ios-tab-${tab}`)).toBeVisible();
    }
    // No horizontal overflow at phone width.
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - window.innerWidth,
    );
    expect(overflow).toBeLessThanOrEqual(0);
  });

  test("tabs switch pages; desktop-only pages open Home", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    await openIos(page, ipc, hub, ios, { hash: "#/hub/transforms" });
    await expect(page.getByTestId("wb-home")).toBeVisible();
    await page.getByTestId("wb-ios-tab-history").click();
    await expect(page.getByTestId("wb-ios-history")).toBeVisible();
    await page.getByTestId("wb-ios-tab-dictionary").click();
    await expect(page.getByTestId("wb-dictionary")).toBeVisible();
    await expect(page.getByTestId("wb-dictionary-import")).toBeHidden();
    await page.getByTestId("wb-ios-tab-snippets").click();
    await expect(page.getByTestId("wb-snippets")).toBeVisible();
    await page.getByTestId("wb-ios-tab-settings").click();
    await expect(page.getByTestId("wb-ios-settings")).toBeVisible();
    await expect(page.getByTestId("wb-settings-modal")).toHaveCount(0);
  });

  test("starts and ends a session", async ({ page, ipc, hub, ios }) => {
    await openIos(page, ipc, hub, ios);
    const card = page.getByTestId("wb-ios-session");
    await expect(card).toHaveAttribute("data-state", "noSession");
    await page.getByTestId("wb-ios-session-start").click();
    await ipc.expectCalled("ios_start_session");
    await expect(card).toHaveAttribute("data-state", "idle");
    await page.getByTestId("wb-ios-session-end").click();
    await ipc.expectCalled("ios_end_session");
    await expect(card).toHaveAttribute("data-state", "noSession");
  });

  test("setup blocks the session until every step is done", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.setup = { keyboardEnabled: false, fullAccess: false, micPermission: 0 };
    await openIos(page, ipc, hub, ios);
    await expect(page.getByTestId("wb-ios-setup")).toBeVisible();
    await expect(page.getByTestId("wb-ios-session")).toHaveCount(0);
    await page.getByTestId("wb-ios-setup-mic-allow").click();
    await ipc.expectCalled("ios_request_mic");
    await expect(page.getByTestId("wb-ios-setup-mic")).toHaveAttribute(
      "data-done",
      "true",
    );
    await page.getByTestId("wb-ios-setup-keyboard-show").click();
    await page.getByTestId("wb-ios-keyboard-open-settings").click();
    await ipc.expectCalled("ios_open_settings");
    // The user comes back from Settings with the keyboard added.
    ios.setup = { keyboardEnabled: true, fullAccess: true, micPermission: 2 };
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("wb-ios-session")).toBeVisible();
  });

  test("session settings are saved", async ({ page, ipc, hub, ios }) => {
    await openIos(page, ipc, hub, ios, { hash: "#/hub/settings" });
    await page.getByTestId("wb-ios-settings-auto-return").click();
    await ipc.expectCalled("ios_set_auto_return", { enabled: false });
    await page.getByTestId("wb-ios-settings-session").click();
    await page.getByTestId("wb-ios-timeout-900").click();
    await ipc.expectCalled("ios_set_session_timeout", { seconds: 900 });
    await page.getByTestId("wb-ios-back").click();
    await expect(page.getByTestId("wb-ios-settings-session")).toContainText(
      "15 minutes",
    );
  });

  test("cleanup is not offered on an iPhone without the memory", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.llmState = "unsupported";
    await openIos(page, ipc, hub, ios, { hash: "#/hub/settings/cleanup" });
    await expect(page.getByTestId("wb-ios-cleanup-none")).toBeVisible();
    await expect(page.getByTestId("fork-cleanup-level-light")).toBeDisabled();
    await expect(page.getByTestId("wb-ios-cleanup-note")).toHaveCount(0);
  });

  test("cleanup levels can be picked once the model is there", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    await openIos(page, ipc, hub, ios, { hash: "#/hub/settings/cleanup" });
    await expect(page.getByTestId("wb-ios-cleanup-note")).toBeVisible();
    await expect(page.getByTestId("wb-ios-cleanup-downloading")).toHaveCount(0);
    await page.getByTestId("fork-cleanup-level-light").click();
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "cleanup_level", value: "light" },
    });
  });

  test("the cleanup model's download shows its progress", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.llmState = "downloading";
    ios.llmDownloaded = 318_599_024;
    await openIos(page, ipc, hub, ios, {
      hash: "#/hub/settings/cleanup",
      fork: { cleanup_level: "light" },
    });
    const card = page.getByTestId("wb-ios-cleanup-downloading");
    await expect(card).toContainText("25 %");
    await expect(card.getByRole("progressbar")).toHaveAttribute(
      "aria-valuenow",
      "25",
    );
    // A level can be chosen while the model is on its way.
    await expect(page.getByTestId("fork-cleanup-level-light")).toBeEnabled();
    // The page follows the download without being reopened.
    ios.llmState = "ready";
    await expect(card).toHaveCount(0);
  });

  test("with cleanup off nothing is downloaded, and the page says so", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.llmState = "waiting_for_speech_model";
    await openIos(page, ipc, hub, ios, {
      hash: "#/hub/settings/cleanup",
      fork: { cleanup_level: "none" },
    });
    await expect(page.getByTestId("wb-ios-cleanup-downloading")).toHaveCount(0);
    await expect(page.getByTestId("wb-ios-cleanup-note")).toContainText(
      "1274 MB",
    );
    await expect(page.getByTestId("fork-cleanup-level-light")).toBeEnabled();
  });

  test("a failed cleanup download can be tried again", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.llmState = "download_failed";
    await openIos(page, ipc, hub, ios, { hash: "#/hub/settings/cleanup" });
    await page.getByTestId("wb-ios-cleanup-retry").click();
    await ipc.expectCalled("fork_llm_retry");
    await expect(page.getByTestId("wb-ios-cleanup-downloading")).toBeVisible();
  });

  test("onboarding runs from welcome to the app", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.setup = { keyboardEnabled: false, fullAccess: false, micPermission: 0 };
    await openIosOnboarding(page, ipc, hub, ios, "welcome");
    const flow = page.getByTestId("wb-onboarding");
    await page.getByTestId("wb-ios-onboarding-start").click();
    await expect(flow).toHaveAttribute("data-step", "languages");
    await page.getByTestId("wb-languages-continue").click();
    await expect(flow).toHaveAttribute("data-step", "microphone");
    await page.getByTestId("wb-ios-onboarding-mic-allow").click();
    await page.getByTestId("wb-ios-onboarding-continue").click();
    await expect(flow).toHaveAttribute("data-step", "keyboard");
    await page.getByTestId("wb-ios-onboarding-open-settings").click();
    await ipc.expectCalled("ios_open_settings");
    ios.setup = { keyboardEnabled: true, fullAccess: true, micPermission: 2 };
    await page.getByTestId("wb-ios-onboarding-continue").click();
    await expect(flow).toHaveAttribute("data-step", "model");
    await page.getByTestId("wb-ios-onboarding-continue").click();
    await expect(flow).toHaveAttribute("data-step", "practice");
    await ipc.expectCalled("ios_start_session");
    await page
      .getByTestId("wb-ios-onboarding-practice-field")
      .fill("Hello from the keyboard");
    await page.getByTestId("wb-ios-onboarding-continue").click();
    await page.getByTestId("wb-ios-onboarding-finish").click();
    await ipc.expectCalled("complete_onboarding");
    await expect(page.getByTestId("wb-ios-tabs")).toBeVisible();
  });
});
