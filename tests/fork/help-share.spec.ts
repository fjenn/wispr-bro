// SPDX-License-Identifier: GPL-3.0-or-later
// S-26b: the Help and Feedback popover, Report a problem, Share with a
// friend and About. Mocked IPC; links only open through the opener plugin
// (the user's browser), never fetched by the page.
import {
  test,
  expect,
  expectNoSeriousAxeFindings,
  openHub,
} from "./hub-fixture";

const RELEASES = "https://github.com/fjenn/wispr-bro/releases";
const DOCS = "https://github.com/fjenn/wispr-bro/tree/wispr-bro/docs/fork/user";
const DOWNLOAD = "https://github.com/fjenn/wispr-bro/releases/latest";
const BREW = "brew install --cask fjenn/tap/wispr-bro";

const HELP_ITEMS = [
  "whats-new",
  "shortcuts",
  "microphone",
  "languages",
  "documentation",
  "report-problem",
  "send-feedback",
];

test.describe("help popover", () => {
  test("groups and keyboard navigation", async ({ page, ipc, hub }) => {
    await openHub(page, ipc, hub);
    const trigger = page.getByTestId("wb-nav-help");
    await trigger.focus();
    await page.keyboard.press("Enter");
    const menu = page.getByRole("menu", { name: "Help" });
    await expect(menu).toBeVisible();
    for (const group of ["Updates", "Essentials", "Get in touch"]) {
      await expect(menu.getByText(group, { exact: true })).toBeVisible();
    }
    // Arrow keys walk every item in order across the groups; Esc returns
    // focus to the Help button.
    for (const id of HELP_ITEMS) {
      await expect(page.getByTestId(`wb-menu-item-${id}`)).toBeFocused();
      await page.keyboard.press("ArrowDown");
    }
    await expect(page.getByTestId("wb-menu-item-send-feedback")).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(page.getByTestId("wb-menu-item-report-problem")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(menu).toBeHidden();
    await expect(trigger).toBeFocused();
  });

  test("Essentials deep-link into Settings", async ({ page, ipc, hub }) => {
    ipc.respond("fork_mic_list", null);
    await openHub(page, ipc, hub);
    for (const [id, section] of [
      ["shortcuts", "hotkeys"],
      ["microphone", "microphone"],
      ["languages", "languages"],
    ] as const) {
      await page.getByTestId("wb-nav-help").click();
      await page.getByTestId(`wb-menu-item-${id}`).click();
      await expect(page).toHaveURL(new RegExp(`#/hub/settings/${section}$`));
      // Esc closes the hotkeys panel, then the modal.
      const modal = page.getByTestId("wb-settings-modal");
      await expect(async () => {
        if ((await modal.count()) > 0) await page.keyboard.press("Escape");
        await expect(modal).toHaveCount(0, { timeout: 500 });
      }).toPass();
    }
  });

  test("What's new and Documentation open GitHub in the browser", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await page.getByTestId("wb-nav-help").click();
    await page.getByTestId("wb-menu-item-whats-new").click();
    await ipc.expectCalled("plugin:opener|open_url", { url: RELEASES });
    await page.getByTestId("wb-nav-help").click();
    await page.getByTestId("wb-menu-item-documentation").click();
    await ipc.expectCalled("plugin:opener|open_url", { url: DOCS });
    // Nothing else was opened or fetched.
    const urls = (await ipc.calls("plugin:opener|open_url")).map(
      (c) => c.args.url,
    );
    expect(urls).toEqual([RELEASES, DOCS]);
  });

  test("Send feedback opens the prefilled feedback form", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await page.getByTestId("wb-nav-help").click();
    await page.getByTestId("wb-menu-item-send-feedback").click();
    await ipc.expectCalled("fork_open_issue", { kind: "feedback" });
    await expect(page.getByTestId("wb-report-dialog")).toHaveCount(0);
  });

  test("Report a problem: dialog, Copy diagnostics, Open GitHub (keyboard only)", async ({
    page,
    ipc,
    hub,
  }) => {
    ipc.respond("fork_copy_diagnostics", 2048);
    await openHub(page, ipc, hub);
    await page.getByTestId("wb-nav-help").focus();
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("wb-menu-item-whats-new")).toBeFocused();
    for (const id of HELP_ITEMS.slice(
      1,
      HELP_ITEMS.indexOf("report-problem") + 1,
    )) {
      await page.keyboard.press("ArrowDown");
      await expect(page.getByTestId(`wb-menu-item-${id}`)).toBeFocused();
    }
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Report a problem" });
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText("never includes anything you dictated");
    // The primary action has focus; Shift+Tab reaches Copy diagnostics.
    await expect(page.getByTestId("wb-report-open")).toBeFocused();
    const copy = page.getByTestId("wb-report-copy-diagnostics");
    for (
      let i = 0;
      i < 5 && !(await copy.evaluate((e) => e === document.activeElement));
      i++
    ) {
      await page.keyboard.press("Shift+Tab");
    }
    await expect(copy).toBeFocused();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_copy_diagnostics");
    await expect(page.getByTestId("wb-report-copy-status")).toHaveText(
      "Diagnostics copied. Paste them into the issue.",
    );
    expect(await ipc.calls("fork_open_issue")).toHaveLength(0);
    await page.getByTestId("wb-report-open").focus();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_open_issue", { kind: "problem" });
    await expect(dialog).toHaveCount(0);
  });

  test("Report a problem: a failed copy says so; Cancel opens nothing", async ({
    page,
    ipc,
    hub,
  }) => {
    ipc.respond("fork_copy_diagnostics", {
      __error: { kind: "Internal", message: "x" },
    });
    await openHub(page, ipc, hub);
    await page.getByTestId("wb-nav-help").click();
    await page.getByTestId("wb-menu-item-report-problem").click();
    await page.getByTestId("wb-report-copy-diagnostics").click();
    await expect(page.getByTestId("wb-report-copy-status")).toHaveText(
      "Couldn't copy diagnostics.",
    );
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(page.getByTestId("wb-report-dialog")).toHaveCount(0);
    expect(await ipc.calls("fork_open_issue")).toHaveLength(0);
  });
});

