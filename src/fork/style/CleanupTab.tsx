// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Style › Cleanup (S-19a): the S-12a level picker (None / Light / Medium),
 * which applies to every app and every language, under a short note on how
 * it combines with the group styles.
 */
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { CleanupLevelPicker } from "./CleanupLevelPicker";

export function CleanupTab() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div
      className="flex max-w-xl flex-col gap-6 pt-6"
      data-testid="wb-style-cleanup"
    >
      <p className="text-[15px] leading-[22px] text-(--wb-ink-2)">
        {t("style.cleanup.description")}
      </p>
      <CleanupLevelPicker />
    </div>
  );
}
