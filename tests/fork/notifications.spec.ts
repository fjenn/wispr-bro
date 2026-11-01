// SPDX-License-Identifier: GPL-3.0-or-later
// S-18 notifications: the bell and its panel (list, filter, ⋯ menu with
// mark all read and the three toggles, empty states), entry text from
// `rust.notify.*` keys with grouped numbers and plurals, clicks and buttons
// through `fork_notifications_activate`, live updates and toasts, Esc
// dismissing toasts, macOS clicks routed by `fork-notification-opened`, the
// one-time feature popup, keyboard use and axe in light and dark (R5 S-18
// tests, §5.5, §11.2). Mocked IPC; see hub-fixture.ts.
import type { Page } from "@playwright/test";
import type {
  NotificationAction,
  NotificationItem,
} from "../../src/fork/bindings";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
  type OpenHubOptions,
} from "./hub-fixture";
import type { IpcMock } from "./fixtures";

const NOW = Date.now();

function entry(
  over: Partial<NotificationItem> & { id: number },
): NotificationItem {
  return {
    kind: "model_ready",
    title_key: "rust.notify.modelReady.title",
    body_key: "rust.notify.modelReady.body",
    args: {},
    action: { type: "open_page", page: "dictation", section: null },
    buttons: [],
    created_at: NOW - 5 * 60_000,
    read: false,
    ...over,
  };
}

/** A small inbox, newest first. */
function fixtureInbox(): NotificationItem[] {
  return [
    entry({
      id: 3,
      kind: "queued_takes_ready",
      title_key: "rust.notify.queuedTakesReady.title_other",
      body_key: "rust.notify.queuedTakesReady.body",
      args: { count: "3" },
      action: { type: "open_page", page: "settings", section: "history" },
      buttons: ["copy_latest", "open"],
      created_at: NOW - 20_000,
    }),
    entry({
      id: 2,
      kind: "milestone",
      title_key: "rust.notify.milestoneWords.title",
      body_key: "rust.notify.milestoneWords.body",
      args: { words: "10000" },
      action: { type: "open_page", page: "insights", section: null },
      created_at: NOW - 3 * 3_600_000,
    }),
    entry({ id: 1, read: true, created_at: NOW - 2 * 86_400_000 }),
  ];
}

const NOTIFICATIONS_ON = {
  system: true,
  model_ready: true,
  update_ready: true,
  queued_takes_ready: true,
  errors: true,
  suggestions: true,
  announcements: true,
  milestones: true,
};

interface Inbox {
  items: NotificationItem[];
  feature: Record<string, unknown> | null;
  /** What `fork_notifications_activate` answers per id (default: the entry's action). */
  actions: Record<number, NotificationAction | null>;
}

/** Installs a stateful inbox backend, then opens the Hub. */
async function openInbox(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  inbox: Partial<Inbox> = {},
  options: OpenHubOptions = {},
): Promise<Inbox> {
  const state: Inbox = {
    items: inbox.items ?? fixtureInbox(),
    feature: inbox.feature ?? null,
    actions: inbox.actions ?? {},
  };
  const unread = () => state.items.filter((i) => !i.read).length;
  ipc.handle("fork_notifications_list", (args) => {
    const items =
      args.filter === "unread"
        ? state.items.filter((i) => !i.read)
        : state.items;
    return { items, unread: unread(), total: state.items.length };
  });
  ipc.handle("fork_notifications_mark_read", (args) => {
    for (const i of state.items) {
      if ((args.ids as number[]).includes(i.id)) i.read = true;
    }
    return unread();
  });
  ipc.handle("fork_notifications_mark_all_read", () => {
    for (const i of state.items) i.read = true;
    return 0;
  });
  ipc.handle("fork_notifications_activate", (args) => {
    const item = state.items.find((i) => i.id === args.id);
    if (!item) return null;
    item.read = true;
    if (args.button === "copy_latest") return { type: "copy_latest" };
    return args.id in state.actions
      ? state.actions[args.id as number]
      : item.action;
  });
  ipc.handle("fork_notifications_feature", () => state.feature);
  await openHub(page, ipc, hub, {
    ...options,
    fork: { notifications: NOTIFICATIONS_ON, ...(options.fork ?? {}) },
  });
  return state;
}

const bell = (page: Page) => page.getByTestId("wb-notifications-bell");
const panel = (page: Page) =>
  page.getByRole("dialog", { name: "Notifications" });

async function openPanel(page: Page) {
  await bell(page).click();
  await expect(panel(page)).toBeVisible();
}

