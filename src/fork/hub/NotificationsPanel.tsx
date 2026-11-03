// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The notifications bell and its panel (S-18, C-17, C-201, C-91).
 *
 *   ┌ Notifications ─────────────── ⋯ ┐   ⋯ = Mark all as read,
 *   │ [All] [Unread]                  │       Suggestions / Announcements /
 *   │ ● Dictation is ready     2 min  │       Milestones, Mac notifications
 *   │   The speech model finished…    │
 *   │   [Copy latest] [Open]          │   buttons only where an entry has
 *   │   …                             │   them (queued takes, shortcut)
 *   └─────────────────────────────────┘
 *
 * The bell shows an unread dot and says the count to screen readers. A click
 * on an entry marks it read and runs its action (`activateNotification`).
 * Toggles write the fork setting `notifications` and filter new entries at
 * creation (Rust). `useNotificationsLive` (mounted once by `ForkApp`) keeps
 * the store live, shows each new entry as a toast while the window has focus
 * and closes toasts on Esc, from this window or from the hotkey tap (observed,
 * never blocked). `useNotificationPageOpen` handles macOS clicks even before
 * the Hub mounts.
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Bell, BellOff, CheckCheck, MoreHorizontal } from "../ui/icons";
import { toast } from "sonner";
import {
  events,
  type NotificationFilter,
  type NotificationItem,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { listenForTransformChanges } from "../transforms/store";
import { listenForMenuActions } from "./menuActions";
import { reportHubReady } from "./ready";
import { EmptyState, FOCUS_RING, Menu, Popover } from "../ui";
import {
  activateNotification,
  buttonLabel,
  markAllRead,
  notificationText,
  relativeTime,
  runHubAction,
  useInbox,
} from "./notifications";

const iconButton = `relative grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink-2) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`;

type ToggleKey = "suggestions" | "announcements" | "milestones" | "system";

function useNow(intervalMs = 30_000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now;
}

function MoreMenu() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  const unread = useInbox((s) => s.unread);
  const current = settings?.notifications;

  const toggle = (key: ToggleKey) => {
    if (!current) return;
    void setSetting("notifications", {
      ...current,
      [key]: !(current[key] ?? true),
    });
  };
  const item = (key: ToggleKey) => ({
    id: `notifications-${key}`,
    label: t(`hub.notifications.toggles.${key}`),
    checked: current?.[key] ?? true,
    disabled: !current,
    onSelect: () => toggle(key),
  });
  const label = t("hub.notifications.more");

  return (
    <Menu
      label={label}
      align="end"
      testId="wb-notifications-menu"
      trigger={
        <button
          type="button"
          data-testid="wb-notifications-more"
          aria-label={label}
          title={label}
          className={iconButton}
        >
          <MoreHorizontal size={18} aria-hidden="true" />
        </button>
      }
      groups={[
        {
          items: [
            {
              id: "notifications-mark-all-read",
              label: t("hub.notifications.markAllRead"),
              icon: CheckCheck,
              disabled: unread === 0,
              onSelect: () => void markAllRead(),
            },
          ],
        },
        {
          label: t("hub.notifications.groups.show"),
          items: [
            item("suggestions"),
            item("announcements"),
            item("milestones"),
          ],
        },
        {
          label: t("hub.notifications.groups.mac"),
          items: [item("system")],
        },
      ]}
    />
  );
}

function FilterSwitch() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const filter = useInbox((s) => s.filter);
  const setFilter = useInbox((s) => s.setFilter);
  const options: NotificationFilter[] = ["all", "unread"];
  return (
    <div
      role="group"
      aria-label={t("hub.notifications.filter.label")}
      className="flex gap-1 px-4 pb-2"
    >
      {options.map((option) => (
        <button
          key={option}
          type="button"
          data-testid={`wb-notifications-filter-${option}`}
          aria-pressed={filter === option}
          onClick={() => setFilter(option)}
          className={`cursor-pointer rounded-full px-3 py-1 text-[12.5px] font-medium transition-colors ${FOCUS_RING} ${
            filter === option
              ? "bg-(--wb-selected) text-(--wb-ink)"
              : "text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink)"
          }`}
        >
          {t(`hub.notifications.filter.${option}`)}
        </button>
      ))}
    </div>
  );
}

