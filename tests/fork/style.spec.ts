// SPDX-License-Identifier: GPL-3.0-or-later
// S-19a/b Style page: the five tabs (Personal / Work / Email / Other /
// Cleanup) and their deep links, each group's apps and sites (move, remove,
// back to the usual group, Undo, "Press Return after dictating"), the "+" picker
// (search, apps already in the group cannot be picked again), the presets
// as before/after cards (Personal: Formal / Relaxed / Very relaxed; the
// others: Formal / Relaxed / Upbeat) saved in the fork setting `styles`,
// the English-only note, the Cleanup tab's level picker, keyboard-only use,
// RTL and axe in light and dark (R5 S-19a, §5.5, §11.2). Mocked IPC with a
// small in-test backend; the rules, the precedence and the store are tested
// in Rust (`fork::context::app_rules`, `fork::text::style_rules`).
//
// WB_SHOTS_DIR=<dir> also writes screenshots of each state.
import path from "node:path";
import type { Page } from "@playwright/test";
import type {
  StyleApp,
  StyleGroup,
  StylePickerApp,
  StyleSite,
} from "../../src/fork/bindings";
import type { IpcMock } from "./fixtures";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
} from "./hub-fixture";

const shotsDir = process.env.WB_SHOTS_DIR;
async function shot(page: Page, name: string) {
  if (!shotsDir) return;
  await page
    .screenshot({
      path: path.join(shotsDir, `s19a-${name}.png`),
      animations: "disabled",
      timeout: 10_000,
    })
    .catch((e) => console.warn(`screenshot ${name} skipped: ${e}`));
}

// A 1×1 PNG, as `fork_style_app_icon` returns icons.
const ICON =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

const BUILTIN: Record<string, StyleGroup> = {
  "com.apple.MobileSMS": "personal",
  "net.whatsapp.WhatsApp": "personal",
  "com.tinyspeck.slackmacgap": "work",
  "com.apple.mail": "email",
  "com.apple.Notes": "other",
};

const BUILTIN_SITES: Record<string, { name: string; group: StyleGroup }> = {
  "app.slack.com": { name: "Slack", group: "work" },
  "mail.google.com": { name: "Gmail", group: "email" },
};

/** A small stand-in for `fork::commands::style`. */
class FakeStyle {
  installed: StylePickerApp[] = [
    { bundle_id: "com.apple.MobileSMS", name: "Messages", group: "personal" },
    { bundle_id: "net.whatsapp.WhatsApp", name: "WhatsApp", group: "personal" },
    { bundle_id: "com.tinyspeck.slackmacgap", name: "Slack", group: "work" },
    { bundle_id: "com.apple.mail", name: "Mail", group: "email" },
    { bundle_id: "com.apple.Notes", name: "Notes", group: "other" },
    { bundle_id: "com.example.Editor", name: "Editor", group: "other" },
  ];
  /** bundle id → { group, auto_send } (the user's rules). */
  rules = new Map<string, { group: StyleGroup | null; auto: boolean }>();
  siteRules = new Map<string, { group: StyleGroup | null; auto: boolean }>();

  builtin(id: string): StyleGroup {
    return BUILTIN[id] ?? "other";
  }

  app(id: string): StyleApp {
    const rule = this.rules.get(id);
    const builtin = this.builtin(id);
    return {
      bundle_id: id,
      name: this.installed.find((a) => a.bundle_id === id)?.name ?? id,
      installed: true,
      group: rule?.group ?? builtin,
      builtin_group: builtin,
      user_group: rule?.group ?? null,
      auto_send: rule?.auto ?? false,
      ai_chat: false,
    };
  }

  site(host: string): StyleSite {
    const rule = this.siteRules.get(host);
    const builtin = BUILTIN_SITES[host] ?? { name: host, group: "other" };
    return {
      host,
      name: builtin.name,
      group: rule?.group ?? builtin.group,
      builtin_group: builtin.group,
      user_group: rule?.group ?? null,
      auto_send: rule?.auto ?? false,
    };
  }

