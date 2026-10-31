// SPDX-License-Identifier: GPL-3.0-or-later
/** "Transcribing…" / "Cleaning up…" with three dots (still under reduced motion). */
import { useTranslation } from "react-i18next";
import type { ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";

export function Working({ state }: { state: ForkPillState }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const text =
    state.phase === "cleaning_up"
      ? t("pill.cleaningUp")
      : t("pill.transcribing");
  return (
    <span
      className="wb-pill-working"
      role="status"
      aria-live="polite"
      data-testid="wb-pill-working"
    >
      <span className="wb-pill-dots" aria-hidden>
        <i />
        <i />
        <i />
      </span>
      <span className="wb-pill-text">{text}</span>
    </span>
  );
}
