// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Style on iPhone: one writing style for everything. The Mac picks a style
 * per kind of app (personal, work, email, other); a keyboard cannot see
 * which app it is typing into, so the phone offers one choice and saves it
 * for every group. The cards and their examples are the Mac's
 * (`../style/PresetCard`, the "other apps" set).
 */
import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type StylePreset } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { PresetCard } from "../style/PresetCard";
import { EXAMPLES, PRESETS } from "../style/presets";
import { STYLE_GROUPS } from "../style/useStyle";
import { IosTitle } from "./parts";

const GROUP = "other" as const;

export function IosStyle() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const name = useId();
  const [initial, setInitial] = useState("A");
  useEffect(() => {
    let active = true;
    void commands
      .forkAccountName()
      .then((account) => {
        const first = account ? [...account.trim()][0] : undefined;
        if (active && first) setInitial(first.toLocaleUpperCase());
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  const override = settings?.greeting_name?.trim();
  const letter = override ? [...override][0].toLocaleUpperCase() : initial;
  const current = settings?.styles[GROUP];
  const choose = (preset: StylePreset) => {
    if (!settings) return;
    const styles = { ...settings.styles };
    for (const group of STYLE_GROUPS) {
      styles[group] = PRESETS[group].includes(preset) ? preset : styles[group];
    }
    styles[GROUP] = preset;
    void setSetting("styles", styles);
  };

  return (
    <div className="flex flex-col gap-4" data-testid="wb-style">
      <IosTitle>{t("hub.pages.style.title")}</IosTitle>
      <p className="text-[15px] leading-[21px] text-(--wb-ink-2)">
        {t("ios.style.intro")} {t("style.englishOnly")}
      </p>
      <fieldset
        disabled={!settings || !!pending.styles}
        aria-busy={!!pending.styles}
        data-testid={`wb-style-group-${GROUP}`}
      >
        <legend className="sr-only">{t("style.presetsHeading")}</legend>
        <div className="wb-ios-presets flex flex-col gap-3">
          {PRESETS[GROUP].map((preset, index) => (
            <PresetCard
              key={preset}
              preset={preset}
              name={name}
              index={index}
              checked={current === preset}
              example={EXAMPLES[GROUP].after[preset] ?? ""}
              initial={letter}
              onSelect={() => choose(preset)}
            />
          ))}
        </div>
      </fieldset>
    </div>
  );
}
