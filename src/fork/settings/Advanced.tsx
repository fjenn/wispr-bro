// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Advanced (S-14; Design W3): the fork's panels (models, AI
 * engine, cleanup) as tinted cards, then Handy's own settings, each hosted
 * in a row card (`HandyRow`) with its description behind the info glyph:
 * this section is for people tuning the engine, so it stays dense.
 */
import { useContext } from "react";
import { useTranslation } from "react-i18next";
import { AccelerationSelector } from "@/components/settings/AccelerationSelector";
import { AppDataDirectory } from "@/components/settings/AppDataDirectory";
import { AppendTrailingSpace } from "@/components/settings/AppendTrailingSpace";
import { AutoSubmit } from "@/components/settings/AutoSubmit";
import { ChannelSelector } from "@/components/settings/ChannelSelector";
import { ClipboardHandlingSetting } from "@/components/settings/ClipboardHandling";
import { CustomWords } from "@/components/settings/CustomWords";
import { ExperimentalToggle } from "@/components/settings/ExperimentalToggle";
import { FillerWordRemoval } from "@/components/settings/FillerWordRemoval";
import { HistoryLimit } from "@/components/settings/HistoryLimit";
import { LanguageDetection } from "@/components/settings/LanguageDetection";
import { LazyStreamClose } from "@/components/settings/LazyStreamClose";
import { LongFormChunking } from "@/components/settings/LongFormChunking";
import { ModelUnloadTimeoutSetting } from "@/components/settings/ModelUnloadTimeout";
import { PasteMethodSetting } from "@/components/settings/PasteMethod";
import { PostProcessingSettingsApi } from "@/components/settings/PostProcessingSettingsApi";
import { PostProcessingSettingsPrompts } from "@/components/settings/PostProcessingSettingsPrompts";
import { PostProcessingToggle } from "@/components/settings/PostProcessingToggle";
import { RecordingRetentionPeriodSelector } from "@/components/settings/RecordingRetentionPeriod";
import { ShortcutActivationSetting } from "@/components/settings/ShortcutActivation";
import { ShowOverlay } from "@/components/settings/ShowOverlay";
import { TranslateToEnglish } from "@/components/settings/TranslateToEnglish";
import { TypingToolSetting } from "@/components/settings/TypingTool";
import { VadBackendSelector } from "@/components/settings/VadBackendSelector";
import { VoiceActivityDetection } from "@/components/settings/VoiceActivityDetection";
import { DebugSettings } from "@/components/settings/debug/DebugSettings";
import { KeyboardImplementationSelector } from "@/components/settings/debug/KeyboardImplementationSelector";
import { LogDirectory } from "@/components/settings/debug/LogDirectory";
import { useSettings } from "@/hooks/useSettings";
import { HandyAppHandlersContext } from "../hub/HandySettingsPage";
import type { UpdateChannel } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { CleanupLevelPicker } from "../style/CleanupLevelPicker";
import { AiEngine } from "./advanced/AiEngine";
import { SpeechModels } from "./advanced/Models";
import { BarSettings } from "./BarSettings";
import { FOCUS_RING, SettingGroup } from "../ui";
import { ChevronRight } from "../ui/icons";
import { HandyRow, PanelCard, SelectRow, ToggleRow } from "./Controls";
import { ForkSettingsDebug } from "./ForkSettingsDebug";
import { useForkSettings } from "./useForkSettings";

/** Handy's transcription settings, each in its own row card. */
const TRANSCRIPTION = [
  ShortcutActivationSetting,
  ChannelSelector,
  VoiceActivityDetection,
  TranslateToEnglish,
  LongFormChunking,
  FillerWordRemoval,
  CustomWords,
  ModelUnloadTimeoutSetting,
  AccelerationSelector,
  LazyStreamClose,
  VadBackendSelector,
  LanguageDetection,
];

