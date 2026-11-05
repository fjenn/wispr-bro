// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hub sidebar (S-13, C-01, C-14, C-15, C-210).
 *
 * Feature pages on top, then the setup-checklist slot (S-30), then Share,
 * Support (OT2-E: its own menu, `SupportMenu`), Settings and Help at the
 * bottom (the Help and Feedback popover, S-26b). Items are buttons; the current page has
 * `aria-current="page"`. Collapsed, the sidebar shows icons only, each with
 * its name as the accessible label and a tooltip.
 *
 * Design Phase 0: flush with the window chrome (no divider), 216 pt wide,
 * 15 px medium labels in ink with 18 pt icons on a 40 pt pitch; the current
 * page gets a barely-there warm fill, never a colour or a bolder label.
 * The traffic lights and the collapse toggle live in the title strip above;
 * the wordmark (glyph only when collapsed) heads the sidebar (Phase 0b).
 */
import { forwardRef, type ReactNode } from "react";
import { CircleHelp, HandHeart } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Badge, FOCUS_RING, Tag, Tooltip, Wordmark } from "../ui";
import {
  useSuggestedCount,
  useSuggestedCountLive,
} from "../dictionary/useDictionary";
import { HelpPopover } from "./HelpPopover";
import { SupportMenu } from "./SupportMenu";
import { navigate, useHubNav } from "./nav";
import { HUB_PAGES, newTagId, pageLabelKey, type HubPage } from "./pages";

/** Content for the setup checklist card (S-30); null renders nothing. */
let sidebarCard: (() => ReactNode) | null = null;

/** S-30 mounts the setup checklist card here. */
export function setSidebarCard(render: (() => ReactNode) | null): void {
  sidebarCard = render;
}

const itemClass = (active: boolean, collapsed: boolean) =>
  `group flex cursor-pointer items-center gap-3 rounded-(--wb-radius-sm) text-[15px] font-medium text-(--wb-ink) transition-colors duration-(--wb-dur-fast) ${
    collapsed ? "mx-auto size-10 justify-center" : "h-9 w-full px-2.5"
  } ${
    active
      ? "bg-(--wb-fill-active)"
      : "hover:bg-(--wb-fill-hover) active:bg-(--wb-fill-press)"
  } ${FOCUS_RING}`;

interface NavButtonProps {
  label: string;
  icon: HubPage["icon"];
  active?: boolean;
  collapsed: boolean;
  tag?: ReactNode;
  /** Collapsed: a dot on the icon (something waits on that page). */
  dot?: boolean;
  testId: string;
  onClick?: () => void;
}

/** A sidebar button; forwards its ref so it can be a menu trigger. */
const NavButton = forwardRef<HTMLButtonElement, NavButtonProps>(
  function NavButton(
    {
      label,
      icon: Icon,
      active = false,
      collapsed,
      tag,
      dot = false,
      testId,
      ...rest
    },
    ref,
  ) {
    const button = (
      <button
        ref={ref}
        type="button"
        data-testid={testId}
        aria-current={active ? "page" : undefined}
        aria-label={collapsed ? label : undefined}
        className={itemClass(active, collapsed)}
        {...rest}
      >
        {collapsed && dot ? (
          <span className="relative inline-flex shrink-0">
            <Icon size={18} />
            <span
              data-testid={`${testId}-dot`}
              className="absolute -end-0.5 -top-0.5 size-2 rounded-full bg-(--wb-accent) ring-2 ring-(--wb-bg)"
            />
          </span>
        ) : (
          <Icon size={18} className="shrink-0" />
        )}
        {!collapsed && (
          <>
            <span className="min-w-0 flex-1 truncate text-start">{label}</span>
            {tag}
          </>
        )}
      </button>
    );
    return button;
  },
);

