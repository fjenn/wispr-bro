// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-02: a take started while an app holds macOS Secure Input (a password
 * field, Terminal's Secure Keyboard Entry). The bar names the holder when
 * known and says that fn+Space cannot lock now and that Esc or ✕ still
 * cancels. It takes the microphone label's place for 5 s; the full text is
 * the tooltip and the screen-reader text (the line may be cut short).
 */
import { useTranslation } from "react-i18next";
import type { ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { LockIcon } from "./icons";

export function secureInputText(
  state: ForkPillState,
  t: (key: string, opts?: Record<string, string>) => string,
): string {
  return state.holder
    ? t("pill.warning.secure_input_app", { app: state.holder })
    : t("pill.warning.secure_input");
}

export function SecureInputNotice({ state }: { state: ForkPillState }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const text = secureInputText(state, t);
  return (
    <span
      className="wb-pill-label wb-pill-warning"
      data-testid="wb-pill-secure-input"
      title={text}
    >
      <LockIcon />
      <span className="wb-pill-text">{text}</span>
    </span>
  );
}
