// SPDX-License-Identifier: GPL-3.0-or-later
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
import { UpdateChecksToggle } from "@/components/settings/UpdateChecksToggle";
import { VadBackendSelector } from "@/components/settings/VadBackendSelector";
import { VoiceActivityDetection } from "@/components/settings/VoiceActivityDetection";
import { DebugSettings } from "@/components/settings/debug/DebugSettings";
import { KeyboardImplementationSelector } from "@/components/settings/debug/KeyboardImplementationSelector";
import { LogDirectory } from "@/components/settings/debug/LogDirectory";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { useSettings } from "@/hooks/useSettings";
import { HandyAppHandlersContext } from "../hub/HandySettingsPage";
import type { UpdateChannel } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { CleanupLevelPicker } from "../style/CleanupLevelPicker";
import { AiEngine } from "./advanced/AiEngine";
import { SpeechModels } from "./advanced/Models";
import { BarSettings } from "./BarSettings";
import { ForkSelect, ForkToggle, Panel } from "./Controls";
import { ForkSettingsDebug } from "./ForkSettingsDebug";
import { useForkSettings } from "./useForkSettings";

export function Advanced() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const { settings: handy, updateSetting } = useSettings();
  const handlers = useContext(HandyAppHandlersContext);
  if (!settings) return null;

  return (
    <div className="flex flex-col gap-6" data-testid="wb-settings-advanced">
      <Panel title={t("speechModels.title")} testId="wb-advanced-models">
        <SpeechModels />
      </Panel>
      <Panel title={t("aiEngine.title")} testId="wb-advanced-ai">
        <AiEngine />
      </Panel>
      <Panel title={t("cleanup.title")} testId="wb-advanced-cleanup">
        <CleanupLevelPicker />
      </Panel>
      <div data-handy-content>
        <SettingsGroup title={t("settingsModal.advanced.output")}>
          <PasteMethodSetting descriptionMode="tooltip" grouped />
          <TypingToolSetting descriptionMode="tooltip" grouped />
          <ClipboardHandlingSetting descriptionMode="tooltip" grouped />
          <AutoSubmit descriptionMode="tooltip" grouped />
          <AppendTrailingSpace descriptionMode="tooltip" grouped />
        </SettingsGroup>
      </div>
      <div data-handy-content>
        <SettingsGroup title={t("settingsModal.advanced.transcription")}>
          <ShortcutActivationSetting descriptionMode="tooltip" grouped />
          <ChannelSelector descriptionMode="tooltip" grouped />
          <VoiceActivityDetection descriptionMode="tooltip" grouped />
          <TranslateToEnglish descriptionMode="tooltip" grouped />
          <LongFormChunking descriptionMode="tooltip" grouped />
          <FillerWordRemoval descriptionMode="tooltip" grouped />
          <CustomWords descriptionMode="tooltip" grouped />
          <ModelUnloadTimeoutSetting descriptionMode="tooltip" grouped />
          <AccelerationSelector descriptionMode="tooltip" grouped />
          <LazyStreamClose descriptionMode="tooltip" grouped />
          <VadBackendSelector descriptionMode="tooltip" grouped />
          <LanguageDetection descriptionMode="tooltip" grouped />
        </SettingsGroup>
      </div>
      <SettingsGroup title={t("settingsModal.advanced.barAndOverlay")}>
        <div className="p-4">
          <BarSettings />
          <ForkToggle
            label={t("settingsModal.advanced.liveText")}
            hint={t("settingsModal.advanced.liveTextHint")}
            checked={settings.live_text_in_pill}
            disabled={!!pending.live_text_in_pill}
            onChange={(checked) =>
              void setSetting("live_text_in_pill", checked)
            }
            testId="wb-advanced-live-text"
          />
        </div>
        <div data-handy-content>
          <ShowOverlay descriptionMode="tooltip" grouped />
        </div>
      </SettingsGroup>
      <div data-handy-content>
        <SettingsGroup title={t("settingsModal.advanced.keyboard")}>
          <KeyboardImplementationSelector descriptionMode="tooltip" grouped />
        </SettingsGroup>
      </div>
      <div data-handy-content>
        <SettingsGroup title={t("settingsModal.advanced.prompts")}>
          <PostProcessingToggle descriptionMode="tooltip" grouped />
          <PostProcessingSettingsApi />
          <PostProcessingSettingsPrompts />
        </SettingsGroup>
      </div>
      <div data-handy-content>
        <SettingsGroup title={t("settingsModal.advanced.history")}>
          <HistoryLimit descriptionMode="tooltip" grouped />
          <RecordingRetentionPeriodSelector descriptionMode="tooltip" grouped />
        </SettingsGroup>
      </div>
      <SettingsGroup title={t("settingsModal.advanced.updates")}>
        <div data-handy-content>
          <UpdateChecksToggle descriptionMode="tooltip" grouped />
        </div>
        <div className="px-4">
          <ForkSelect
            label={t("settingsModal.advanced.updateChannel")}
            hint={t("settingsModal.advanced.updateChannelHint")}
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
          />
        </div>
      </SettingsGroup>
      <div data-handy-content>
        <SettingsGroup title={t("settingsModal.advanced.storage")}>
          <AppDataDirectory descriptionMode="tooltip" grouped />
          <LogDirectory grouped />
        </SettingsGroup>
      </div>
      <SettingsGroup title={t("settingsModal.advanced.experimental")}>
        <div data-handy-content>
          <ExperimentalToggle descriptionMode="tooltip" grouped />
        </div>
        <div className="px-4">
          <ForkToggle
            label={t("settingsModal.advanced.debugMode")}
            hint={t("settingsModal.advanced.debugModeHint")}
            checked={handy?.debug_mode ?? false}
            onChange={(checked) => void updateSetting("debug_mode", checked)}
            testId="wb-advanced-debug-mode"
          />
        </div>
      </SettingsGroup>
      {handy?.debug_mode && (
        <div data-handy-content data-testid="wb-advanced-debug">
          <DebugSettings
            onPreviewOnboarding={handlers.onPreviewOnboarding}
            onRunOnboarding={handlers.onRunOnboarding}
            onRunModelMigration={handlers.onRunModelMigration}
          />
        </div>
      )}
      <details
        className="rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-4"
        data-testid="wb-advanced-fork-diagnostics"
      >
        <summary className="cursor-pointer text-sm font-medium">
          {t("settingsModal.advanced.forkDiagnostics")}
        </summary>
        <div className="mt-4">
          <ForkSettingsDebug />
        </div>
      </details>
    </div>
  );
}
