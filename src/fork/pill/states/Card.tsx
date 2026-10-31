// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The card: a take's text that is not pasted, with Copy and ✕ (Esc
 * dismisses it too, observed by the backend from the hotkey tap).
 *
 * - S-07a: a take recorded before the speech model was ready. It is never
 *   pasted late; the label says it is ready ("2 more in History").
 * - S-05: a take that could not be pasted (no text field, focus moved, a
 *   password field, sleep …); the label says why (`rust.pill.card.<reason>`,
 *   shared with the VoiceOver announcement). A password field's text never
 *   reaches this page: the card says it is hidden, Copy still copies it.
 *
 * Copy puts the text on the clipboard (the only write, no restore). The
 * text is also in History.
 */
import { useTranslation } from "react-i18next";
import { commands, type ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { CheckIcon, CloseIcon } from "./icons";

export function Card({ state }: { state: ForkPillState }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const reason = state.card_reason;
  const hidden = state.text == null;
  if (hidden && reason !== "secure_field") return null;
  let label: string;
  if (reason) label = t(`rust.pill.card.${reason}`);
  else if (state.more > 0) label = t("pill.cardMore", { count: state.more });
  else label = t("pill.cardReady");
  return (
    <>
      <div
        className="wb-pill-card"
        data-testid="wb-pill-card"
        data-reason={reason ?? "queued"}
      >
        <span
          className="wb-pill-card-label"
          data-testid="wb-pill-card-label"
          role="status"
          aria-live="polite"
          title={label}
        >
          {label}
        </span>
        <span
          className="wb-pill-card-text"
          data-testid="wb-pill-card-text"
          data-hidden={hidden || undefined}
          title={state.text ?? undefined}
        >
          {hidden ? t("pill.card.hidden") : state.text}
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
