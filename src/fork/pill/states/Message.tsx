// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A message or an error after a take ("No speech heard", "Microphone
 * access is off" …). Errors with a known cause offer Fix (opens the right
 * System Settings pane); "Cancelled" offers Undo for 4 s (S-06); every
 * message can be dismissed. The text is shared
 * with the VoiceOver announcement (`rust.pill.message.*`).
 */
import { useTranslation } from "react-i18next";
import { commands, type ForkPillState, type PillFix } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { AlertIcon, CloseIcon, InfoIcon } from "./icons";

/** The action button's text: Retry for setup, Undo after a cancel (S-06). */
function fixLabel(fix: PillFix, t: (key: string) => string): string {
  switch (fix) {
    case "retry_download":
      return t("pill.retry");
    case "undo":
      return t("pill.undo");
    case "view_changes":
      return t("pill.viewChanges");
    default:
      return t("pill.fix");
  }
}

export function Message({ state }: { state: ForkPillState }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  if (!state.message) return null;
  const error = state.tone === "error";
  return (
    <>
      <span
        className="wb-pill-message"
        role={error ? "alert" : "status"}
        data-testid="wb-pill-message"
      >
        {error ? <AlertIcon /> : <InfoIcon />}
        <span className="wb-pill-text">
          {t(`rust.pill.message.${state.message}`)}
        </span>
      </span>
      {state.fix && (
        <button
          type="button"
          className="wb-pill-fix"
          data-testid="wb-pill-fix"
          onClick={() => void commands.forkPillAction("fix")}
        >
          {fixLabel(state.fix, t)}
        </button>
      )}
      <button
        type="button"
        className="wb-pill-btn wb-pill-dismiss"
        data-testid="wb-pill-dismiss"
        aria-label={t("pill.dismiss")}
        title={t("pill.dismiss")}
        onClick={() => void commands.forkPillAction("dismiss")}
      >
        <CloseIcon />
      </button>
    </>
  );
}