test.describe("notifications", () => {
  test("the bell shows the unread count and the panel lists entries", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInbox(page, ipc, hub);
    await expect(bell(page)).toHaveAttribute(
      "aria-label",
      "Notifications, 2 unread",
    );
    await expect(page.getByTestId("wb-notifications-badge")).toBeVisible();
    await openPanel(page);

    const list = page.getByRole("list", { name: "Notifications" });
    const rows = list.getByRole("listitem");
    await expect(rows).toHaveCount(3);
    // Newest first; plural and grouped numbers from the args.
    await expect(rows.nth(0)).toContainText("3 dictations ready");
    await expect(rows.nth(0)).toContainText("Just now");
    await expect(rows.nth(1)).toContainText("10,000 words dictated");
    await expect(rows.nth(1)).toContainText("3 hours ago");
    await expect(rows.nth(2)).toContainText("Dictation is ready");
    await expect(rows.nth(2)).toHaveAttribute("data-read", "true");
    // Screen readers hear "Unread" first, then the visible text.
    await expect(
      list.getByRole("button", { name: "3 dictations ready" }),
    ).toHaveAccessibleName(
      /^Unread, 3 dictations ready\s*Just now\s*What you said/,
    );
    await expect(
      list.getByRole("button", { name: "Dictation is ready" }),
    ).toHaveAccessibleName(/^Dictation is ready\s*2 days ago/);
    // Buttons only where the entry has them.
    await expect(page.getByTestId("wb-notification-3-copy_latest")).toHaveText(
      "Copy latest",
    );
    await expect(page.getByTestId("wb-notification-3-open")).toHaveText("Open");
    await expect(rows.nth(1).getByRole("button")).toHaveCount(1);
  });

  test("the filter shows unread entries and its own empty state", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInbox(page, ipc, hub);
    await openPanel(page);
    const unread = page.getByTestId("wb-notifications-filter-unread");
    await expect(
      page.getByTestId("wb-notifications-filter-all"),
    ).toHaveAttribute("aria-pressed", "true");
    await unread.click();
    await expect(unread).toHaveAttribute("aria-pressed", "true");
    await ipc.expectCalled("fork_notifications_list", { filter: "unread" });
    await expect(panel(page).getByRole("listitem")).toHaveCount(2);

    // Mark all as read from the ⋯ menu: the unread view empties.
    await page.getByTestId("wb-notifications-more").click();
    await page.getByTestId("wb-menu-item-notifications-mark-all-read").click();
    await ipc.expectCalled("fork_notifications_mark_all_read");
    await expect(page.getByTestId("wb-notifications-empty")).toContainText(
      "You have read everything.",
    );
    await expect(bell(page)).toHaveAttribute("aria-label", "Notifications");
    await expect(page.getByTestId("wb-notifications-badge")).toBeHidden();
  });

  test("an empty inbox says nothing new", async ({ page, ipc, hub }) => {
    await openInbox(page, ipc, hub, { items: [] });
    await openPanel(page);
    await expect(page.getByTestId("wb-notifications-empty")).toContainText(
      "Nothing new",
    );
    await page.getByTestId("wb-notifications-more").click();
    await expect(
      page.getByTestId("wb-menu-item-notifications-mark-all-read"),
    ).toHaveAttribute("data-disabled", "");
  });

  test("a click marks the entry read and opens its page", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInbox(page, ipc, hub);
    await openPanel(page);
    await panel(page)
      .getByRole("button", { name: "10,000 words dictated" })
      .click();
    await ipc.expectCalled("fork_notifications_activate", {
      id: 2,
      button: null,
    });
    await expect(panel(page)).toBeHidden();
    await expect(page.getByTestId("wb-main")).toHaveAttribute(
      "data-page",
      "insights",
    );
    await expect(bell(page)).toHaveAttribute(
      "aria-label",
      "Notifications, 1 unread",
    );
  });

  test("buttons run their own action", async ({ page, ipc, hub }) => {
    await openInbox(page, ipc, hub);
    await openPanel(page);
    await page.getByTestId("wb-notification-3-copy_latest").click();
    await ipc.expectCalled("fork_notifications_activate", {
      id: 3,
      button: "copy_latest",
    });
    // Copying happens in Rust; the panel stays and the page does not change.
    await expect(panel(page)).toBeVisible();
    await expect(page.getByTestId("wb-main")).toHaveAttribute(
      "data-page",
      "dictation",
    );
    await page.getByTestId("wb-notification-3-open").click();
    await ipc.expectCalled("fork_notifications_activate", {
      id: 3,
      button: "open",
    });
    await expect(page).toHaveURL(/#\/hub\/settings\/history$/);
  });

  test("a failed action says so", async ({ page, ipc, hub }) => {
    await openInbox(page, ipc, hub);
    ipc.handle("fork_notifications_activate", () => {
      throw new Error("backend down");
    });
    await openPanel(page);
    await panel(page)
      .getByRole("button", { name: "Dictation is ready" })
      .click();
    await expect(panel(page).getByRole("alert")).toHaveText(
      "That did not work. Try again.",
    );
  });

  test("the toggles write the notifications setting", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInbox(page, ipc, hub);
    await openPanel(page);
    await page.getByTestId("wb-notifications-more").click();
    const menu = page.getByRole("menu", { name: "More notification options" });
    await expect(menu).toBeVisible();
    for (const name of [
      "Suggestions",
      "Announcements",
      "Milestones",
      "Mac notifications",
    ]) {
      await expect(
        menu.getByRole("menuitemcheckbox", { name }),
      ).toHaveAttribute("aria-checked", "true");
    }
    await menu.getByRole("menuitemcheckbox", { name: "Suggestions" }).click();
    // Toggles keep the menu open.
    await expect(menu).toBeVisible();
    await expect(
      menu.getByRole("menuitemcheckbox", { name: "Suggestions" }),
    ).toHaveAttribute("aria-checked", "false");
    await menu.getByRole("menuitemcheckbox", { name: "Milestones" }).click();
    await expect
      .poll(() => hub.forkSettings.notifications)
      .toEqual({ ...NOTIFICATIONS_ON, suggestions: false, milestones: false });
    await page.keyboard.press("Escape");
    await expect(menu).toBeHidden();
  });

  test("new entries refresh the panel and toast while focused; Esc closes toasts", async ({
    page,
    ipc,
    hub,
  }) => {
    const inbox = await openInbox(page, ipc, hub, { items: [] });
    await ipc.waitForListener("fork-notifications-changed");
    const posted = entry({
      id: 9,
      kind: "update_ready",
      title_key: "rust.notify.updateReady.title",
      body_key: "rust.notify.updateReady.body",
      args: { version: "1.0.1" },
      created_at: Date.now(),
    });
    inbox.items.unshift(posted);
    await ipc.emit("fork-notifications-changed", { unread: 1, posted });
    const toast = page.locator("[data-sonner-toast]");
    await expect(toast).toContainText("Update ready");
    await expect(toast).toContainText(
      "Wispr Bro 1.0.1 installs the next time the app restarts.",
    );
    await expect(bell(page)).toHaveAttribute(
      "aria-label",
      "Notifications, 1 unread",
    );
    await page.keyboard.press("Escape");
    await expect(toast).toHaveCount(0);

    // Esc seen by the hotkey tap (another app in front) closes them too.
    await ipc.emit("fork-notifications-changed", { unread: 1, posted });
    await expect(toast).toHaveCount(1);
    await ipc.emit("fork-toasts-dismiss", { cancelled_take: false });
    await expect(toast).toHaveCount(0);

    // The toast's button activates the entry.
    await ipc.emit("fork-notifications-changed", { unread: 1, posted });
    await toast.getByRole("button", { name: "Open" }).click();
    await ipc.expectCalled("fork_notifications_activate", {
      id: 9,
      button: null,
    });
    await expect(page.getByTestId("wb-main")).toHaveAttribute(
      "data-page",
      "dictation",
    );
  });

  test("a macOS click opens the right page or the panel", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInbox(page, ipc, hub);
    await ipc.waitForListener("fork-notification-opened");
    await ipc.emit("fork-notification-opened", {
      id: 3,
      action: { type: "open_page", page: "settings", section: "history" },
    });
    await expect(page).toHaveURL(/#\/hub\/settings\/history$/);
    await expect(page.getByRole("tab", { name: "History" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await ipc.emit("fork-notification-opened", {
      id: null,
      action: { type: "open_notifications" },
    });
    await expect(panel(page)).toBeVisible();
  });

  test("macOS Open reveals History without completing unfinished onboarding", async ({
    page,
    ipc,
    hub,
  }) => {
    const hubOpened = openInbox(
      page,
      ipc,
      hub,
      {},
      { handy: { onboarding_completed: false } },
    );
    await expect(
      page.getByRole("heading", {
        name: "Speak instead of typing, in any app",
      }),
    ).toBeVisible();
    await ipc.waitForListener("fork-notification-opened");
    await ipc.emit("fork-notification-opened", {
      id: 3,
      action: { type: "open_page", page: "settings", section: "history" },
    });
    await hubOpened;

    await expect(page).toHaveURL(/#\/hub\/settings\/history$/);
    await expect(page.getByTestId("wb-main")).toHaveAttribute(
      "data-page",
      "settings",
    );
    await expect(page.getByRole("tab", { name: "History" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(hub.handy.onboarding_completed).toBe(false);
    expect(await ipc.calls("complete_onboarding")).toHaveLength(0);
  });

  test.describe("feature popup", () => {
    const feature = {
      id: "inbox",
      dismiss_id: "feature.inbox",
      title_key: "rust.notify.features.inbox.title",
      body_key: "rust.notify.features.inbox.body",
      action: { type: "open_notifications" },
      entry_id: 7,
    };

    test("shows once and remembers the dismissal", async ({
      page,
      ipc,
      hub,
    }) => {
      await openInbox(page, ipc, hub, {
        feature,
        items: [entry({ id: 7, kind: "announcement" })],
      });
      const dialog = page.getByRole("dialog", {
        name: "Your notifications live here",
      });
      await expect(dialog).toBeVisible();
      await expect(dialog).toContainText("New in this version");
      await page.getByTestId("wb-feature-popup-dismiss").click();
      await expect(dialog).toBeHidden();
      await expect
        .poll(() => hub.forkSettings.dismissed_banners)
        .toEqual(["feature.inbox"]);
      await ipc.expectCalled("fork_notifications_mark_read", { ids: [7] });
      await page.reload();
      await expect(page.getByTestId("wb-sidebar")).toBeVisible();
      await expect(dialog).toBeHidden();
    });

    test("Show me opens what the feature points at", async ({
      page,
      ipc,
      hub,
    }) => {
      await openInbox(page, ipc, hub, { feature });
      await page.getByTestId("wb-feature-popup-open").click();
      await expect(panel(page)).toBeVisible();
      await expect
        .poll(() => hub.forkSettings.dismissed_banners)
        .toEqual(["feature.inbox"]);
    });

    test("Esc closes it like Got it", async ({ page, ipc, hub }) => {
      await openInbox(page, ipc, hub, { feature });
      await expect(page.getByTestId("wb-feature-popup")).toBeVisible();
      await page.keyboard.press("Escape");
      await expect(page.getByTestId("wb-feature-popup")).toBeHidden();
      await expect
        .poll(() => hub.forkSettings.dismissed_banners)
        .toEqual(["feature.inbox"]);
    });

    test("stays away once dismissed or when there is none", async ({
      page,
      ipc,
      hub,
    }) => {
      await openInbox(
        page,
        ipc,
        hub,
        { feature },
        { fork: { dismissed_banners: ["feature.inbox"] } },
      );
      await ipc.expectCalled("fork_notifications_feature");
      await expect(page.getByTestId("wb-feature-popup")).toBeHidden();
    });
  });

  test("works from the keyboard", async ({ page, ipc, hub }) => {
    await openInbox(page, ipc, hub);
    await bell(page).focus();
    await page.keyboard.press("Enter");
    await expect(panel(page)).toBeVisible();
    // Focus moves into the panel: ⋯, the filter, then the entries.
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("wb-notifications-filter-all")).toBeFocused();
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(
      panel(page).getByRole("button", { name: "3 dictations ready" }),
    ).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(
      page.getByTestId("wb-notification-3-copy_latest"),
    ).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(panel(page)).toBeHidden();
    await expect(bell(page)).toBeFocused();
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe: panel, menu and popup (${theme})`, async ({
      page,
      ipc,
      hub,
    }) => {
      await openInbox(page, ipc, hub, {}, { handy: { theme } });
      await openPanel(page);
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-notifications-more").click();
      await expect(page.getByTestId("wb-notifications-menu")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.keyboard.press("Escape");
      await page.keyboard.press("Escape");

      await page.reload();
      await openInbox(page, ipc, hub, {
        feature: {
          id: "inbox",
          dismiss_id: "feature.popup-axe",
          title_key: "rust.notify.features.inbox.title",
          body_key: "rust.notify.features.inbox.body",
          action: { type: "open_notifications" },
          entry_id: null,
        },
      });
      await expect(page.getByTestId("wb-feature-popup")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }
});
