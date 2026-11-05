// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Advanced › "Translate to English" (audit OT2). Handy's row
 * stayed on for every model, but only models that can translate (Whisper,
 * Canary and a few others: `supports_translation` in Handy's catalog) do
 * anything with it (`cpp_translation_task` in Handy's transcription
 * manager). The default models (Parakeet) cannot, so the row is off and
 * disabled for them and says how to get it.
 */
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import { useModelStore } from "@/stores/modelStore";
import { FORK_NAMESPACE } from "../../i18n";
import { ToggleRow } from "../Controls";

export function TranslateRow() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { t: handy } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const currentModel = useModelStore((s) => s.currentModel);
  const models = useModelStore((s) => s.models);

  useEffect(() => {
    if (!useModelStore.getState().initialized) {
      void useModelStore.getState().initialize();
    }
  }, []);

  // The store can hold null before the backend answers.
  const model = (models ?? []).find((m) => m.id === currentModel);
  const supported = model?.supports_translation ?? false;
  const on = getSetting("translate_to_english") ?? false;
  const hint = supported
    ? handy("settings.advanced.translateToEnglish.description")
    : model
      ? t("settingsModal.advanced.translateUnsupported", { model: model.name })
      : t("settingsModal.advanced.translateNoModel");

  return (
    <ToggleRow
      label={handy("settings.advanced.translateToEnglish.label")}
      hint={hint}
      checked={supported && on}
      disabled={!supported || isUpdating("translate_to_english")}
      onChange={(checked) =>
        void updateSetting("translate_to_english", checked)
      }
      testId="wb-advanced-translate"
    />
  );
}
