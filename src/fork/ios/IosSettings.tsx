// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings on iPhone: one grouped list (the iOS pattern) whose rows push
 * a page each, in place of the Mac's two-column modal. The settings are
 * the same ones (Handy's and the fork's stores); only what an iPhone has
 * is listed. Not here because the phone has none: shortcuts, the bar,
 * Dock/tray/launch at login, update channel, microphone choice, per-app
 * styles, Transforms, Command Mode. Style lives in the My Voice tab.
 *
 *   Settings                    pushed pages (#/hub/settings/<id>)
 *   ├ Languages ›               languages
 *   ├ Cleanup ›                 cleanup
 *   ├ End session after ›       session
 *   ├ Return to my app ◉
 *   ├ Dictation sounds ◉
 *   ├ Appearance  ▣▣▣
 *   ├ Sync with your Mac ›      sync
 *   ├ Data and Privacy ›        privacy
 *   └ About › Acknowledgements › about, licences
 */
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import type { Theme } from "@/bindings";
import {
  LanguageOptions,
  LanguageSearchField,
  useLanguagePicker,
} from "@/components/shared/LanguagePicker";
import { useSettings } from "@/hooks/useSettings";
import { languageList } from "@/lib/constants/languages";
import { applyTheme, getStoredTheme } from "@/lib/utils/theme";
import { commands, type CleanupLevel } from "../bindings";
import { navigate, useHubNav } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import { About } from "../settings/About";
import { DataPrivacy } from "../settings/DataPrivacy";
import { useForkSettings } from "../settings/useForkSettings";
import { CLEANUP_LEVELS } from "../style/CleanupLevelPicker";
import { SegmentedControl, toast } from "../ui";
import {
  Globe,
  Info,
  RefreshCw,
  ScrollText,
  ShieldCheck,
  Sparkles,
  Timer,
  Undo2,
  Volume2,
} from "../ui/icons";
import { useCleanupPhase } from "./cleanupPhase";
import { SESSION_TIMEOUTS } from "./commands";
import { sessionTimeoutLabel } from "./format";
import { IosSync, useLastSynced } from "./IosSync";
import { Licences } from "./Licences";
import { screenFor } from "./nav";
import {
  IosChoiceRow,
  IosGroup,
  IosNavRow,
  IosSwitchRow,
  IosTitle,
  IosValueRow,
} from "./parts";
import { useIosStore } from "./useIos";

export function IosSettings() {
  const page = useHubNav((s) => s.page);
  const section = useHubNav((s) => s.section);
  const screen = screenFor({ page, section });
  if (screen.kind !== "settings") return <SettingsRoot />;
  switch (screen.page) {
    case "languages":
      return <LanguagesPage />;
    case "cleanup":
      return <CleanupPage />;
    case "session":
      return <SessionPage />;
    case "sync":
      return <IosSync />;
    case "privacy":
      return <PrivacyPage />;
    case "about":
      return <AboutPage />;
    case "licences":
      return <Licences />;
  }
}

function SettingsRoot() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings: handy, updateSetting, isUpdating } = useSettings();
  const { settings } = useForkSettings();
  const timeout = useIosStore((s) => s.timeout);
  const autoReturn = useIosStore((s) => s.autoReturn);
  const setAutoReturn = useIosStore((s) => s.setAutoReturn);
  const cleanupIs = useCleanupPhase();
  const lastSynced = useLastSynced();
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  const languages = handy?.spoken_languages ?? [];
  const theme: Theme = handy?.theme ?? getStoredTheme();

  return (
    <div className="flex flex-col gap-6" data-testid="wb-ios-settings">
      <IosTitle>{t("ios.tabs.settings")}</IosTitle>
      <IosGroup label={t("ios.settings.groups.dictation")}>
        <IosNavRow
          icon={Globe}
          label={t("ios.settings.pages.languages")}
          value={languages.length ? languageList(languages, i18n.language) : ""}
          onClick={() => navigate("settings", "languages")}
          testId="wb-ios-settings-languages"
        />
        <IosNavRow
          icon={Sparkles}
          label={t("ios.settings.pages.cleanup")}
          value={
            cleanupIs === "unknown"
              ? ""
              : cleanupIs === "none"
                ? t("ios.settings.cleanup.notHere")
                : cleanupIs === "off"
                  ? t("ios.settings.cleanup.off")
                  : settings
                    ? t(`settings.cleanup.levels.${settings.cleanup_level}`)
                    : ""
          }
          onClick={() => navigate("settings", "cleanup")}
          testId="wb-ios-settings-cleanup"
        />
      </IosGroup>
      <IosGroup
        label={t("ios.settings.groups.session")}
        footer={t("ios.settings.session.autoReturnHint")}
      >
        <IosNavRow
          icon={Timer}
          label={t("ios.settings.session.endAfter")}
          value={sessionTimeoutLabel(t, timeout)}
          onClick={() => navigate("settings", "session")}
          testId="wb-ios-settings-session"
        />
        <IosSwitchRow
          icon={Undo2}
          label={t("ios.settings.session.autoReturn")}
          checked={autoReturn}
          onChange={(on) =>
            void setAutoReturn(on).then((ok) => {
              if (!ok) toast.error(t("ios.settings.saveFailed"));
            })
          }
          testId="wb-ios-settings-auto-return"
        />
      </IosGroup>
      <IosGroup label={t("ios.settings.groups.feedback")}>
        <IosSwitchRow
          icon={Volume2}
          label={t("ios.settings.sounds")}
          hint={t("ios.settings.soundsHint")}
          checked={!!handy?.audio_feedback}
          disabled={!handy || isUpdating("audio_feedback")}
          onChange={(on) => void updateSetting("audio_feedback", on)}
          testId="wb-ios-settings-sounds"
        />
      </IosGroup>
      <IosGroup label={t("ios.settings.groups.appearance")}>
        <div className="p-2">
          <SegmentedControl<Theme>
            value={theme}
            onChange={(next) => {
              applyTheme(next);
              void updateSetting("theme", next);
            }}
            label={t("ios.settings.groups.appearance")}
            options={(["system", "light", "dark"] as const).map((value) => ({
              value,
              label: t(`ios.settings.theme.${value}`),
            }))}
            className="w-full"
            testId="wb-ios-settings-theme"
          />
        </div>
      </IosGroup>
      <IosGroup label={t("ios.settings.groups.app")}>
        <IosNavRow
          icon={RefreshCw}
          label={t("ios.settings.pages.sync")}
          value={lastSynced}
          onClick={() => navigate("settings", "sync")}
          testId="wb-ios-settings-sync"
        />
        <IosNavRow
          icon={ShieldCheck}
          label={t("settingsModal.sections.privacy")}
          onClick={() => navigate("settings", "privacy")}
          testId="wb-ios-settings-privacy"
        />
        <IosNavRow
          icon={Info}
          label={t("settingsModal.sections.about")}
          onClick={() => navigate("settings", "about")}
          testId="wb-ios-settings-about"
        />
        <IosNavRow
          icon={ScrollText}
          label={t("ios.settings.pages.licences")}
          onClick={() => navigate("settings", "licences")}
          testId="wb-ios-settings-licences"
        />
      </IosGroup>
      <p
        className="text-center text-[13px] leading-[18px] text-(--wb-ink-3) tabular-nums"
        data-testid="wb-settings-version"
      >
        {t("rust.appName")}{" "}
        {t("settingsModal.about.version", { version: version ?? "…" })}
        <br />
        {t("ios.settings.local")}
      </p>
    </div>
  );
}

