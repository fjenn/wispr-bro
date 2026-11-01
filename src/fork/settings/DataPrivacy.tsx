// SPDX-License-Identifier: GPL-3.0-or-later
import { ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";

export function DataPrivacy() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div
      className="flex min-h-64 flex-col items-center justify-center gap-3 rounded-(--wb-radius-card) border border-dashed border-(--wb-line-strong) p-8 text-center"
      data-testid="wb-settings-privacy"
    >
      <ShieldCheck size={28} aria-hidden="true" className="text-(--wb-ink-2)" />
      <h3 className="text-base font-semibold">
        {t("settingsModal.privacy.placeholderTitle")}
      </h3>
      <p className="max-w-md text-sm text-(--wb-ink-2)">
        {t("settingsModal.privacy.placeholderBody")}
      </p>
    </div>
  );
}
