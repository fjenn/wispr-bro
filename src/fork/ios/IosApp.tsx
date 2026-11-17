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
 *   │ Home  History  Dictionary  Snippets ⚙│  tab bar + home indicator
 *   └──────────────────────────────────────┘
 *
 * The pages are the Hub's own (Dictionary, Snippets, Style, Insights,
 * History); only Home and Settings are composed for the phone. Navigation
 * is the Hub's store read as tabs (`./nav`). No window chrome, sidebar,
 * shortcuts, tutorials or update status: none of them exist on iPhone.
 */
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ComponentType,
} from "react";
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { DictionaryPage } from "../dictionary/DictionaryPage";
import {
  HandyAppHandlersContext,
  type HandyAppHandlers,
} from "../hub/HandySettingsPage";
import { navigate, syncHubWithHash, useHubNav } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import { InsightsPage } from "../insights/InsightsPage";
import { useForkSettings } from "../settings/useForkSettings";
import { SnippetsPage } from "../snippets/SnippetsPage";
import { IosStyle } from "./IosStyle";
import { TooltipProvider } from "../ui";
import {
  AudioLines,
  BookA,
  ChevronLeft,
  Clock,
  Settings,
  TextQuote,
  type Glyph,
} from "../ui/icons";
import { MotionRoot, useMotionPresets } from "../ui/motion";
import { IosHistory } from "./IosHistory";
import { IosHome } from "./IosHome";
import { IosSettings } from "./IosSettings";
import {
  IOS_TABS,
  screenFor,
  screenId,
  TAB_LOCATION,
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
  dictionary: BookA,
  snippets: TextQuote,
  settings: Settings,
};

/** Scrolled past the large title: the bar turns solid and shows the title. */
const BAR_SOLID_AT = 30;

function Page({ screen }: { screen: IosScreen }) {
  let Component: ComponentType;
  if (screen.kind === "style") Component = IosStyle;
  else if (screen.kind === "insights") Component = InsightsPage;
  else if (screen.kind === "settings") Component = IosSettings;
  else {
    Component = {
      home: IosHome,
      history: IosHistory,
      dictionary: DictionaryPage,
      snippets: SnippetsPage,
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
  useEffect(() => syncHubWithHash(), []);

  const tab = screen.kind === "tab" ? screen.tab : null;
  useEffect(() => {
    if (tab) setOrigin(tab);
  }, [tab]);

  // Each navigation starts at the top of the page.
  useLayoutEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
    setSolid(false);
  }, [id, seq]);

  const pushed = screen.kind !== "tab";
  const activeTab: IosTab =
    screen.kind === "tab"
      ? screen.tab
      : screen.kind === "settings"
        ? "settings"
        : origin === "settings"
          ? "settings"
          : "home";
  const backTab: IosTab = activeTab;
  const title =
    screen.kind === "tab"
      ? t(`ios.tabs.${screen.tab}`)
      : screen.kind === "settings"
        ? t(`ios.settings.pages.${screen.page}`)
        : t(`hub.pages.${screen.kind}.title`);

  const go = (next: IosTab) => {
    if (next === tab) {
      mainRef.current?.scrollTo({ top: 0, behavior: "smooth" });
      return;
    }
    const to = TAB_LOCATION[next];
    navigate(to.page, to.section);
  };

  return (
    <HandyAppHandlersContext.Provider value={props}>
      <MotionRoot>
        <TooltipProvider>
          <div
            data-testid="wb-ios"
            data-screen={id}
            dir={getLanguageDirection(i18n.language)}
            className="wb-ios-root"
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
