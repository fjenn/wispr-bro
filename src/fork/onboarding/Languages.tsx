// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Languages (S-29a; R5 puts it in S-29b, pulled forward because cleanup
 * removes far fewer fillers without it: S-12a, nl/fr 95 % → 67 %).
 * Preselected from macOS (Handy's `suggestedLanguages`, English always
 * included); the picker is Handy's (`LanguagePicker`, MIT, reused by
 * import). Continue saves Handy's `spoken_languages` (and the Chinese
 * script when Chinese is picked) and asks the fork to re-pick the speech
 * model. No model is named here.
 */
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  LanguageOptions,
  LanguageSearchField,
  useLanguagePicker,
} from "@/components/shared/LanguagePicker";
import { useSettings } from "@/hooks/useSettings";
import { LANGUAGES, languageList } from "@/lib/constants/languages";
import { isWeakLanguage } from "@/lib/models/catalog";
import {
  isChinese,
  suggestedLanguages,
  suggestedScript,
  type ChineseScriptChoice,
} from "@/lib/utils/languageSuggestions";
import type { ChineseScript } from "@/bindings";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

const SCRIPT_SETTING: Record<ChineseScriptChoice, ChineseScript> = {
  "zh-Hans": "simplified",
  "zh-Hant": "traditional_tw",
  "zh-HK": "traditional_hk",
};

export function Languages({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings, updateSetting } = useSettings();
  const saved = settings?.spoken_languages ?? [];
  const [languages, setLanguages] = useState<string[]>(() =>
    saved.length ? saved : suggestedLanguages(),
  );
  // Handy's settings may arrive after the first render: seed from them
  // until the user changes something.
  const touched = useRef(false);
  const savedKey = saved.join(",");
  useEffect(() => {
    if (!touched.current && savedKey) setLanguages(savedKey.split(","));
  }, [savedKey]);
  const [busy, setBusy] = useState(false);
  const picker = useLanguagePicker({
    languages,
    onChange: (next) => {
      touched.current = true;
      setLanguages(next);
    },
    keepOne: true,
  });
  const ui = i18n.language;
  const weak = languages.filter(isWeakLanguage);

  const save = async () => {
    if (preview) {
      onContinue();
      return;
    }
    setBusy(true);
    await updateSetting("spoken_languages", languages);
    if (languages.some(isChinese) && !saved.some(isChinese)) {
      await updateSetting("chinese_script", SCRIPT_SETTING[suggestedScript()]);
    }
    // S-07b: a language change re-picks the model in the background.
    void commands.forkSpeechSetupRefresh();
    setBusy(false);
    onContinue();
  };

  return (
    <div className="mx-auto flex w-full max-w-[600px] flex-col items-center gap-5">
      <div className="text-center">
        <h1 className="m-0 font-(--wb-font-display) text-3xl font-semibold text-balance">
          {t("onboardingFlow.languages.title")}
        </h1>
        <p className="mt-2 mb-0 text-sm leading-6 text-pretty text-(--wb-ink-2)">
          {t("onboardingFlow.languages.subtitle")}
        </p>
      </div>
      <div
        className="flex w-full flex-col overflow-hidden rounded-(--wb-radius-md) border border-(--wb-line-strong) bg-(--wb-surface) text-start"
        data-testid="wb-onboarding-languages"
      >
        <LanguageSearchField
          picker={picker}
          className="border-b border-(--wb-line)"
          inputClassName="min-w-[120px] text-sm"
        />
        <div
          role="group"
          aria-label={t("onboardingFlow.languages.listLabel")}
          className="grid max-h-[220px] grid-cols-[repeat(auto-fill,minmax(150px,1fr))] content-start gap-[2px] overflow-y-auto p-1.5"
        >
          <LanguageOptions
            picker={picker}
            noMatch={t("onboardingFlow.languages.noMatch", {
              query: picker.query.trim(),
            })}
            emptyClassName="p-5"
          />
        </div>
      </div>
      <p
        className="m-0 text-center text-xs leading-5 text-(--wb-ink-2)"
        data-testid="wb-onboarding-languages-note"
      >
        {weak.length
          ? t("onboardingFlow.languages.weak", {
              count: weak.length,
              languages: languageList(weak, ui),
            })
          : t("onboardingFlow.languages.count", { count: LANGUAGES.length })}
      </p>
      <Button
        variant="primary"
        disabled={busy || !languages.length}
        onClick={() => void save()}
        data-testid="wb-languages-continue"
      >
        {t("onboardingFlow.continue")}
      </Button>
    </div>
  );
}
