// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import { AppLanguageSelector } from "@/components/settings/AppLanguageSelector";
import { ChineseScriptSetting } from "@/components/settings/ChineseScript";
import { PreferredLanguages } from "@/components/settings/PreferredLanguages";
import { ThemeSelector } from "@/components/settings/ThemeSelector";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { useSettings } from "@/hooks/useSettings";
import { isChinese } from "@/lib/utils/languageSuggestions";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { MicrophonePicker } from "./MicrophonePicker";
import { chordLabel } from "./hotkeys/model";
import { useForkSettings } from "./useForkSettings";

export function General({ onChangeHotkeys }: { onChangeHotkeys: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings: forkSettings } = useForkSettings();
  const { getSetting } = useSettings();
  const speaksChinese =
    (getSetting("spoken_languages") ?? []).some(isChinese) ||
    (getSetting("chinese_script") ?? "as_transcribed") !== "as_transcribed";
  const binding = forkSettings?.hotkeys.dictate?.[0];
  const shortcut = binding
    ? chordLabel(binding.chord)
    : t("settingsModal.hotkeys.notSet");

  return (
    <div className="flex flex-col gap-6" data-testid="wb-settings-general">
      <SettingsGroup title={t("settingsModal.general.shortcuts")}>
        <SettingContainer
          title={t("settingsModal.general.shortcutTitle")}
          description={t("settingsModal.general.shortcutHint")}
          grouped
        >
          <div className="flex flex-wrap items-center justify-end gap-2">
            <span
              className="text-sm text-(--wb-ink-2)"
              data-testid="wb-shortcut-summary"
            >
              {binding?.activation === "hold"
                ? t("settingsModal.general.holdToTalk", { shortcut })
                : shortcut}
            </span>
            <Button onClick={onChangeHotkeys} data-testid="wb-shortcuts-change">
              {t("settingsModal.general.change")}
            </Button>
          </div>
        </SettingContainer>
      </SettingsGroup>
      <section aria-labelledby="wb-general-mic" className="flex flex-col gap-3">
        <h3
          id="wb-general-mic"
          className="text-xs font-semibold tracking-wide text-(--wb-ink-3) uppercase"
        >
          {t("settingsModal.general.microphone")}
        </h3>
        <div className="rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-4">
          <MicrophonePicker />
        </div>
      </section>
      <div id="wb-general-languages" data-handy-content>
        <SettingsGroup title={t("settingsModal.general.languages")}>
          <PreferredLanguages descriptionMode="tooltip" grouped />
          {speaksChinese && (
            <ChineseScriptSetting descriptionMode="tooltip" grouped />
          )}
        </SettingsGroup>
      </div>
      <div data-handy-content>
        <SettingsGroup title={t("settingsModal.general.appearance")}>
          <AppLanguageSelector descriptionMode="tooltip" grouped />
          <ThemeSelector descriptionMode="tooltip" grouped />
        </SettingsGroup>
      </div>
    </div>
  );
}
