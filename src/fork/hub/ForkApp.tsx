// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Hub (S-13): Wispr Bro's main window once onboarding is done.
 *
 * `App.tsx` renders this in place of Handy's sidebar and section layout (the
 * one React hook of R5 §4.2); Handy's onboarding switch around it is
 * unchanged until S-29a. Layout:
 *
 *   ┌ sidebar ───────┬ header (drag strip, theme, bell) ───────┐
 *   │ (traffic       ├──────────────────────────────────────────┤
 *   │  lights, ⧉)    │ page slot (scrolls)                      │
 *   │ pages…         │                                          │
 *   │ share/settings ├──────────────────────────────────────────┤
 *   │ help           │ footer (jobs · update · version)         │
 *   └────────────────┴──────────────────────────────────────────┘
 *
 * On macOS the window uses an overlay title bar while the Hub shows, so the
 * sidebar runs under the traffic lights (C-25); the strip at the top is a
 * drag region. Onboarding screens get the standard title bar back.
 */
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import SecureInputWarning from "@/components/SecureInputWarning";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { WhatsNewGate } from "@/components/whats-new";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { FOCUS_RING, markNewTagSeen } from "../ui";
import { listenForTransformChanges } from "../transforms/store";
import { useDropRouter } from "./dropRouter";
import { Footer } from "./Footer";
import {
  HandyAppHandlersContext,
  type HandyAppHandlers,
} from "./HandySettingsPage";
import { FeaturePopup } from "./FeaturePopup";
import { Header } from "./Header";
import { syncHubWithHash, useHubNav } from "./nav";
import { useMenuActions } from "./menuActions";
import { useNotificationsLive } from "./NotificationsPanel";
import { hubPage, newTagId } from "./pages";
import { Sidebar } from "./Sidebar";
import "../styles/fonts.css";
import "../styles/tokens.css";

export const SIDEBAR_COLLAPSED_KEY = "wb.hub.sidebarCollapsed";

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "1";
  } catch {
    return false;
  }
}

function writeCollapsed(collapsed: boolean): void {
  try {
    localStorage.setItem(SIDEBAR_COLLAPSED_KEY, collapsed ? "1" : "0");
  } catch {
    // A UI convenience only; the sidebar still toggles for this session.
  }
}

function isMac(): boolean {
  try {
    return platform() === "macos";
  } catch {
    return false;
  }
}

/** Overlay title bar while the Hub is mounted (macOS only). */
function useOverlayTitleBar(): void {
  useEffect(() => {
    if (!isMac()) return;
    const window = getCurrentWindow();
    void window
      .setTitleBarStyle("overlay")
      .catch((e) => console.warn("fork: overlay title bar failed", e));
    document.documentElement.dataset.forkHub = "";
    return () => {
      delete document.documentElement.dataset.forkHub;
      void window
        .setTitleBarStyle("visible")
        .catch((e) => console.warn("fork: title bar restore failed", e));
    };
  }, []);
}

export type ForkAppProps = HandyAppHandlers;

export function ForkApp(props: ForkAppProps) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings: forkSettings } = useForkSettings();
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const pageId = useHubNav((s) => s.page);
  const seq = useHubNav((s) => s.seq);
  const mainRef = useRef<HTMLElement>(null);
  const page = hubPage(pageId);
  const direction = getLanguageDirection(i18n.language);
  const Page = page.component;

  useOverlayTitleBar();
  useDropRouter();
  useMenuActions();
  useNotificationsLive();
  useEffect(() => {
    void listenForTransformChanges();
  }, []);
  useEffect(() => syncHubWithHash(), []);

  // Each navigation starts at the top of the page.
  useLayoutEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
  }, [pageId, seq]);

  // Opening a page clears its "New!" tag.
  useEffect(() => {
    if (page.tag === "new" && forkSettings) {
      void markNewTagSeen(newTagId(page.id));
    }
  }, [page, forkSettings]);

  const toggleCollapsed = () => {
    setCollapsed((was) => {
      writeCollapsed(!was);
      return !was;
    });
  };
  const collapseLabel = collapsed
    ? t("hub.sidebar.expand")
    : t("hub.sidebar.collapse");
  const CollapseIcon = collapsed ? PanelLeftOpen : PanelLeftClose;

  return (
    <HandyAppHandlersContext.Provider value={props}>
      <div
        data-testid="wb-hub"
        dir={direction}
        className="relative flex h-screen cursor-default overflow-hidden bg-(--wb-canvas) text-(--wb-ink) select-none"
      >
        {/* In the title strip, right of the traffic lights (C-14). Physical
            `left`: macOS keeps the traffic lights on the left in RTL too. First in
            the DOM so it is the first stop when tabbing. */}
        <button
          type="button"
          data-testid="wb-sidebar-toggle"
          aria-label={collapseLabel}
          title={collapseLabel}
          aria-expanded={!collapsed}
          aria-controls="wb-sidebar"
          onClick={toggleCollapsed}
          className={`absolute top-(--wb-traffic-top) left-[76px] z-20 grid size-6 cursor-pointer place-items-center rounded-[7px] text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
        >
          <CollapseIcon size={16} aria-hidden="true" />
        </button>
        <ErrorBoundary context="What's New">
          <WhatsNewGate />
        </ErrorBoundary>
        <ErrorBoundary context="Feature popup">
          <FeaturePopup />
        </ErrorBoundary>
        <Sidebar collapsed={collapsed} />
        <div className="flex min-w-0 flex-1 flex-col">
          {/* The traffic lights stay top-left in every language; in RTL the
              sidebar is on the right, so the header always clears them. */}
          <Header inset={collapsed || direction === "rtl"} />
          <main
            ref={mainRef}
            data-testid="wb-main"
            data-page={page.id}
            className="min-h-0 flex-1 overflow-y-auto"
          >
            <div className="mx-auto flex w-full max-w-[880px] flex-col gap-6 px-8 pt-6 pb-10">
              <SecureInputWarning />
              <ErrorBoundary key={page.id} context={page.id}>
                <Page />
              </ErrorBoundary>
            </div>
          </main>
          <Footer />
        </div>
      </div>
    </HandyAppHandlersContext.Provider>
  );
}