export function Advanced() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const { settings: handy, updateSetting } = useSettings();
  const handlers = useContext(HandyAppHandlersContext);
  if (!settings) return null;

  const g = (key: string) => t(`settingsModal.advanced.${key}`);
  return (
    <div className="flex flex-col gap-8" data-testid="wb-settings-advanced">
      <div data-testid="wb-advanced-models">
        <SettingGroup label={t("speechModels.title")}>
          <PanelCard>
            <SpeechModels />
          </PanelCard>
        </SettingGroup>
      </div>
      <div data-testid="wb-advanced-ai">
        <SettingGroup label={t("aiEngine.title")}>
          <PanelCard>
            <AiEngine />
          </PanelCard>
        </SettingGroup>
      </div>
      <div data-testid="wb-advanced-cleanup">
        <SettingGroup label={t("cleanup.title")}>
          <PanelCard>
            <CleanupLevelPicker />
          </PanelCard>
        </SettingGroup>
      </div>
      <SettingGroup label={g("output")}>
        <HandyRow>
          <PasteMethodSetting descriptionMode="tooltip" grouped />
        </HandyRow>
        <HandyRow>
          <TypingToolSetting descriptionMode="tooltip" grouped />
        </HandyRow>
        <HandyRow>
          <ClipboardHandlingSetting descriptionMode="tooltip" grouped />
        </HandyRow>
        <HandyRow>
          <AutoSubmit descriptionMode="tooltip" grouped />
        </HandyRow>
        <HandyRow>
          <AppendTrailingSpace descriptionMode="tooltip" grouped />
        </HandyRow>
      </SettingGroup>
      <SettingGroup label={g("transcription")}>
        {TRANSCRIPTION.map((Setting, index) => (
          <HandyRow key={index}>
            <Setting descriptionMode="tooltip" grouped />
          </HandyRow>
        ))}
      </SettingGroup>
      <SettingGroup label={g("barAndOverlay")}>
        <BarSettings />
        <ToggleRow
          label={g("liveText")}
          hint={g("liveTextHint")}
          checked={settings.live_text_in_pill}
          disabled={!!pending.live_text_in_pill}
          onChange={(checked) => void setSetting("live_text_in_pill", checked)}
          testId="wb-advanced-live-text"
        />
        <HandyRow>
          <ShowOverlay descriptionMode="tooltip" grouped />
        </HandyRow>
      </SettingGroup>
      <SettingGroup label={g("keyboard")}>
        <HandyRow>
          <KeyboardImplementationSelector descriptionMode="tooltip" grouped />
        </HandyRow>
      </SettingGroup>
      <SettingGroup label={g("prompts")}>
        <HandyRow>
          <PostProcessingToggle descriptionMode="tooltip" grouped />
        </HandyRow>
        <HandyRow>
          <PostProcessingSettingsApi />
        </HandyRow>
        <HandyRow>
          <PostProcessingSettingsPrompts />
        </HandyRow>
      </SettingGroup>
      <SettingGroup label={g("history")}>
        <HandyRow>
          <HistoryLimit descriptionMode="tooltip" grouped />
        </HandyRow>
        <HandyRow>
          <RecordingRetentionPeriodSelector descriptionMode="tooltip" grouped />
        </HandyRow>
      </SettingGroup>
      <SettingGroup label={g("updates")}>
        {/* S-28: the fork setting (mirrored into Handy's at once); Handy's
            own toggle wrote only Handy's value, which the next start
            overwrote. */}
        <ToggleRow
          label={t("settingsModal.privacy.choices.updates")}
          hint={t("settingsModal.privacy.choices.updatesHint")}
          checked={settings.update_checks_enabled}
          disabled={!!pending.update_checks_enabled}
          onChange={(checked) =>
            void setSetting("update_checks_enabled", checked)
          }
          testId="wb-advanced-update-checks"
        />
        <SelectRow
          label={g("updateChannel")}
          hint={g("updateChannelHint")}
          value={settings.update_channel}
          disabled={!!pending.update_channel}
          options={["stable", "beta"].map((value) => ({
            value,
            label: t(`settingsModal.advanced.updateChannels.${value}`),
          }))}
          onChange={(value) =>
            void setSetting("update_channel", value as UpdateChannel)
          }
          testId="wb-advanced-update-channel"
          width="w-36"
        />
      </SettingGroup>
      <SettingGroup label={g("storage")}>
        <HandyRow>
          <AppDataDirectory descriptionMode="tooltip" grouped />
        </HandyRow>
        <HandyRow>
          <LogDirectory grouped />
        </HandyRow>
      </SettingGroup>
      <SettingGroup label={g("experimental")}>
        <HandyRow>
          <ExperimentalToggle descriptionMode="tooltip" grouped />
        </HandyRow>
        <ToggleRow
          label={g("debugMode")}
          hint={g("debugModeHint")}
          checked={handy?.debug_mode ?? false}
          onChange={(checked) => void updateSetting("debug_mode", checked)}
          testId="wb-advanced-debug-mode"
        />
      </SettingGroup>
      {handy?.debug_mode && (
        <div
          data-handy-content
          className="wb-handy"
          data-testid="wb-advanced-debug"
        >
          <DebugSettings
            onPreviewOnboarding={handlers.onPreviewOnboarding}
            onRunOnboarding={handlers.onRunOnboarding}
            onRunModelMigration={handlers.onRunModelMigration}
          />
        </div>
      )}
      <details
        className="group rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-5 py-4"
        data-testid="wb-advanced-fork-diagnostics"
      >
        <summary
          className={`flex cursor-pointer list-none items-center gap-2 rounded text-[15px] font-medium [&::-webkit-details-marker]:hidden ${FOCUS_RING}`}
        >
          <ChevronRight
            size={16}
            aria-hidden="true"
            className="shrink-0 text-(--wb-ink-3) transition-transform duration-(--wb-dur-fast) group-open:rotate-90 rtl:-scale-x-100"
          />
          {g("forkDiagnostics")}
        </summary>
        <div className="mt-4">
          <ForkSettingsDebug />
        </div>
      </details>
    </div>
  );
}