  overview() {
    const ids = new Set([...Object.keys(BUILTIN), ...this.rules.keys()]);
    const hosts = new Set([
      ...Object.keys(BUILTIN_SITES),
      ...this.siteRules.keys(),
    ]);
    return {
      apps: [...ids]
        .map((id) => this.app(id))
        .sort((a, b) => a.name.localeCompare(b.name)),
      sites: [...hosts]
        .map((host) => this.site(host))
        .sort((a, b) => a.name.localeCompare(b.name)),
    };
  }

  set(id: string, patch: Partial<{ group: StyleGroup | null; auto: boolean }>) {
    const rule = { group: null, auto: false, ...this.rules.get(id), ...patch };
    if (rule.group === null && !rule.auto) this.rules.delete(id);
    else this.rules.set(id, rule);
    return this.overview();
  }

  setSite(
    host: string,
    patch: Partial<{ group: StyleGroup | null; auto: boolean }>,
  ) {
    const rule = {
      group: null,
      auto: false,
      ...this.siteRules.get(host),
      ...patch,
    };
    if (rule.group === null && !rule.auto) this.siteRules.delete(host);
    else this.siteRules.set(host, rule);
    return this.overview();
  }

  install(ipc: IpcMock) {
    ipc.handle("fork_style_overview", () => this.overview());
    ipc.handle("fork_style_app_icon", () => ICON);
    ipc.handle("fork_style_installed_apps", () =>
      this.installed.map((a) => ({ ...a, group: this.app(a.bundle_id).group })),
    );
    ipc.handle("fork_style_set_app_group", (a) =>
      this.set(a.bundleId as string, {
        group: (a.group as StyleGroup | null) ?? null,
      }),
    );
    ipc.handle("fork_style_remove_app", (a) => {
      const id = a.bundleId as string;
      return this.set(id, {
        group: this.builtin(id) === "other" ? null : "other",
      });
    });
    ipc.handle("fork_style_set_auto_send", (a) =>
      this.set(a.bundleId as string, { auto: a.on as boolean }),
    );
    ipc.handle("fork_style_set_site_group", (a) =>
      this.setSite(a.host as string, {
        group: (a.group as StyleGroup | null) ?? null,
      }),
    );
    ipc.handle("fork_style_remove_site", (a) => {
      const host = a.host as string;
      return this.setSite(host, {
        group: BUILTIN_SITES[host]?.group === "other" ? null : "other",
      });
    });
    ipc.handle("fork_style_set_site_auto_send", (a) =>
      this.setSite(a.host as string, { auto: a.on as boolean }),
    );
  }
}

const styleTest = test.extend<{ style: FakeStyle }>({
  style: async ({}, use) => {
    await use(new FakeStyle());
  },
});

async function openStyle(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  style: FakeStyle,
  options: Parameters<typeof openHub>[3] = {},
) {
  await openHub(page, ipc, hub, { hash: "#/hub/style", ...options });
  style.install(ipc);
  ipc.respond({
    fork_llm_last_cleanup: null,
  });
  await page.reload();
  await expect(page.getByTestId("wb-style")).toBeVisible();
}

const chips = (page: Page) => page.getByTestId("wb-style-app");
const chip = (page: Page, id: string) =>
  page.locator(`[data-testid="wb-style-app"][data-bundle-id="${id}"]`);