/* ── Languages ─────────────────────────────────────────────────────── */

function LanguagesPage() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, updateSetting } = useSettings();
  const saved = settings?.spoken_languages ?? [];
  const [languages, setLanguages] = useState<string[]>(saved);
  const touched = useRef(false);
  const savedKey = saved.join(",");
  useEffect(() => {
    if (!touched.current) setLanguages(savedKey ? savedKey.split(",") : []);
  }, [savedKey]);

  const picker = useLanguagePicker({
    languages,
    keepOne: true,
    onChange: (next) => {
      touched.current = true;
      setLanguages(next);
      void updateSetting("spoken_languages", next).then(() =>
        // The speech model follows the languages, in the background.
        commands.forkSpeechSetupRefresh(),
      );
    },
  });

  return (
    <div className="flex flex-col gap-4" data-testid="wb-ios-languages">
      <IosTitle>{t("ios.settings.pages.languages")}</IosTitle>
      <p className="text-[15px] leading-[21px] text-(--wb-ink-2)">
        {t("onboardingFlow.languages.subtitle")}
      </p>
      <div className="wb-ios-languages">
        <LanguageSearchField
          picker={picker}
          className="border-b border-(--wb-line)"
          inputClassName="min-w-[120px]"
        />
        <div
          role="group"
          aria-label={t("onboardingFlow.languages.listLabel")}
          className="grid grid-cols-2 content-start gap-[2px] p-1.5"
        >
          <LanguageOptions
            picker={picker}
            noMatch={t("onboardingFlow.languages.noMatch", {
              query: picker.query.trim(),
            })}
            emptyClassName="col-span-2 p-5"
          />
        </div>
      </div>
    </div>
  );
}

