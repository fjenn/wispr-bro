// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-02: the dictation shortcut stopped working (Accessibility off, the
 * app's grant no longer matching after an update, or the keyboard listener
 * stopped). Fix opens the Fix sheet in the Hub; the message stays until the
 * shortcut works again, then "Your shortcut works again" replaces it.
 * The text is shared with the VoiceOver announcement
 * (`rust.pill.message.shortcut_*`).
 */
import { useTranslation } from "react-i18next";
import { commands, type ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { AlertIcon, CloseIcon } from "./icons";

export function isShortcutLost(state: ForkPillState): boolean {
  return (
    state.phase === "message" &&
    (state.message === "shortcut_lost" || state.message === "shortcut_stopped")
  );
}

export function ShortcutLost({ state }: { state: ForkPillState }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const detail =
    state.message === "shortcut_stopped"
      ? t("pill.shortcutDetail.stopped")
      : t("pill.shortcutDetail.permission");
  return (
    <>
      <span
        className="wb-pill-message"
        role="alert"
        data-testid="wb-pill-shortcut-lost"
        title={detail}
      >
        <AlertIcon />
        <span className="wb-pill-text">
          {t(`rust.pill.message.${state.message}`)}
        </span>
        <span className="wb-sr-only">{detail}</span>
      </span>
      <button
        type="button"
        className="wb-pill-fix"
        data-testid="wb-pill-fix"
        onClick={() => void commands.forkPillAction("fix")}
      >
        {t("pill.fix")}
      </button>
      <button
        type="button"
        className="wb-pill-disc wb-pill-dismiss"
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
