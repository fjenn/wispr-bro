// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Settings page slot until S-14's Settings modal (R5 S-13 decision).
 *
 * The fork sidebar replaces Handy's (R5 §4.2: `Sidebar.tsx` untouched), so
 * Handy's settings sections move here unchanged, as tabs, and every existing
 * setting stays reachable. The tab is the page's `section`, so deep links
 * like `navigate("settings", "history")` open a section directly.
 *
 * History keeps Handy's file transcription, but only for files dropped on
 * the History tab (C-205, RE-33): the tab panel is a drop zone.
 */
import { createContext, useContext, useRef } from "react";
import { useTranslation } from "react-i18next";
import { FileAudio } from "lucide-react";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { SECTIONS_CONFIG, type SidebarSection } from "@/components/Sidebar";
import { DebugSettings, ModelsSettings } from "@/components/settings";
import { transcribeFiles } from "@/components/settings/history/transcribeFiles";
import type { OnboardingStep } from "@/components/onboarding/onboardingStore";
import { useSettings } from "@/hooks/useSettings";
import { FORK_NAMESPACE } from "../i18n";
import { TabPanel, Tabs } from "../ui";
import { useDropZone } from "./dropRouter";
import { navigate, useHubNav } from "./nav";
import { PageHeader } from "./PageHeader";

/** Handlers Handy's Debug section needs from `App.tsx` (onboarding control). */
export interface HandyAppHandlers {
  onPreviewOnboarding: (step: OnboardingStep) => void;
  onRunOnboarding: () => void;
  onRunModelMigration: () => void;
}

const noop = () => undefined;

export const HandyAppHandlersContext = createContext<HandyAppHandlers>({
  onPreviewOnboarding: noop,
  onRunOnboarding: noop,
  onRunModelMigration: noop,
});

const DEFAULT_SECTION: SidebarSection = "general";

function isSection(value: string | null): value is SidebarSection {
  return (
    value !== null &&
    Object.prototype.hasOwnProperty.call(SECTIONS_CONFIG, value)
  );
}

function SectionContent({ section }: { section: SidebarSection }) {
  const handlers = useContext(HandyAppHandlersContext);
  if (section === "models") {
    return (
      <ModelsSettings
        onChangeLanguages={() => navigate("settings", "general")}
      />
    );
  }
  if (section === "debug") {
    return (
      <DebugSettings
        onPreviewOnboarding={handlers.onPreviewOnboarding}
        onRunOnboarding={handlers.onRunOnboarding}
        onRunModelMigration={handlers.onRunModelMigration}
      />
    );
  }
  const Component =
    SECTIONS_CONFIG[section].component ?? SECTIONS_CONFIG.general.component;
  return <Component />;
}

/** History with Handy's "drop audio to transcribe", scoped to this panel. */
function HistoryDropZone({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  const active = useDropZone(ref, {
    onDrop: async (paths) => {
      await transcribeFiles(paths);
    },
  });
  return (
    <div
      ref={ref}
      data-testid="wb-drop-zone-history"
      data-drop-active={active || undefined}
      className="relative w-full"
    >
      {children}
      {active && (
        <div className="pointer-events-none absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 rounded-(--wb-radius-card) border-2 border-dashed border-(--wb-accent) bg-(--wb-canvas)/90 text-center">
          <FileAudio
            size={36}
            aria-hidden="true"
            className="text-(--wb-ink-2)"
          />
          <p className="text-base font-semibold">
            {t("settings.history.dropTitle")}
          </p>
          <p className="text-sm text-(--wb-ink-2)">
            {t("settings.history.dropDescription")}
          </p>
        </div>
      )}
    </div>
  );
}

export function HandySettingsPage() {
  const { t } = useTranslation();
  const { t: tf } = useTranslation(FORK_NAMESPACE);
  const { settings } = useSettings();
  const requested = useHubNav((s) => s.section);

  const sections = (
    Object.entries(SECTIONS_CONFIG) as [
      SidebarSection,
      (typeof SECTIONS_CONFIG)[SidebarSection],
    ][]
  )
    .filter(([, config]) => config.enabled(settings))
    .map(([id, config]) => ({ value: id, label: t(config.labelKey) }));

  const section =
    isSection(requested) && sections.some((s) => s.value === requested)
      ? requested
      : DEFAULT_SECTION;

  return (
    <>
      <PageHeader title={tf("hub.pages.settings.title")} />
      <Tabs
        value={section}
        onValueChange={(value) => navigate("settings", value)}
        tabs={sections}
        label={tf("hub.settings.sectionsLabel")}
      >
        {sections.map(({ value }) => (
          <TabPanel key={value} value={value} className="pt-4">
            {value === section && (
              <div
                data-handy-content
                className={`flex flex-col items-center gap-4 ${value === "models" ? "h-[calc(100vh-15rem)]" : ""}`}
              >
                <ErrorBoundary context={`settings ${value}`}>
                  {value === "history" ? (
                    <HistoryDropZone>
                      <SectionContent section={value} />
                    </HistoryDropZone>
                  ) : (
                    <SectionContent section={value} />
                  )}
                </ErrorBoundary>
              </div>
            )}
          </TabPanel>
        ))}
      </Tabs>
    </>
  );
}
