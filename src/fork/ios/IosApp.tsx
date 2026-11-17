// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Wispr Bro for iPhone: the app shell. `App.tsx` renders it in place of
 * the Hub (`../hub/ForkApp`) when `isIos()`.
 *
 *   ┌ status bar (safe area) ──────────────┐
 *   │ ‹ Back        small title            │  bar: solid once scrolled
 *   │ Large title                          │
 *   │ page (one column, scrolls)           │
 *   │                                      │
 *   ├──────────────────────────────────────┤
 *   │ Home  History  My Voice  Scratchpad ⚙│  tab bar + home indicator
 *   └──────────────────────────────────────┘
 *
 * The pages are the Hub's own (Dictionary, Snippets, Insights, History);
 * Home, My Voice (three rows that push Dictionary, Snippets and Style),
 * Scratchpad and Settings are composed for the phone. A note's editor covers the tab
 * bar and puts its actions in the top bar's end slot. Navigation is the
 * Hub's store read as tabs (`./nav`). No window chrome, sidebar,
 * shortcuts, tutorials or update status: none of them exist on iPhone.
 */
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ComponentType,
  type TouchEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { getLanguageDirection } from "@/lib/utils/rtl";
import {
  HandyAppHandlersContext,
  type HandyAppHandlers,
} from "../hub/HandySettingsPage";
import { navigate, syncHubWithHash, useHubNav } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import { DictionaryPage } from "../dictionary/DictionaryPage";
import { InsightsPage } from "../insights/InsightsPage";
import { SnippetsPage } from "../snippets/SnippetsPage";
import { useForkSettings } from "../settings/useForkSettings";
import { TooltipProvider } from "../ui";
import {
  AudioLines,
  ChevronLeft,
  Clock,
  NotebookPen,
  Settings,
  Speech,
  type Glyph,
} from "../ui/icons";
import { MotionRoot, useMotionPresets } from "../ui/motion";
import { IosHistory } from "./IosHistory";
import { IosHome } from "./IosHome";
import { IosMyVoice } from "./IosMyVoice";
import { IosStyle } from "./IosStyle";
import { BAR_END_ID, IosNote } from "./IosNote";
import { IosScratchpad } from "./IosScratchpad";
import { IosSettings } from "./IosSettings";
import { useSyncInbox } from "./IosSync";
import {
  IOS_TABS,
  screenFor,
  screenId,
  tabLocation,
  type IosScreen,
  type IosTab,
} from "./nav";
import { useIosLive } from "./useIos";
import { useKeyboardInset } from "./useKeyboardInset";
import "../styles/fonts.css";
import "../styles/tokens.css";
import "./ios.css";

const TAB_ICONS: Record<IosTab, Glyph> = {
  home: AudioLines,
  history: Clock,
  voice: Speech,
  scratchpad: NotebookPen,
  settings: Settings,
};

/** A back swipe starts this close to the leading edge and travels this far. */
const EDGE_SWIPE_FROM = 24;
const EDGE_SWIPE_DISTANCE = 72;

/** Scrolled past the large title: the bar turns solid and shows the title. */
const BAR_SOLID_AT = 30;

function Page({ screen }: { screen: IosScreen }) {
  let Component: ComponentType;
  if (screen.kind === "voice") {
    Component = {
      dictionary: DictionaryPage,
      snippets: SnippetsPage,
      style: IosStyle,
    }[screen.page];
    return <Component />;
  }
  if (screen.kind === "note") return <IosNote key={screen.id} id={screen.id} />;
  if (screen.kind === "insights") Component = InsightsPage;
  else if (screen.kind === "settings") Component = IosSettings;
  else {
    Component = {
      home: IosHome,
      history: IosHistory,
      voice: IosMyVoice,
      scratchpad: IosScratchpad,
      settings: IosSettings,
    }[screen.tab];
  }
  return <Component />;
}

export type IosAppProps = HandyAppHandlers;