test.describe("share with a friend", () => {
  test("copies the link and the Homebrew line, no tracking", async ({
    page,
    ipc,
    hub,
    context,
  }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await openHub(page, ipc, hub, { hash: "#/hub/share" });
    await expect(page.getByTestId("wb-share")).toBeVisible();
    await expect(page.getByTestId("wb-share-link")).toHaveValue(DOWNLOAD);
    await expect(page.getByTestId("wb-share-brew")).toHaveValue(BREW);
    expect(new URL(DOWNLOAD).search).toBe("");

    await page.getByRole("button", { name: "Copy download link" }).click();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
      DOWNLOAD,
    );
    await expect(page.getByTestId("wb-share-status-link")).toHaveText("Copied");
    await page.getByRole("button", { name: "Copy Homebrew command" }).click();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
      BREW,
    );
    await expect(page.getByTestId("wb-share-status-brew")).toHaveText("Copied");
    // Copying never talks to the backend or opens anything.
    const names = await ipc.commandNames();
    expect(names.filter((n) => n.includes("opener"))).toEqual([]);
  });

  test("Share… opens the share sheet next to the button", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub, { hash: "#/hub/share" });
    const button = page.getByTestId("wb-share-sheet");
    const box = await button.boundingBox();
    await button.focus();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_share_sheet");
    const [call] = await ipc.calls("fork_share_sheet");
    const anchor = call.args.anchor as Record<string, number>;
    expect(anchor.x).toBeCloseTo(box!.x, 0);
    expect(anchor.y).toBeCloseTo(box!.y, 0);
    expect(anchor.width).toBeCloseTo(box!.width, 0);
    expect(anchor.height).toBeCloseTo(box!.height, 0);
    await expect(page.getByTestId("wb-share-sheet-status")).toHaveText("");

    ipc.respond("fork_share_sheet", {
      __error: { kind: "Internal", message: "no window" },
    });
    await button.click();
    await expect(page.getByTestId("wb-share-sheet-status")).toHaveText(
      "Couldn't open the share sheet.",
    );
  });
});

test("About: version, GPL-3.0, source, Handy credit and support", async ({
  page,
  ipc,
  hub,
}) => {
  await openHub(page, ipc, hub, { hash: "#/hub/settings/about" });
  await expect(page.getByTestId("wb-about-version")).toHaveText(
    "Version 1.0.0-alpha.1",
  );
  await expect(page.getByTestId("wb-about-license")).toContainText("GPL-3.0");
  await expect(page.getByTestId("wb-about-credit")).toContainText(
    "Built on Handy by CJ Pais",
  );
  for (const [id, url] of [
    ["wb-about-source", "https://github.com/fjenn/wispr-bro"],
    [
      "wb-about-license-link",
      "https://github.com/fjenn/wispr-bro/blob/wispr-bro/LICENSE",
    ],
    ["wb-about-handy", "https://github.com/cjpais/Handy"],
  ] as const) {
    await page.getByTestId(id).click();
    await ipc.expectCalled("plugin:opener|open_url", { url });
  }
  await page.getByTestId("wb-about-report").click();
  await ipc.expectCalled("fork_open_issue", { kind: "problem" });
  await page.getByTestId("wb-about-feedback").click();
  await ipc.expectCalled("fork_open_issue", { kind: "feedback" });
});

for (const theme of ["light", "dark"] as const) {
  for (const app_language of ["en", "ar"] as const) {
    const dir = app_language === "ar" ? "rtl" : "ltr";
    test(`axe: Share, Help popover, Report dialog and About (${theme}, ${dir}, reduced motion)`, async ({
      page,
      ipc,
      hub,
    }) => {
      test.setTimeout(180_000);
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openHub(page, ipc, hub, {
        hash: "#/hub/share",
        handy: { theme, app_language },
      });
      await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", dir);
      await expect(page.getByTestId("wb-share")).toBeVisible();
      // Links and commands stay left-to-right inside a mirrored page.
      await expect(page.getByTestId("wb-share-brew")).toHaveAttribute(
        "dir",
        "ltr",
      );
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-nav-help").click();
      await expect(page.getByTestId("wb-help-menu")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-menu-item-report-problem").click();
      await expect(page.getByTestId("wb-report-dialog")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.keyboard.press("Escape");
      await expect(page.getByTestId("wb-report-dialog")).toHaveCount(0);
      await page.evaluate(() => {
        window.location.hash = "#/hub/settings/about";
      });
      await expect(page.getByTestId("wb-about-support")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }
}
