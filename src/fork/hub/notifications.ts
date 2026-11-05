// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Notifications inbox state for the Hub (S-18, C-17, C-201).
 *
 * The Rust side (`fork/notify.rs`, `fork.db` › `notifications`) is the
 * source of truth. This store lists it (`fork_notifications_list`), keeps the
 * unread count for the bell, and follows `ForkNotificationsChanged`. Entry
 * text is `fork.json` keys (`rust.notify.*`) plus string args, translated
 * here so the inbox follows the app language.
 *
 * Activation (`activateNotification`) goes through
 * `fork_notifications_activate`: Rust marks the entry read and runs its side
 * of the action (copy, System Settings, Finder); the Hub runs the rest
 * (navigate, open this panel).
 */
import type { TFunction } from "i18next";
import { create } from "zustand";
import {
  commands,
  type NotificationAction,
  type NotificationButton,
  type NotificationFilter,
  type NotificationItem,
} from "../bindings";
import { isHubPageId, navigate } from "./nav";

export type InboxStatus = "idle" | "loading" | "ready" | "error";

export interface InboxState {
  items: NotificationItem[];
  unread: number;
  total: number;
  filter: NotificationFilter;
  status: InboxStatus;
  /** The bell's panel is open. */
  open: boolean;
  setOpen: (open: boolean) => void;
  setFilter: (filter: NotificationFilter) => void;
  refresh: () => Promise<void>;
  setUnread: (unread: number) => void;
}

/** Entries one list call shows. */
export const INBOX_LIMIT = 100;

let refreshing: Promise<void> | null = null;
let refreshAgain = false;

export const useInbox = create<InboxState>((set, get) => ({
  items: [],
  unread: 0,
  total: 0,
  filter: "all",
  status: "idle",
  open: false,
  setOpen: (open) => {
    set({ open });
    if (open) void get().refresh();
  },
  setFilter: (filter) => {
    set({ filter });
    void get().refresh();
  },
  setUnread: (unread) => set({ unread }),
  refresh: async () => {
    // Overlapping triggers (events in a burst) collapse into one extra read.
    if (refreshing) {
      refreshAgain = true;
      return refreshing;
    }
    refreshing = (async () => {
      do {
        refreshAgain = false;
        if (get().status === "idle") set({ status: "loading" });
        try {
          const result = await commands.forkNotificationsList(
            get().filter,
            INBOX_LIMIT,
          );
          if (result.status === "ok") {
            // A backend without the command (or not ready) lists nothing.
            const page = result.data ?? { items: [], unread: 0, total: 0 };
            set({
              items: page.items ?? [],
              unread: page.unread ?? 0,
              total: page.total ?? 0,
              status: "ready",
            });
          } else if (result.error.kind === "NotReady") {
            set({ status: "ready" });
          } else {
            set({ status: "error" });
          }
        } catch {
          set({ status: "error" });
        }
      } while (refreshAgain);
    })();
    try {
      await refreshing;
    } finally {
      refreshing = null;
    }
  },
}));

/** Test hook: back to a fresh store. */
export function resetInbox(): void {
  refreshing = null;
  refreshAgain = false;
  useInbox.setState({
    items: [],
    unread: 0,
    total: 0,
    filter: "all",
    status: "idle",
    open: false,
  });
}

const PLURAL_SUFFIX = /_(zero|one|two|few|many|other)$/;
const DIGITS = /^\d+$/;

/** The interpolation values of an entry: numbers grouped per language. */
function interpolation(
  item: NotificationItem,
  t: TFunction,
  language: string,
): Record<string, string | number> {
  const format = new Intl.NumberFormat(language);
  const values: Record<string, string | number> = {
    appName: t("rust.appName"),
  };
  for (const [name, value] of Object.entries(item.args)) {
    if (value === undefined) continue;
    values[name] = DIGITS.test(value) ? format.format(Number(value)) : value;
  }
  return values;
}

/** Translates one key of an entry. A plural key Rust picked (`_one` /
 * `_other`, English rules) is resolved again with the current language's
 * CLDR rules; `{{count}}` keeps the grouped number ("10,000"), which
 * i18next's own `count` option would replace with the raw one. */
function translate(
  key: string,
  item: NotificationItem,
  t: TFunction,
  language: string,
): string {
  const values = interpolation(item, t, language);
  const count = item.args.count;
  if (PLURAL_SUFFIX.test(key) && count !== undefined && DIGITS.test(count)) {
    const base = key.replace(PLURAL_SUFFIX, "");
    const category = new Intl.PluralRules(language).select(Number(count));
    const form = t(`${base}_${category}`, { ...values, defaultValue: "" });
    return form || t(`${base}_other`, values);
  }
  return t(key, values);
}

export interface NotificationText {
  title: string;
  body: string | null;
}

export function notificationText(
  item: NotificationItem,
  t: TFunction,
  language: string,
): NotificationText {
  return {
    title: translate(item.title_key, item, t, language),
    body: item.body_key ? translate(item.body_key, item, t, language) : null,
  };
}

/** Label of a button. */
export function buttonLabel(button: NotificationButton, t: TFunction): string {
  const keys: Record<NotificationButton, string> = {
    copy_latest: "rust.notify.buttons.copyLatest",
    open: "rust.notify.buttons.open",
    open_accessibility: "rust.notify.buttons.openAccessibility",
  };
  return t(keys[button]);
}

/** "Just now", "5 minutes ago", "Yesterday", … in the app language. */
export function relativeTime(
  createdAt: number,
  now: number,
  t: TFunction,
  language: string,
): string {
  const seconds = Math.round((createdAt - now) / 1000);
  if (Math.abs(seconds) < 60) return t("hub.notifications.justNow");
  const rtf = new Intl.RelativeTimeFormat(language, { numeric: "auto" });
  const minutes = Math.round(seconds / 60);
  if (Math.abs(minutes) < 60) return rtf.format(minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return rtf.format(hours, "hour");
  const days = Math.round(hours / 24);
  if (Math.abs(days) < 7) return rtf.format(days, "day");
  return new Intl.DateTimeFormat(language, {
    month: "short",
    day: "numeric",
  }).format(new Date(createdAt));
}

/** Runs the Hub part of an action. */
export function runHubAction(action: NotificationAction | null): void {
  if (!action) return;
  switch (action.type) {
    case "open_page":
      if (isHubPageId(action.page)) {
        useInbox.getState().setOpen(false);
        navigate(action.page, action.section);
      }
      return;
    case "open_notifications":
      useInbox.getState().setOpen(true);
      return;
    default:
      // copy_latest, open_accessibility_settings, reveal_crash_report: done
      // by Rust.
      return;
  }
}

/** Activates an entry (a click, or one of its buttons). Resolves `false`
 * when the backend refused. */
export async function activateNotification(
  id: number,
  button: NotificationButton | null = null,
): Promise<boolean> {
  try {
    const result = await commands.forkNotificationsActivate(id, button);
    if (result.status !== "ok") return false;
    runHubAction(result.data);
    void useInbox.getState().refresh();
    return true;
  } catch {
    return false;
  }
}

/** Marks every entry read. */
export async function markAllRead(): Promise<void> {
  try {
    const result = await commands.forkNotificationsMarkAllRead();
    if (result.status === "ok") {
      useInbox.getState().setUnread(result.data ?? 0);
    }
  } finally {
    void useInbox.getState().refresh();
  }
}