styleTest.describe("style page", () => {
  styleTest(
    "five tabs; Personal first with its apps and three presets",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style);
      await expect(page.getByTestId("wb-page-title")).toHaveText("Style");
      const tabs = page.getByRole("tablist", { name: "Style groups" });
      await expect(tabs.getByRole("tab")).toHaveText([
        "Personal messages",
        "Work messages",
        "Email",
        "Other",
        "Cleanup",
      ]);
      await expect(
        page.getByRole("tab", { name: "Personal messages" }),
      ).toHaveAttribute("aria-selected", "true");
      await expect(chips(page)).toHaveText(["Messages", "WhatsApp"]);
      const radios = page.getByRole("radio");
      await expect(radios).toHaveCount(3);
      await expect(page.getByTestId("wb-style-preset-formal")).toBeVisible();
      await expect(page.getByTestId("wb-style-preset-relaxed")).toHaveAttribute(
        "data-selected",
        "true",
      );
      await expect(
        page.getByTestId("wb-style-preset-very_relaxed"),
      ).toBeVisible();
      await expect(page.getByTestId("wb-style-preset-upbeat")).toHaveCount(0);
      await expect(page.getByTestId("wb-style-before")).toHaveAttribute(
        "lang",
        "en",
      );
      await expect(
        page.getByTestId("wb-style-example-very_relaxed"),
      ).toHaveText(
        "hey Sam are you around this weekend? we could grab dinner on Saturday if you're free",
      );
      await expect(page.getByTestId("wb-style-english-note")).toHaveText(
        "Styles apply to dictations in English. Cleanup works in every language.",
      );
      await shot(page, "personal");
    },
  );

  styleTest(
    "a preset card saves the group's style",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style);
      const veryRelaxed = page.getByRole("radio", { name: "Very relaxed" });
      await veryRelaxed.click();
      await expect(veryRelaxed).toBeChecked();
      await ipc.expectCalled("fork_set_setting", {
        update: {
          key: "styles",
          value: {
            personal: "very_relaxed",
            work: "formal",
            email: "formal",
            other: "formal",
          },
        },
      });
      await expect(
        page.getByTestId("wb-style-preset-very_relaxed"),
      ).toHaveAttribute("data-selected", "true");
      // Work offers Upbeat instead of Very relaxed.
      await page.getByRole("tab", { name: "Work messages" }).click();
      await expect(page).toHaveURL(/#\/hub\/style\/work$/);
      await expect(chips(page)).toHaveText(["Slack"]);
      await expect(
        page.getByTestId("wb-style-preset-very_relaxed"),
      ).toHaveCount(0);
      const upbeat = page.getByRole("radio", { name: "Upbeat" });
      await upbeat.click();
      await expect(upbeat).toBeChecked();
      await expect
        .poll(() => hub.forkSettings.styles)
        .toMatchObject({
          personal: "very_relaxed",
          work: "upbeat",
        });
      await expect(page.getByTestId("wb-style-example-upbeat")).toContainText(
        "for the quick turnaround!",
      );
      await shot(page, "work");
    },
  );

  styleTest(
    "deep links open a tab; Cleanup holds the level picker",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style, { hash: "#/hub/style/email" });
      await expect(page.getByRole("tab", { name: "Email" })).toHaveAttribute(
        "aria-selected",
        "true",
      );
      await expect(chips(page)).toHaveText(["Mail"]);
      await page.getByRole("tab", { name: "Cleanup" }).click();
      await expect(page.getByTestId("wb-style-cleanup")).toBeVisible();
      // Controlled radio: it changes after the backend answers, so click
      // (check() fails if the state has not flipped yet under load).
      await page.getByRole("radio", { name: /Medium/ }).click();
      await ipc.expectCalled("fork_set_setting", {
        update: { key: "cleanup_level", value: "medium" },
      });
      await shot(page, "cleanup");
    },
  );

  styleTest(
    "sites appear alongside apps and use the same group controls",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style, { hash: "#/hub/style/email" });
      await expect(chips(page)).toHaveText(["Mail"]);
      const gmail = page.locator(
        '[data-testid="wb-style-site"][data-host="mail.google.com"]',
      );
      await expect(gmail).toBeVisible();
      await expect(gmail).toContainText("Gmail");
      await expect(gmail).toContainText("mail.google.com");
      await expect(gmail).toHaveAccessibleName(
        "Gmail mail.google.com: options",
      );
      await gmail.click();
      await expectNoSeriousAxeFindings(page);
      await page
        .getByRole("menu")
        .getByRole("menuitem", { name: "Work messages" })
        .click();
      await ipc.expectCalled("fork_style_set_site_group", {
        host: "mail.google.com",
        group: "work",
      });
      await expect(page.getByTestId("wb-style-site")).toHaveCount(0);
      await page.getByRole("tab", { name: "Work messages" }).click();
      await expect(chips(page)).toHaveText(["Slack"]);
      await expect(page.getByTestId("wb-style-site")).toHaveCount(2);
      await expect(page.getByTestId("wb-style-sites")).toContainText(
        "app.slack.com",
      );
      await expect(page.getByTestId("wb-style-sites")).toContainText(
        "mail.google.com",
      );
      await gmail.click();
      await page
        .getByRole("menuitem", {
          name: "Back to its usual group (Email)",
        })
        .click();
      await ipc.expectCalled("fork_style_set_site_group", {
        host: "mail.google.com",
        group: null,
      });
      await expect(page.getByTestId("wb-style-site")).toHaveCount(1);
      await shot(page, "sites");
    },
  );

  styleTest(
    "move an app to another group, then Undo",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style);
      await chip(page, "com.apple.MobileSMS").click();
      // Radix labels the menu by its trigger ("Messages: options").
      const menu = page.getByRole("menu");
      await expect(menu).toBeVisible();
      await shot(page, "menu");
      await menu.getByRole("menuitem", { name: "Work messages" }).click();
      await ipc.expectCalled("fork_style_set_app_group", {
        bundleId: "com.apple.MobileSMS",
        group: "work",
      });
      await expect(chips(page)).toHaveText(["WhatsApp"]);
      await expect(
        page.getByText("Messages now uses the Work messages style."),
      ).toBeVisible();
      await page.getByRole("button", { name: "Undo" }).click();
      await expect(chips(page)).toHaveText(["Messages", "WhatsApp"]);
      // A moved app can go back to its usual group from the menu.
      await page.getByRole("tab", { name: "Personal messages" }).click();
      await chip(page, "com.apple.MobileSMS").click();
      await page
        .getByRole("menu")
        .getByRole("menuitem", { name: "Email" })
        .click();
      await page.getByRole("tab", { name: "Email" }).click();
      await expect(chips(page)).toHaveText(["Mail", "Messages"]);
      await chip(page, "com.apple.MobileSMS").click();
      await page
        .getByRole("menuitem", {
          name: "Back to its usual group (Personal messages)",
        })
        .click();
      await expect(chips(page)).toHaveText(["Mail"]);
    },
  );

  styleTest(
    "remove an app: it joins Other",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style, { hash: "#/hub/style/work" });
      await chip(page, "com.tinyspeck.slackmacgap").click();
      await page
        .getByRole("menuitem", { name: "Remove from Work messages" })
        .click();
      await ipc.expectCalled("fork_style_remove_app", {
        bundleId: "com.tinyspeck.slackmacgap",
      });
      await expect(chips(page)).toHaveCount(0);
      await expect(page.getByTestId("wb-style-apps-empty")).toBeVisible();
      await page.getByRole("tab", { name: "Other" }).click();
      await expect(chips(page)).toHaveText(["Notes", "Slack"]);
      // Other has no Remove: every other app is in it already.
      await chip(page, "com.tinyspeck.slackmacgap").click();
      await expect(page.getByRole("menuitem", { name: /^Remove/ })).toHaveCount(
        0,
      );
      await expect(
        page.getByText(
          "Every app that is not in another group uses this style too.",
        ),
      ).toBeVisible();
    },
  );

  styleTest(
    "Press Return after dictating is off by default and set per app",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style, { hash: "#/hub/style/work" });
      const slack = chip(page, "com.tinyspeck.slackmacgap");
      await expect(slack).toHaveAttribute("data-auto-send", "false");
      await slack.click();
      const toggle = page.getByRole("menuitemcheckbox", {
        name: "Press Return after dictating",
      });
      await expect(toggle).toHaveAttribute("aria-checked", "false");
      await toggle.click();
      await ipc.expectCalled("fork_style_set_auto_send", {
        bundleId: "com.tinyspeck.slackmacgap",
        on: true,
      });
      await expect(toggle).toHaveAttribute("aria-checked", "true");
      await page.keyboard.press("Escape");
      await expect(slack).toHaveAttribute("data-auto-send", "true");
      await expect(slack).toHaveAccessibleName(
        "Slack, presses Return after dictating: options",
      );
      await shot(page, "auto-send");
    },
  );

  styleTest(
    "the + picker: search, add, and apps already here cannot be picked",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style);
      await page.getByTestId("wb-style-add-app").click();
      const dialog = page.getByRole("dialog", {
        name: "Add an app to Personal messages",
      });
      await expect(dialog).toBeVisible();
      await expect(page.getByTestId("wb-style-picker-search")).toBeFocused();
      const apps = page.getByTestId("wb-style-picker-app");
      await expect(apps).toHaveCount(6);
      await expect(
        page.locator(
          '[data-testid="wb-style-picker-app"][data-bundle-id="com.apple.MobileSMS"]',
        ),
      ).toBeDisabled();
      await shot(page, "picker");
      await page.getByTestId("wb-style-picker-search").fill("edi");
      await expect(apps).toHaveCount(1);
      await expect(apps).toContainText("In Other");
      await apps.click();
      await ipc.expectCalled("fork_style_set_app_group", {
        bundleId: "com.example.Editor",
        name: "Editor",
        group: "personal",
      });
      await expect(dialog).toBeHidden();
      await expect(chips(page)).toHaveText(["Editor", "Messages", "WhatsApp"]);
      await page.getByTestId("wb-style-add-app").click();
      await page.getByTestId("wb-style-picker-search").fill("zzz");
      await expect(page.getByText("No app matches.")).toBeVisible();
    },
  );

  styleTest(
    "a change in another window refreshes the page",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style);
      await expect(chips(page)).toHaveCount(2);
      style.set("com.apple.Notes", { group: "personal" });
      await ipc.waitForListener("fork-app-rules-changed");
      await ipc.emit("fork-app-rules-changed", { rules: 1 });
      await expect(chips(page)).toHaveText(["Messages", "Notes", "WhatsApp"]);
    },
  );

  styleTest(
    "keyboard only: tabs, a preset, an app's menu",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style);
      await page.getByRole("tab", { name: "Personal messages" }).focus();
      await page.keyboard.press("ArrowRight");
      await expect(
        page.getByRole("tab", { name: "Work messages" }),
      ).toBeFocused();
      await expect(
        page.getByRole("tab", { name: "Work messages" }),
      ).toHaveAttribute("aria-selected", "true");
      // Into the panel: the first app, then its menu.
      await page.keyboard.press("Tab");
      await page.keyboard.press("Tab");
      await expect(chip(page, "com.tinyspeck.slackmacgap")).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(page.getByRole("menu")).toBeVisible();
      await page.keyboard.press("Escape");
      await expect(chip(page, "com.tinyspeck.slackmacgap")).toBeFocused();
      // Add app, the site chip, then the selected preset.
      await page.keyboard.press("Tab");
      await expect(page.getByTestId("wb-style-add-app")).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(
        page.locator(
          '[data-testid="wb-style-site"][data-host="app.slack.com"]',
        ),
      ).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(page.getByRole("radio", { name: "Formal" })).toBeFocused();
      await page.keyboard.press("ArrowRight");
      await expect(page.getByRole("radio", { name: "Relaxed" })).toBeChecked();
      await expect
        .poll(() => hub.forkSettings.styles)
        .toMatchObject({
          work: "relaxed",
        });
    },
  );

  styleTest(
    "RTL: the page mirrors and stays accessible; examples stay English",
    async ({ page, ipc, hub, style }) => {
      await openStyle(page, ipc, hub, style, {
        handy: { app_language: "ar" },
      });
      await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
      await expect(page.getByTestId("wb-style-example-formal")).toHaveAttribute(
        "dir",
        "ltr",
      );
      await expectNoSeriousAxeFindings(page);
      await shot(page, "rtl");
    },
  );
});

for (const theme of ["light", "dark"] as const) {
  styleTest(
    `axe: every tab, the app menu and the picker (${theme})`,
    async ({ page, ipc, hub, style }) => {
      await page.emulateMedia({ colorScheme: theme });
      style.set("com.tinyspeck.slackmacgap", { auto: true });
      await openStyle(page, ipc, hub, style, { handy: { theme } });
      for (const tab of [
        "Personal messages",
        "Work messages",
        "Email",
        "Other",
        "Cleanup",
      ]) {
        await page.getByRole("tab", { name: tab }).click();
        await expectNoSeriousAxeFindings(page);
      }
      await shot(page, `cleanup-${theme}`);
      await page.getByRole("tab", { name: "Work messages" }).click();
      await shot(page, `work-${theme}`);
      await chip(page, "com.tinyspeck.slackmacgap").click();
      await expect(page.getByRole("menu")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-style-add-app").click();
      await expect(page.getByTestId("wb-style-picker-app")).toHaveCount(6);
      await expectNoSeriousAxeFindings(page);
      await shot(page, `picker-${theme}`);
    },
  );
}
