// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The card (S-07a): the text of a take that was recorded before the speech
 * model was ready. It is never pasted late; Copy puts it on the clipboard
 * (the only write, no restore), ✕ or Esc dismisses it. The text is also in
 * History, with any earlier queued takes ("2 more in History").
 */
import { useTranslation } from "react-i18next";
import { commands, type ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { CheckIcon, CloseIcon } from "./icons";

export function Card({ state }: { state: ForkPillState }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  if (state.text == null) return null;
  return (
    <>
      <div className="wb-pill-card" data-testid="wb-pill-card">
        <span className="wb-pill-card-label" role="status" aria-live="polite">
          {state.more > 0
            ? t("pill.cardMore", { count: state.more })
            : t("pill.cardReady")}
        </span>
        <span
          className="wb-pill-card-text"
          data-testid="wb-pill-card-text"
          title={state.text}
        >
          {state.text}
        </span>
      </div>
      <button
        type="button"
        className="wb-pill-fix wb-pill-copy"
        data-testid="wb-pill-copy"
        data-copied={state.copied || undefined}
        onClick={() => void commands.forkPillAction("copy")}
      >
        {state.copied ? (
          <>
            <CheckIcon />
            <span>{t("pill.copied")}</span>
          </>
        ) : (
          t("pill.copy")
        )}
      </button>
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
