// SPDX-License-Identifier: GPL-3.0-or-later
/** A saved dictation whose audio is waiting for a usable speech model. */
import { Clock3 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";

export function WaitingRow() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <span
      data-testid="wb-history-waiting"
      role="status"
      className="flex items-center gap-1.5"
    >
      <Clock3 size={14} aria-hidden="true" className="shrink-0" />
      {t("history.waitingForModel")}
    </span>
  );
}