export function IosApp(props: IosAppProps) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const page = useHubNav((s) => s.page);
  const section = useHubNav((s) => s.section);
  const seq = useHubNav((s) => s.seq);
  const screen = screenFor({ page, section });
  const id = screenId(screen);
  const mainRef = useRef<HTMLElement>(null);
  const [solid, setSolid] = useState(false);
  // The tab a pushed page was opened from (Back returns there).
  const [origin, setOrigin] = useState<IosTab>("home");
  const m = useMotionPresets();

  useForkSettings();
  useIosLive();
  useKeyboardInset();
  // A file from AirDrop or Files opens its review on the Sync page.
  useSyncInbox();
  useEffect(() => syncHubWithHash(), []);

  const tab: IosTab | null = screen.kind === "tab" ? screen.tab : null;
  useEffect(() => {
    if (tab) setOrigin(tab);
  }, [tab]);

  // Each navigation starts at the top of the page.
  useLayoutEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
    setSolid(false);
  }, [id, seq]);

  const pushed = tab === null;
  // A note's editor is the whole screen: no tab bar under it.
  const full = screen.kind === "note";
  const activeTab: IosTab =
    tab ??
    (screen.kind === "settings"
      ? "settings"
      : screen.kind === "note"
        ? "scratchpad"
        : screen.kind === "voice"
          ? "voice"
          : origin === "settings"
            ? "settings"
            : "home");
  const backTab: IosTab = activeTab;
  const title = tab
    ? t(`ios.tabs.${tab}`)
    : screen.kind === "settings"
      ? t(`ios.settings.pages.${screen.page}`)
      : screen.kind === "note"
        ? ""
        : screen.kind === "voice"
          ? t(`hub.pages.${screen.page}.title`)
          : t("hub.pages.insights.title");

  const go = (next: IosTab) => {
    if (next === tab) {
      mainRef.current?.scrollTo({ top: 0, behavior: "smooth" });
      return;
    }
    const to = tabLocation(next);
    navigate(to.page, to.section);
  };

  // A swipe in from the leading edge goes back, as a pushed page does on
  // iPhone.
  const swipe = useRef<{ x: number; y: number } | null>(null);
  const rtl = getLanguageDirection(i18n.language) === "rtl";
  const onTouchStart = (event: TouchEvent<HTMLDivElement>) => {
    const touch = event.touches[0];
    const fromEdge = rtl ? window.innerWidth - touch.clientX : touch.clientX;
    swipe.current =
      pushed && event.touches.length === 1 && fromEdge <= EDGE_SWIPE_FROM
        ? { x: touch.clientX, y: touch.clientY }
        : null;
  };
  const onTouchEnd = (event: TouchEvent<HTMLDivElement>) => {
    const start = swipe.current;
    swipe.current = null;
    if (!start) return;
    const touch = event.changedTouches[0];
    const dx = (touch.clientX - start.x) * (rtl ? -1 : 1);
    const dy = Math.abs(touch.clientY - start.y);
    if (dx >= EDGE_SWIPE_DISTANCE && dy < dx / 2) go(backTab);
  };

  return (
    <HandyAppHandlersContext.Provider value={props}>
      <MotionRoot>
        <TooltipProvider>
          <div
            data-testid="wb-ios"
            data-screen={id}
            data-full={full || undefined}
            dir={getLanguageDirection(i18n.language)}
            className="wb-ios-root"
            onTouchStart={onTouchStart}
            onTouchEnd={onTouchEnd}
          >
            <header
              className="wb-ios-bar"
              data-solid={solid || undefined}
              data-pushed={pushed || undefined}
              data-testid="wb-ios-bar"
            >
              <div className="wb-ios-bar-row">
                {pushed && (
                  <button
                    type="button"
                    className="wb-ios-back"
                    data-testid="wb-ios-back"
                    onClick={() => go(backTab)}
                  >
                    <ChevronLeft
                      size={24}
                      aria-hidden="true"
                      className="rtl:-scale-x-100"
                    />
                    <span>{t(`ios.tabs.${backTab}`)}</span>
                  </button>
                )}
                <span className="wb-ios-bar-title" aria-hidden="true">
                  {title}
                </span>
                {/* A pushed page's own actions (the note editor's). */}
                <div id={BAR_END_ID} className="wb-ios-bar-end" />
              </div>
            </header>
            <main
              ref={mainRef}
              data-testid="wb-main"
              data-page={id}
              data-pushed={pushed || undefined}
              className="wb-ios-scroll"
              onScroll={(event) => {
                const next = event.currentTarget.scrollTop > BAR_SOLID_AT;
                setSolid((was) => (was === next ? was : next));
              }}
            >
              <motion.div
                key={id}
                initial={m.page.initial}
                animate={m.page.animate}
                className="wb-ios-page"
              >
                <ErrorBoundary key={id} context={id}>
                  <Page screen={screen} />
                </ErrorBoundary>
              </motion.div>
            </main>
            <nav
              className="wb-ios-tabs"
              aria-label={t("ios.tabs.label")}
              data-testid="wb-ios-tabs"
            >
              {IOS_TABS.map((item) => {
                const Icon = TAB_ICONS[item];
                const current = item === activeTab;
                return (
                  <button
                    key={item}
                    type="button"
                    className="wb-ios-tab"
                    aria-current={current ? "page" : undefined}
                    data-testid={`wb-ios-tab-${item}`}
                    onClick={() => go(item)}
                  >
                    <Icon size={24} aria-hidden="true" />
                    <span>{t(`ios.tabs.${item}`)}</span>
                  </button>
                );
              })}
            </nav>
          </div>
        </TooltipProvider>
      </MotionRoot>
    </HandyAppHandlersContext.Provider>
  );
}
