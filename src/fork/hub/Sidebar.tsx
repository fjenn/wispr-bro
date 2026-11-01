// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hub sidebar (S-13, C-01, C-14, C-15, C-210).
 *
 * Feature pages on top, then the setup-checklist slot (S-30), then Share,
 * Settings and Help at the bottom. Items are buttons; the current page has
 * `aria-current="page"`. Collapsed, the sidebar shows icons only, each with
 * its name as the accessible label and a tooltip.
 *
 * The top strip is empty on purpose: the macOS traffic lights and the
 * collapse button (rendered by `ForkApp`) sit there, and it drags the window.
 */
import { forwardRef, type ReactNode } from "react";
import { CircleHelp, History, Keyboard, Languages } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Tooltip } from "@/components/ui/Tooltip";
import { FORK_NAMESPACE } from "../i18n";
import { FOCUS_RING, Menu, Tag } from "../ui";
import { navigate, useHubNav, type HubPageId } from "./nav";
import { HUB_PAGES, newTagId, pageLabelKey, type HubPage } from "./pages";

/** Content for the setup checklist card (S-30); null renders nothing. */
let sidebarCard: (() => ReactNode) | null = null;

/** S-30 mounts the setup checklist card here. */
export function setSidebarCard(render: (() => ReactNode) | null): void {
  sidebarCard = render;
}

const itemClass = (active: boolean, collapsed: boolean) =>
  `group flex cursor-pointer items-center gap-2.5 rounded-(--wb-radius-control) text-[13.5px] transition-colors ${
    collapsed ? "mx-auto size-10 justify-center" : "w-full px-2.5 py-[7px]"
  } ${
    active
      ? "bg-(--wb-selected) font-semibold text-(--wb-ink)"
      : "font-medium text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink)"
  } ${FOCUS_RING}`;

interface NavButtonProps {
  label: string;
  icon: HubPage["icon"];
  active?: boolean;
  collapsed: boolean;
  tag?: ReactNode;
  testId: string;
  onClick?: () => void;
}

/** A sidebar button; forwards its ref so it can be a menu trigger. */
const NavButton = forwardRef<HTMLButtonElement, NavButtonProps>(
  function NavButton(
    { label, icon: Icon, active = false, collapsed, tag, testId, ...rest },
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
        <Icon
          size={18}
          strokeWidth={active ? 2.1 : 1.8}
          aria-hidden="true"
          className="shrink-0"
        />
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
  const label = t(pageLabelKey(page.id));
  const button = (
    <NavButton
      testId={`wb-nav-${page.id}`}
      label={label}
      icon={page.icon}
      active={active}
      collapsed={collapsed}
      onClick={() => navigate(page.id)}
      tag={
        page.tag ? (
          <Tag
            kind={page.tag}
            seenId={page.tag === "new" ? newTagId(page.id) : undefined}
          />
        ) : undefined
      }
    />
  );
  return collapsed ? (
    <Tooltip text={label} placement="below" className="w-full justify-center">
      {button}
    </Tooltip>
  ) : (
    button
  );
}

function HelpMenu({ collapsed }: { collapsed: boolean }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const label = t("hub.help.title");
  const go = (page: HubPageId, section?: string) => () =>
    navigate(page, section);
  return (
    <Menu
      label={label}
      side="right"
      align="end"
      testId="wb-help-menu"
      groups={[
        {
          label: t("hub.help.essentials"),
          items: [
            {
              id: "shortcuts",
              label: t("hub.help.shortcuts"),
              icon: Keyboard,
              onSelect: go("settings", "hotkeys"),
            },
            {
              id: "models",
              label: t("hub.help.models"),
              icon: Languages,
              onSelect: go("settings", "models"),
            },
            {
              id: "history",
              label: t("hub.help.history"),
              icon: History,
              onSelect: go("dictation", "history"),
            },
          ],
        },
      ]}
      trigger={
        <NavButton
          testId="wb-nav-help"
          label={label}
          icon={CircleHelp}
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

  return (
    <aside
      id="wb-sidebar"
      data-testid="wb-sidebar"
      data-collapsed={collapsed || undefined}
      className={`flex h-full shrink-0 flex-col bg-(--wb-sidebar) transition-[width] duration-200 motion-reduce:transition-none ${
        collapsed
          ? "w-(--wb-sidebar-collapsed-width)"
          : "w-(--wb-sidebar-width)"
      }`}
    >
      <div
        data-tauri-drag-region
        className="h-[calc(var(--wb-titlebar-height)+12px)] shrink-0"
      />
      <nav
        aria-label={t("hub.nav.label")}
        className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3 pb-3"
      >
        <ul className="flex flex-col gap-0.5">
          {main.map((page) => (
            <li key={page.id}>
              <PageButton page={page} collapsed={collapsed} />
            </li>
          ))}
        </ul>
        <div className="flex-1" />
        {card && !collapsed && <div data-testid="wb-sidebar-card">{card}</div>}
        <ul className="flex flex-col gap-0.5">
          {bottom.map((page) => (
            <li key={page.id}>
              <PageButton page={page} collapsed={collapsed} />
            </li>
          ))}
          <li>
            <HelpMenu collapsed={collapsed} />
          </li>
        </ul>
      </nav>
    </aside>
  );
}
