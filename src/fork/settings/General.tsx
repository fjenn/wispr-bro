// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import { AppLanguageSelector } from "@/components/settings/AppLanguageSelector";
import { ChineseScriptSetting } from "@/components/settings/ChineseScript";
import { PreferredLanguages } from "@/components/settings/PreferredLanguages";
import { ThemeSelector } from "@/components/settings/ThemeSelector";
import { useSettings } from "@/hooks/useSettings";
import { isChinese } from "@/lib/utils/languageSuggestions";
import { FORK_NAMESPACE } from "../i18n";
import { Button, SettingGroup, SettingRow } from "../ui";
import { HandyRow } from "./Controls";
import { MicrophonePicker } from "./MicrophonePicker";
import { ChordKeys } from "./hotkeys/ChordKeys";
import { useForkSettings } from "./useForkSettings";

export function General({ onChangeHotkeys }: { onChangeHotkeys: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings: forkSettings } = useForkSettings();
  const { getSetting } = useSettings();
  const speaksChinese =
    (getSetting("spoken_languages") ?? []).some(isChinese) ||
    (getSetting("chinese_script") ?? "as_transcribed") !== "as_transcribed";
  const binding = forkSettings?.hotkeys.dictate?.[0];

  return (
    <div className="flex flex-col gap-8" data-testid="wb-settings-general">
      <SettingGroup>
        <SettingRow
          title={t("settingsModal.general.shortcutTitle")}
          description={t("settingsModal.general.shortcutHint")}
          testId="wb-shortcut-row"
          control={
            <>
              <span
                className="flex items-center gap-1.5 text-[14px] text-(--wb-ink-2)"
                data-testid="wb-shortcut-summary"
              >
                {binding ? (
                  binding.activation === "hold" ? (
                    <HoldSummary chord={binding.chord} />
                  ) : (
                    <ChordKeys chord={binding.chord} />
                  )
                ) : (
                  t("settingsModal.hotkeys.notSet")
                )}
              </span>
              <Button
                variant="outline"
                size="sm"
                onClick={onChangeHotkeys}
                data-testid="wb-shortcuts-change"
              >
                {t("settingsModal.general.change")}
              </Button>
            </>
          }
        />
      </SettingGroup>
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

/** "Hold fn to talk", with the keys drawn as key caps. */
function HoldSummary({ chord }: { chord: string }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const marker = "\u0000";
  const [before, after = ""] = t("settingsModal.general.holdToTalk", {
    shortcut: marker,
  }).split(marker);
  return (
    <>
      {before.trim() && <span>{before}</span>}
      <ChordKeys chord={chord} />
      {after.trim() && <span>{after}</span>}
    </>
  );
}
