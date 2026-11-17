// SPDX-License-Identifier: GPL-3.0-or-later
// Wispr Bro for iPhone: the phone shell, Home's session and setup, Settings
// and onboarding, on the mocked backend (`ios-fixture.ts`).
import { test, expect, note, openIos, openIosOnboarding } from "./ios-fixture";

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
    for (const tab of ["home", "history", "voice", "scratchpad", "settings"]) {
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
    await page.getByTestId("wb-ios-tab-voice").click();
    await page.getByTestId("wb-ios-voice-dictionary").click();
    await expect(page.getByTestId("wb-dictionary")).toBeVisible();
    await expect(page.getByTestId("wb-dictionary-import")).toBeHidden();
    await page.getByTestId("wb-ios-tab-scratchpad").click();
    await expect(page.getByTestId("wb-ios-scratchpad")).toBeVisible();
    await page.getByTestId("wb-ios-tab-settings").click();
    await expect(page.getByTestId("wb-ios-settings")).toBeVisible();
    await expect(page.getByTestId("wb-settings-modal")).toHaveCount(0);
  });

  test("My Voice is three rows that open Dictionary, Snippets and Style", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    await openIos(page, ipc, hub, ios, {
      beforeNavigate: () => {
        ipc.respond("fork_dictionary_list", {
          entries: [],
          counts: { words: 40, rules: 2, suggested: 0 },
        });
        ipc.respond("fork_snippets_list", {
          snippets: [],
          counts: { snippets: 6, uses: 0 },
        });
      },
    });
    const tabs = page.getByTestId("wb-ios-tabs").getByRole("button");
    await expect(tabs).toHaveText([
      "Home",
      "History",
      "My Voice",
      "Scratchpad",
      "Settings",
    ]);
    const tab = page.getByTestId("wb-ios-tab-voice");
    await tab.click();
    const voice = page.getByTestId("wb-ios-voice");
    await expect(voice.getByRole("button")).toHaveCount(3);
    await expect(page.getByTestId("wb-ios-voice-dictionary")).toContainText(
      "Dictionary",
    );
    await expect(
      page.getByTestId("wb-ios-voice-dictionary-summary"),
    ).toHaveText("42 words");
    await expect(page.getByTestId("wb-ios-voice-snippets-summary")).toHaveText(
      "6 snippets",
    );
    await expect(
      page.getByTestId("wb-ios-voice-style-summary"),
    ).not.toBeEmpty();
    await expect(page.getByTestId("wb-ios-back")).toHaveCount(0);

    // A row pushes its page; the tab stays lit; Back returns to the rows.
    await page.getByTestId("wb-ios-voice-dictionary").click();
    await expect(page.getByTestId("wb-dictionary")).toBeVisible();
    await expect(page.getByTestId("wb-dictionary-add")).toBeVisible();
    await expect(tab).toHaveAttribute("aria-current", "page");
    await expect(page.getByTestId("wb-ios-back")).toHaveText("My Voice");
    await page.getByTestId("wb-ios-back").click();
    await expect(voice).toBeVisible();

    await page.getByTestId("wb-ios-voice-snippets").click();
    await expect(page.getByTestId("wb-snippets")).toBeVisible();
    await page.getByTestId("wb-snippets-add").click();
    await expect(page.getByTestId("wb-snippet-dialog")).toBeVisible();
    await page.keyboard.press("Escape");
    // The tab again: back to the rows.
    await tab.click();
    await expect(voice).toBeVisible();

    await page.getByTestId("wb-ios-voice-style").click();
    await expect(page.getByTestId("wb-style")).toBeVisible();
    // A swipe in from the left edge goes back.
    await page.getByTestId("wb-ios").evaluate((root) => {
      const touch = (type: string, x: number) => {
        const point = new Touch({
          identifier: 1,
          target: root,
          clientX: x,
          clientY: 400,
        });
        root.dispatchEvent(
          new TouchEvent(type, {
            bubbles: true,
            touches: type === "touchend" ? [] : [point],
            changedTouches: [point],
          }),
        );
      };
      touch("touchstart", 6);
      touch("touchend", 180);
    });
    await expect(voice).toBeVisible();

    // Style has one home: not on Home, not in Settings.
    await page.getByTestId("wb-ios-tab-home").click();
    await expect(page.getByTestId("wb-ios-home-style")).toHaveCount(0);
    await page.getByTestId("wb-ios-tab-settings").click();
    await expect(page.getByTestId("wb-ios-settings-style")).toHaveCount(0);
    // A link to one of the pages opens it under My Voice.
    await page.evaluate(() => {
      window.location.hash = "#/hub/snippets";
    });
    await expect(page.getByTestId("wb-snippets")).toBeVisible();
    await expect(tab).toHaveAttribute("aria-current", "page");
  });

  test("Scratchpad: new note, autosave, pin, search and delete", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    await openIos(page, ipc, hub, ios, { hash: "#/hub/scratchpad" });
    await expect(page.getByTestId("wb-scratchpad-empty")).toBeVisible();
    await page
      .getByTestId("wb-scratchpad-empty")
      .getByRole("button", { name: "New note" })
      .click();
    await ipc.expectCalled("fork_scratchpad_new", {});
    const editor = page.getByTestId("wb-ios-note");
    await expect(editor).toBeVisible();
    // The editor covers the tab bar.
    await expect(page.getByTestId("wb-ios-tabs")).toBeHidden();
    await page
      .getByTestId("wb-note-body")
      .fill("Groceries\nMilk and two lemons");
    await expect(page.getByTestId("wb-note-status")).toHaveText("5 words");
    await ipc.expectCalled("fork_scratchpad_save", {
      id: 1,
      body: "Groceries\nMilk and two lemons",
    });
    await expect(editor).toHaveAttribute("data-save", "saved");
    await page.getByTestId("wb-ios-back").click();
    await expect(page.getByTestId("wb-note-1")).toContainText("Groceries");
    await expect(page.getByTestId("wb-note-1")).toContainText(
      "Milk and two lemons",
    );

    ios.notes.push(note(2, "Ideas for Saturday"));
    await page.getByTestId("wb-ios-tab-home").click();
    await page.getByTestId("wb-ios-tab-scratchpad").click();
    await page.getByTestId("wb-note-more-2").click();
    await page.getByTestId("wb-menu-item-pin").click();
    await ipc.expectCalled("fork_scratchpad_set_pinned", {
      id: 2,
      pinned: true,
    });
    await expect(
      page.getByTestId("wb-scratchpad-pinned").getByTestId("wb-note-2"),
    ).toBeVisible();

    await page.getByTestId("wb-scratchpad-search").fill("lemons");
    await expect(page.getByTestId("wb-note-2")).toHaveCount(0);
    await expect(page.getByTestId("wb-note-1")).toBeVisible();
    await page.getByTestId("wb-scratchpad-search").fill("nothing like this");
    await expect(page.getByTestId("wb-scratchpad-no-results")).toBeVisible();
    await page.getByTestId("wb-scratchpad-search").fill("");

    await page.getByTestId("wb-note-more-1").click();
    await page.getByTestId("wb-menu-item-delete").click();
    await page.getByTestId("wb-note-delete-confirm").click();
    await ipc.expectCalled("fork_scratchpad_delete", { id: 1 });
    await expect(page.getByTestId("wb-note-1")).toHaveCount(0);
  });

  test("a note left empty is removed", async ({ page, ipc, hub, ios }) => {
    await openIos(page, ipc, hub, ios, { hash: "#/hub/scratchpad" });
    await page
      .getByTestId("wb-scratchpad-empty")
      .getByRole("button", { name: "New note" })
      .click();
    await expect(page.getByTestId("wb-note-body")).toBeVisible();
    await page.getByTestId("wb-ios-back").click();
    await ipc.expectCalled("fork_scratchpad_delete", { id: 1 });
    await expect(page.getByTestId("wb-scratchpad-empty")).toBeVisible();
  });

  test("Record puts a take at the note's caret", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.notes = [note(4, "Before after")];
    ios.takeText = "the dictated words";
    await openIos(page, ipc, hub, ios, { hash: "#/hub/scratchpad/4" });
    const body = page.getByTestId("wb-note-body");
    await expect(body).toHaveValue("Before after");
    await body.evaluate((el: HTMLTextAreaElement) => {
      el.focus();
      el.setSelectionRange(6, 6);
    });
    const record = page.getByTestId("wb-note-record");
    await record.click();
    await ipc.expectCalled("ios_take_start");
    await expect(record).toHaveAttribute("data-state", "listening");
    await expect(record).toHaveText("Stop");
    await expect(page.getByTestId("wb-note-record-level")).toBeVisible();
    await record.click();
    await ipc.expectCalled("ios_take_stop", { id: "1" });
    await expect(body).toHaveValue("Before the dictated words after");
    await expect(record).toHaveText("Record");
    await ipc.expectCalled("fork_scratchpad_save", {
      id: 4,
      body: "Before the dictated words after",
    });
  });

  test("Polish rewrites the note, with Undo", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.notes = [note(7, "so um this is the the note")];
    ios.polish = { outcome: "changed", text: "This is the note." };
    await openIos(page, ipc, hub, ios, { hash: "#/hub/scratchpad/7" });
    const body = page.getByTestId("wb-note-body");
    await page.getByTestId("wb-note-polish").click();
    await expect(body).toHaveValue("This is the note.");
    await expect(page.getByTestId("wb-note-notice")).toContainText(
      "Note polished",
    );
    await page.getByTestId("wb-note-polish-undo").click();
    await expect(body).toHaveValue("so um this is the the note");
  });

  test("Polish says why it can't run when the cleanup model is not here", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.notes = [note(7, "so um this is the the note")];
    // Cleanup off: the model is not on the phone.
    ios.llmState = "waiting_for_speech_model";
    await openIos(page, ipc, hub, ios, {
      hash: "#/hub/scratchpad/7",
      fork: { cleanup_level: "none" },
    });
    const before = (await ipc.calls("fork_scratchpad_polish")).length;
    await page.getByTestId("wb-note-polish").click();
    await expect(page.getByTestId("wb-note-notice")).toContainText(
      "Choose a cleanup level in Settings",
    );
    expect((await ipc.calls("fork_scratchpad_polish")).length).toBe(before);
    await page.getByTestId("wb-note-notice-settings").click();
    await expect(page.getByTestId("fork-cleanup-level-light")).toBeVisible();
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

  test("cleanup is not offered on an iPhone without Apple Intelligence", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.appleAvailability = "device_not_eligible";
    await openIos(page, ipc, hub, ios, { hash: "#/hub/settings/cleanup" });
    await expect(page.getByTestId("wb-ios-cleanup-none")).toBeVisible();
    await expect(page.getByTestId("fork-cleanup-level-light")).toBeDisabled();
    await expect(page.getByTestId("wb-ios-cleanup-note")).toHaveCount(0);
  });

  test("cleanup levels can be picked with Apple Intelligence on", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    await openIos(page, ipc, hub, ios, { hash: "#/hub/settings/cleanup" });
    await expect(page.getByTestId("wb-ios-cleanup-note")).toContainText(
      "Apple Intelligence",
    );
    await expect(page.getByTestId("wb-ios-cleanup-off")).toHaveCount(0);
    await page.getByTestId("fork-cleanup-level-light").click();
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "cleanup_level", value: "light" },
    });
  });

  test("with Apple Intelligence off the page says how to turn it on, and follows", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.appleAvailability = "not_enabled";
    await openIos(page, ipc, hub, ios, { hash: "#/hub/settings/cleanup" });
    await expect(page.getByTestId("wb-ios-cleanup-off")).toContainText(
      "Apple Intelligence & Siri",
    );
    await expect(page.getByTestId("fork-cleanup-level-medium")).toBeDisabled();
    // Switched on in the Settings app: the page notices by itself.
    ios.appleAvailability = "available";
    await expect(page.getByTestId("wb-ios-cleanup-off")).toHaveCount(0, {
      timeout: 8000,
    });
    await expect(page.getByTestId("fork-cleanup-level-medium")).toBeEnabled();
  });

  test("while Apple's model downloads a level can already be chosen", async ({
    page,
    ipc,
    hub,
    ios,
  }) => {
    ios.appleAvailability = "model_not_ready";
    await openIos(page, ipc, hub, ios, { hash: "#/hub/settings/cleanup" });
    await expect(page.getByTestId("wb-ios-cleanup-preparing")).toBeVisible();
    await expect(page.getByTestId("fork-cleanup-level-light")).toBeEnabled();
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