function Entry({ item, now }: { item: NotificationItem; now: number }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { title, body } = notificationText(item, t, i18n.language);
  const when = relativeTime(item.created_at, now, t, i18n.language);
  const [failed, setFailed] = useState(false);
  const run = async (button: NotificationItem["buttons"][number] | null) => {
    setFailed(!(await activateNotification(item.id, button)));
  };
  const titleId = `wb-notification-${item.id}-title`;

  return (
    <li
      data-testid={`wb-notification-${item.id}`}
      data-kind={item.kind}
      data-read={item.read ? "true" : "false"}
      className="border-b border-(--wb-line) last:border-b-0"
    >
      {/* The button's name is its visible text (title, time, body), with
          "Unread" spoken first for unread entries. */}
      <button
        type="button"
        onClick={() => void run(null)}
        className={`flex w-full cursor-pointer gap-2.5 px-4 pt-3 text-start hover:bg-(--wb-hover) ${item.buttons.length ? "pb-1.5" : "pb-3"} ${FOCUS_RING} focus-visible:-outline-offset-2`}
      >
        <span
          aria-hidden="true"
          className={`mt-1.5 size-2 shrink-0 rounded-full ${item.read ? "bg-transparent" : "bg-(--wb-accent)"}`}
        />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="flex items-baseline gap-2">
            <span
              id={titleId}
              className={`min-w-0 flex-1 text-[13px] leading-5 ${item.read ? "font-medium text-(--wb-ink-2)" : "font-semibold text-(--wb-ink)"}`}
            >
              {!item.read && (
                <span className="sr-only">
                  {t("hub.notifications.unreadDot")},{" "}
                </span>
              )}
              {title}
            </span>
            <span className="shrink-0 text-[11.5px] text-(--wb-ink-3)">
              {when}
            </span>
          </span>
          {body && (
            <span className="text-[12.5px] leading-[18px] text-(--wb-ink-2)">
              {body}
            </span>
          )}
        </span>
      </button>
      {item.buttons.length > 0 && (
        <div className="flex flex-wrap gap-2 ps-[34px] pe-4 pb-3">
          {item.buttons.map((button) => (
            <button
              key={button}
              type="button"
              data-testid={`wb-notification-${item.id}-${button}`}
              onClick={() => void run(button)}
              className={`cursor-pointer rounded-(--wb-radius-control) border border-(--wb-line-strong) bg-(--wb-card) px-2.5 py-1 text-[12px] font-medium text-(--wb-ink) hover:bg-(--wb-hover) ${FOCUS_RING}`}
            >
              {buttonLabel(button, t)}
            </button>
          ))}
        </div>
      )}
      {failed && (
        <p role="alert" className="px-4 pb-3 text-[12px] text-(--wb-ink)">
          {t("hub.notifications.actionFailed")}
        </p>
      )}
    </li>
  );
}

function PanelBody() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const items = useInbox((s) => s.items);
  const status = useInbox((s) => s.status);
  const filter = useInbox((s) => s.filter);
  const now = useNow();

  if (status === "error") {
    return (
      <p role="alert" className="px-4 py-6 text-[12.5px] text-(--wb-ink-2)">
        {t("hub.notifications.loadFailed")}
      </p>
    );
  }
  if (items.length === 0) {
    const key = filter === "unread" ? "emptyUnread" : "empty";
    return (
      <EmptyState
        compact
        testId="wb-notifications-empty"
        icon={BellOff}
        title={t(`hub.notifications.${key}.title`)}
        body={t(`hub.notifications.${key}.body`)}
      />
    );
  }
  return (
    <ul
      aria-label={t("hub.notifications.list")}
      data-testid="wb-notifications-list"
      className="max-h-[min(440px,calc(100vh-140px))] overflow-y-auto"
    >
      {items.map((item) => (
        <Entry key={item.id} item={item} now={now} />
      ))}
    </ul>
  );
}

