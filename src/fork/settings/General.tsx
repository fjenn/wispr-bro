// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import { AppLanguageSelector } from "@/components/settings/AppLanguageSelector";
import { ChineseScriptSetting } from "@/components/settings/ChineseScript";
import { PreferredLanguages } from "@/components/settings/PreferredLanguages";
import { ThemeSelector } from "@/components/settings/ThemeSelector";
import { useSettings } from "@/hooks/useSettings";
import { isChinese } from "@/lib/utils/languageSuggestions";
import { FORK_NAMESPACE } from "../i18n";
import { Button, SettingGroup } from "../ui";
import { ChevronRight } from "../ui/icons";
import { HandyRow } from "./Controls";
import { MicrophonePicker } from "./MicrophonePicker";
import { DictationKeyRow } from "./hotkeys/DictationKey";

export function General({ onChangeHotkeys }: { onChangeHotkeys: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { getSetting } = useSettings();
  const speaksChinese =
    (getSetting("spoken_languages") ?? []).some(isChinese) ||
    (getSetting("chinese_script") ?? "as_transcribed") !== "as_transcribed";

  return (
    <div className="flex flex-col gap-8" data-testid="wb-settings-general">
      <div className="flex flex-col items-start gap-2">
        <DictationKeyRow className="self-stretch" />
        <Button
          variant="ghost"
          size="sm"
          onClick={onChangeHotkeys}
          trailing={
            <ChevronRight
              size={14}
              aria-hidden="true"
              className="rtl:-scale-x-100"
            />
          }
          data-testid="wb-more-shortcuts"
        >
          {t("settingsModal.hotkeys.key.more")}
        </Button>
      </div>
      <section aria-labelledby="wb-general-mic" className="flex flex-col gap-3">
        <h3 id="wb-general-mic" className="type-micro text-(--wb-ink-3)">
          {t("settingsModal.general.microphone")}
        </h3>
        <div className="rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-5 py-4">
          <MicrophonePicker hideLegend />
        </div>
      </section>
      <div id="wb-general-languages">
        <SettingGroup label={t("settingsModal.general.languages")}>
          <HandyRow>
            <PreferredLanguages descriptionMode="inline" grouped />
          </HandyRow>
          {speaksChinese && (
            <HandyRow>
              <ChineseScriptSetting descriptionMode="tooltip" grouped />
            </HandyRow>
          )}
        </SettingGroup>
      </div>
      <SettingGroup label={t("settingsModal.general.appearance")}>
        <HandyRow>
          <AppLanguageSelector descriptionMode="inline" grouped />
        </HandyRow>
        <HandyRow>
          <ThemeSelector descriptionMode="inline" grouped />
        </HandyRow>
      </SettingGroup>
    </div>
  );
}
