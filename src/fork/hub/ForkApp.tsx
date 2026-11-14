// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Hub (S-13): Wispr Bro's main window once onboarding is done.
 *
 * `App.tsx` renders this in place of Handy's sidebar and section layout (the
 * one React hook of R5 §4.2); Handy's onboarding switch around it is
 * unchanged until S-29a. Layout (Design Phase 0, SYSTEM-PLAN §7):
 *
 *   ┌ ● ● ●  ⧉ ─────────────── title strip (46 pt) ─ jobs · update ☾ 🔔 ┐
 *   │ sidebar         ╭──────────────────────────────────────────────╮  │
 *   │ (chrome)        │ inset content panel, 22 pt radius            │  │
 *   │ pages…          │ page slot (scrolls inside the panel)         │  │
 *   │ checklist card  │                                              │  │
 *   │ share/settings  │                                              │  │
 *   │ help            ╰──────────────────────────────────────────────╯  │
 *   └───────────────────────────────────────────────────────── 10 pt ──┘
 *
 * On macOS the window uses an overlay title bar while the Hub shows, so the
 * strip runs under the traffic lights (C-25) and drags the window; the
 * onboarding screens use the same overlay strip. There is no footer:
 * the version and the update panel live in Settings › About; background
 * jobs and an update that needs attention show in the title strip.
 */
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { WhatsNewGate } from "../whats-new/WhatsNewGate";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { motion } from "motion/react";
import { IconButton, markNewTagSeen, TooltipProvider } from "../ui";
import { PanelLeft } from "../ui/icons";
import { MotionRoot, useMotionPresets } from "../ui/motion";
import { listenForTransformChanges } from "../transforms/store";
import { FixShortcutSheet } from "../settings/FixShortcutSheet";
import {
  FIX_SHORTCUT_SECTION,
  listenForShortcutHealth,
  useShortcutHealth,
} from "../settings/shortcutHealth";
import { useDropRouter } from "./dropRouter";
import {
  HandyAppHandlersContext,
  type HandyAppHandlers,
} from "./HandySettingsPage";
import { FeaturePopup } from "./FeaturePopup";
import { TitleStrip } from "./Header";
import { syncHubWithHash, useHubNav } from "./nav";
import { useMenuActions } from "./menuActions";
import { useNotificationsLive } from "./NotificationsPanel";
import { hubPage, newTagId } from "./pages";
import { HubNotices } from "./HubNotices";
import { setSidebarCard, Sidebar } from "./Sidebar";
import { renderSetupChecklist } from "./SetupChecklist";
import { FirstUseChooser } from "../onboarding/FirstUseChooser";
import { useFirstUseLive } from "../onboarding/firstUseStore";
import { LessonHost } from "../tutorials";
import { installBeforeCaret } from "../ui/beforeCaret";
import "../styles/fonts.css";
import "../styles/tokens.css";

// The smart leading space for dictations into the Hub's own fields.
installBeforeCaret();

// S-30: the setup checklist fills S-13's sidebar slot.
setSidebarCard(renderSetupChecklist);

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

/**
 * Overlay title bar (macOS only). The window is built with it (Design
 * Phase 0, `fork/hub/mod.rs`) and onboarding keeps it too, so this only
 * re-asserts it for a window built before that change.
 */
function useOverlayTitleBar(): void {
  useEffect(() => {
    if (!isMac()) return;
    void getCurrentWindow()
      .setTitleBarStyle("overlay")
      .catch((e) => console.warn("fork: overlay title bar failed", e));
    document.documentElement.dataset.forkHub = "";
    return () => {
      delete document.documentElement.dataset.forkHub;
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
  useMenuActions(props.onRunOnboarding);
  useNotificationsLive();
  useFirstUseLive();
  useEffect(() => {
    void listenForTransformChanges();
  }, []);
  useEffect(() => syncHubWithHash(), []);
  // S-02: shortcut health (banner, Fix sheet, the bar's and tray's Fix).
  useEffect(() => listenForShortcutHealth(), []);
  // The shortcut-problem notification opens the Fix sheet.
  const section = useHubNav((s) => s.section);
  useEffect(() => {
    if (section === FIX_SHORTCUT_SECTION) {
      useShortcutHealth.getState().openSheet();
    }
  }, [section, seq]);

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
  const m = useMotionPresets();

  const toggle = (
    <IconButton
      data-testid="wb-sidebar-toggle"
      aria-label={collapseLabel}
      title={collapseLabel}
      aria-expanded={!collapsed}
      aria-controls="wb-sidebar"
      onClick={toggleCollapsed}
    >
      <PanelLeft size={20} />
    </IconButton>
  );

  return (
    <HandyAppHandlersContext.Provider value={props}>
      <MotionRoot>
        <TooltipProvider>
          <div
            data-testid="wb-hub"
            dir={direction}
            className="relative flex h-screen cursor-default flex-col overflow-hidden bg-(--wb-chrome) text-(--wb-ink) select-none"
          >
            {/* First in the DOM so the sidebar toggle is the first tab stop. */}
            <TitleStrip start={toggle} />
            <ErrorBoundary context="What's New">
              <WhatsNewGate />
            </ErrorBoundary>
            <ErrorBoundary context="Feature popup">
              <FeaturePopup />
            </ErrorBoundary>
            <ErrorBoundary context="Fix shortcut">
              <FixShortcutSheet />
            </ErrorBoundary>
            <ErrorBoundary context="First use">
              <FirstUseChooser />
            </ErrorBoundary>
            <ErrorBoundary context="Lesson">
              <LessonHost />
            </ErrorBoundary>
            <div className="flex min-h-0 flex-1">
              <Sidebar collapsed={collapsed} />
              <main
                ref={mainRef}
                data-testid="wb-main"
                data-page={page.id}
                className="mb-(--wb-panel-inset-bottom) me-(--wb-panel-inset-end) min-h-0 min-w-0 flex-1 overflow-y-auto rounded-(--wb-radius-xl) border border-(--wb-line) bg-(--wb-panel)"
              >
                <motion.div
                  key={page.id}
                  initial={m.page.initial}
                  animate={m.page.animate}
                  className="mx-auto flex w-full max-w-[1000px] flex-col gap-6 px-(--wb-page-pad-x) pt-(--wb-page-pad-top) pb-10 max-[860px]:px-8"
                >
                  <ErrorBoundary context="Notices">
                    <HubNotices />
                  </ErrorBoundary>
                  <ErrorBoundary key={page.id} context={page.id}>
                    <Page />
                  </ErrorBoundary>
                </motion.div>
              </main>
            </div>
          </div>
        </TooltipProvider>
      </MotionRoot>
    </HandyAppHandlersContext.Provider>
  );
}