/** The header's bell with its panel. */
export function NotificationsBell() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const open = useInbox((s) => s.open);
  const setOpen = useInbox((s) => s.setOpen);
  const unread = useInbox((s) => s.unread);
  const title = t("hub.notifications.title");
  const bellLabel =
    unread > 0 ? t("hub.notifications.bellUnread", { count: unread }) : title;

  return (
    <Popover
      label={title}
      testId="wb-notifications-panel"
      className="w-[360px]"
      open={open}
      onOpenChange={setOpen}
      trigger={
        <button
          type="button"
          data-testid="wb-notifications-bell"
          data-unread={unread}
          aria-label={bellLabel}
          title={bellLabel}
          className={iconButton}
        >
          <Bell size={20} />
          {unread > 0 && (
            <span
              aria-hidden="true"
              data-testid="wb-notifications-badge"
              className="absolute end-1 top-1 size-2 rounded-full bg-(--wb-accent) ring-2 ring-(--wb-chrome)"
            />
          )}
        </button>
      }
    >
      <div className="flex items-center justify-between gap-2 py-2 ps-4 pe-2">
        <h2 className="text-[13.5px] font-semibold">{title}</h2>
        <MoreMenu />
      </div>
      <FilterSwitch />
      <div className="border-t border-(--wb-line)">
        <PanelBody />
      </div>
    </Popover>
  );
}

/** Shows `item` as a toast with its first action. */
function showToast(
  item: NotificationItem,
  t: ReturnType<typeof useTranslation>["t"],
  language: string,
) {
  const { title, body } = notificationText(item, t, language);
  const button = item.buttons[0] ?? null;
  toast(title, {
    id: `wb-notification-${item.id}`,
    description: body ?? undefined,
    action: {
      label: button ? buttonLabel(button, t) : t("rust.notify.buttons.open"),
      onClick: () => void activateNotification(item.id, button),
    },
  });
}

/** Handles native Open before onboarding's render gate can mount the Hub. */
export function useNotificationPageOpen(): boolean {
  const [opened, setOpened] = useState(false);

  useEffect(() => {
    const stop = events.forkNotificationOpened.listen((event) => {
      const action = event.payload.action;
      toast.dismiss();
      runHubAction(action);
      if (action?.type === "open_page") setOpened(true);
    });
    // S-38: events for a rebuilt Hub wait until these listeners are up.
    void reportHubReady([
      stop,
      listenForTransformChanges(),
      listenForMenuActions(),
    ]);
    return () => {
      void stop
        .then((unlisten) => unlisten?.())
        .catch(() => {
          // Listening failed (logged by the event plugin); nothing to stop.
        });
    };
  }, []);

  return opened;
}

/**
 * Keeps the inbox live for the Hub (mount once): loads it, refreshes it on
 * `ForkNotificationsChanged`, toasts new entries while this window has focus
 * and closes toasts on Esc.
 */
export function useNotificationsLive(): void {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);

  useEffect(() => {
    void useInbox.getState().refresh();
  }, []);

  useEffect(() => {
    const stops = [
      events.forkNotificationsChanged.listen((event) => {
        const { unread, posted } = event.payload;
        useInbox.getState().setUnread(unread);
        void useInbox.getState().refresh();
        if (posted && document.hasFocus()) showToast(posted, t, i18n.language);
      }),
      events.forkToastsDismiss.listen(() => toast.dismiss()),
    ];
    // Esc in this window closes toasts too; it is never stopped or
    // prevented, so dialogs and popovers still see it.
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") toast.dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      for (const stop of stops) {
        void stop
          .then((unlisten) => unlisten?.())
          .catch(() => {
            // Listening failed (logged by the event plugin); nothing to stop.
          });
      }
    };
  }, [t, i18n.language]);
}
