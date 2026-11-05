// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Style › Cleanup (S-19a; flowmatch): one banner and the three cleanup
 * levels (None / Light / Medium) as cards with a sample, saved in the fork
 * setting `cleanup_level`. It applies to every app and every language. The
 * full picker with the last dictation's result stays in Settings ›
 * Advanced (`CleanupLevelPicker`).
 */
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { CleanupLevel } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { Hero } from "../ui";
import { photos } from "@/assets/fork/art";
import { CLEANUP_LEVELS } from "./CleanupLevelPicker";
import { ChoiceCard } from "./PresetCard";
import { CLEANUP_EXAMPLES } from "./presets";

export function CleanupTab() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, error, setSetting } = useForkSettings();
  const name = useId();
  const legend = useId();
  return (
    <div className="flex flex-col gap-6 pt-6" data-testid="wb-style-cleanup">
      <Hero
        id="style-cleanup"
        image={photos.style.src}
        imageLift={1.1}
        title={t("style.cleanup.bannerTitle")}
        body={t("style.cleanup.description")}
        note={t("style.cleanup.note")}
      />
      <fieldset
        disabled={!settings || !!pending.cleanup_level}
        aria-busy={!!pending.cleanup_level}
        data-testid="fork-cleanup"
      >
        <legend id={legend} className="sr-only">
          {t("cleanup.picker.legend")}
        </legend>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {CLEANUP_LEVELS.map((level) => (
            <ChoiceCard
              key={level}
              value={level}
              name={name}
              checked={settings?.cleanup_level === level}
              onSelect={() =>
                void setSetting("cleanup_level", level as CleanupLevel)
              }
              testId={`fork-cleanup-level-${level}`}
              exampleTestId={`wb-style-cleanup-example-${level}`}
              title={t(`cleanup.levels.${level}.label`)}
              description={t(`style.cleanup.levels.${level}`)}
              note={
                level === "medium" ? t("style.cleanup.mediumNote") : undefined
              }
              example={CLEANUP_EXAMPLES[level]}
              exampleLabel={t("style.after")}
            />
          ))}
        </div>
      </fieldset>
      {error && (
        <p role="alert" className="text-[13px] text-(--wb-danger)">
          {t("cleanup.setError", {
            message: "message" in error ? error.message : error.kind,
          })}
        </p>
      )}
    </div>
  );
}