/* ── Cleanup ───────────────────────────────────────────────────────── */

function CleanupPage() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const phase = useCleanupPhase();
  const unavailable = phase === "none" || phase === "off";
  return (
    <div className="flex flex-col gap-4" data-testid="wb-ios-cleanup">
      <IosTitle>{t("ios.settings.pages.cleanup")}</IosTitle>
      <p className="text-[15px] leading-[21px] text-(--wb-ink-2)">
        {t("style.cleanup.description")}
      </p>
      {(unavailable || phase === "preparing") && (
        <div
          role="status"
          data-testid={`wb-ios-cleanup-${phase}`}
          className="rounded-(--wb-radius-lg) bg-(--wb-accent-soft) p-4"
        >
          <p className="type-card-title text-(--wb-ink)">
            {t(`ios.settings.cleanup.${phase}Title`)}
          </p>
          <p className="mt-1 text-[15px] leading-[21px] text-(--wb-ink-2)">
            {t(`ios.settings.cleanup.${phase}Body`)}
          </p>
        </div>
      )}
      <div
        role="radiogroup"
        aria-label={t("cleanup.picker.legend")}
        aria-disabled={unavailable || undefined}
        className="wb-ios-group"
        data-testid="fork-cleanup"
      >
        {CLEANUP_LEVELS.map((level) => (
          <IosChoiceRow
            key={level}
            label={t(`cleanup.levels.${level}.label`)}
            hint={t(`style.cleanup.levels.${level}`)}
            checked={!unavailable && settings?.cleanup_level === level}
            disabled={
              unavailable ||
              phase === "unknown" ||
              !settings ||
              !!pending.cleanup_level
            }
            onSelect={() =>
              void setSetting("cleanup_level", level as CleanupLevel)
            }
            testId={`fork-cleanup-level-${level}`}
          />
        ))}
      </div>
      {!unavailable && (
        <p
          className="px-1 text-[13px] leading-[18px] text-(--wb-ink-2)"
          data-testid="wb-ios-cleanup-note"
        >
          {t("ios.settings.cleanup.note")}
        </p>
      )}
    </div>
  );
}

/* ── Session ───────────────────────────────────────────────────────── */

function SessionPage() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const timeout = useIosStore((s) => s.timeout);
  const setTimeout = useIosStore((s) => s.setTimeout);
  return (
    <div className="flex flex-col gap-4" data-testid="wb-ios-session-settings">
      <IosTitle>{t("ios.settings.session.endAfter")}</IosTitle>
      <p className="text-[15px] leading-[21px] text-(--wb-ink-2)">
        {t("ios.settings.session.intro")}
      </p>
      <div
        role="radiogroup"
        aria-label={t("ios.settings.session.endAfter")}
        className="wb-ios-group"
      >
        {SESSION_TIMEOUTS.map((seconds) => (
          <IosChoiceRow
            key={seconds}
            label={sessionTimeoutLabel(t, seconds)}
            checked={timeout === seconds}
            onSelect={() =>
              void setTimeout(seconds).then((ok) => {
                if (!ok) toast.error(t("ios.settings.saveFailed"));
              })
            }
            testId={`wb-ios-timeout-${seconds}`}
          />
        ))}
      </div>
      <p className="px-4 text-[13px] leading-[18px] text-(--wb-ink-3)">
        {t("ios.settings.session.footer")}
      </p>
    </div>
  );
}

/* ── Data and Privacy, About ───────────────────────────────────────── */

function PrivacyPage() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div className="flex flex-col gap-5" data-testid="wb-ios-privacy">
      <IosTitle>{t("settingsModal.sections.privacy")}</IosTitle>
      <DataPrivacy />
    </div>
  );
}

function AboutPage() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div className="flex flex-col gap-5" data-testid="wb-ios-about">
      <IosTitle>{t("settingsModal.sections.about")}</IosTitle>
      <About />
    </div>
  );
}