function PageButton({
  page,
  collapsed,
}: {
  page: HubPage;
  collapsed: boolean;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const active = useHubNav((s) => s.page === page.id);
  const name = t(pageLabelKey(page.id));
  // S-34: words auto-learn suggested wait on the Dictionary page.
  const waiting = page.id === "dictionary" ? <SuggestedBadge /> : null;
  const suggested = useSuggestedText(page.id === "dictionary");
  const label = suggested && collapsed ? `${name}, ${suggested}` : name;
  const button = (
    <NavButton
      testId={`wb-nav-${page.id}`}
      label={label}
      icon={page.icon}
      active={active}
      collapsed={collapsed}
      dot={!!suggested}
      onClick={() => navigate(page.id)}
      tag={
        waiting && suggested ? (
          waiting
        ) : page.tag ? (
          <Tag
            kind={page.tag}
            seenId={page.tag === "new" ? newTagId(page.id) : undefined}
          />
        ) : undefined
      }
    />
  );
  return collapsed ? (
    <Tooltip text={label} side="right">
      {button}
    </Tooltip>
  ) : (
    button
  );
}

/** "Suggested words: 2" while auto-learn suggestions wait, else null. */
function useSuggestedText(enabled: boolean): string | null {
  const { t } = useTranslation(FORK_NAMESPACE);
  const count = useSuggestedCount();
  return enabled && count > 0 ? t("hub.sidebar.suggested", { count }) : null;
}

/** The waiting suggestions' count beside "Dictionary". */
function SuggestedBadge() {
  const count = useSuggestedCount();
  const text = useSuggestedText(true);
  if (!text) return null;
  return (
    <span title={text} aria-label={text} role="img">
      <Badge tone="accent" testId="wb-nav-dictionary-badge">
        {count > 99 ? "99+" : count}
      </Badge>
    </span>
  );
}

function HelpMenu({ collapsed }: { collapsed: boolean }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <HelpPopover
      trigger={
        <NavButton
          testId="wb-nav-help"
          label={t("hub.help.title")}
          icon={CircleHelp}
          collapsed={collapsed}
        />
      }
    />
  );
}

function SupportItem({ collapsed }: { collapsed: boolean }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const label = t("hub.support.nav", { appName: t("rust.appName") });
  return (
    <SupportMenu
      trigger={
        <NavButton
          testId="wb-nav-support"
          label={label}
          icon={HandHeart}
          collapsed={collapsed}
        />
      }
    />
  );
}

export function Sidebar({ collapsed }: { collapsed: boolean }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const main = HUB_PAGES.filter((p) => p.group === "main");
  const bottom = HUB_PAGES.filter((p) => p.group === "bottom");
  const card = sidebarCard?.();
  useSuggestedCountLive();

  return (
    <aside
      id="wb-sidebar"
      data-testid="wb-sidebar"
      data-collapsed={collapsed || undefined}
      className={`flex h-full shrink-0 flex-col bg-(--wb-chrome) transition-[width] duration-(--wb-dur-base) ease-(--wb-ease-out) motion-reduce:transition-none ${
        collapsed
          ? "w-(--wb-sidebar-collapsed-width)"
          : "w-(--wb-sidebar-width)"
      }`}
    >
      <div
        data-tauri-drag-region
        className={`flex h-14 shrink-0 items-center pb-2 ${collapsed ? "justify-center" : "px-5"}`}
      >
        <Wordmark compact={collapsed} />
      </div>
      <nav
        aria-label={t("hub.nav.label")}
        className="flex min-h-0 flex-1 flex-col gap-3 px-3 pb-2.5"
      >
        {/* Only the pages and the setup card scroll: Share, Settings and
            Help stay pinned at the bottom (Mac wave 6: at the default
            752 pt window with the checklist open, they had scrolled out
            of view). */}
        <div
          className="-mx-3 flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3"
          data-testid="wb-sidebar-scroll"
        >
          <ul className="flex flex-col gap-1">
            {main.map((page) => (
              <li key={page.id}>
                <PageButton page={page} collapsed={collapsed} />
              </li>
            ))}
          </ul>
          <div className="flex-1" />
          {card && !collapsed && (
            <div data-testid="wb-sidebar-card">{card}</div>
          )}
        </div>
        <ul className="flex shrink-0 flex-col gap-1 border-t border-(--wb-line) pt-3">
          {bottom.map((page) => [
            <li key={page.id}>
              <PageButton page={page} collapsed={collapsed} />
            </li>,
            page.id === "share" && (
              <li key="support">
                <SupportItem collapsed={collapsed} />
              </li>
            ),
          ])}
          <li>
            <HelpMenu collapsed={collapsed} />
          </li>
        </ul>
      </nav>
    </aside>
  );
}
