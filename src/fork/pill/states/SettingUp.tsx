// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Setting up… 40%" (S-07a): a take was recorded before the first speech
 * model finished downloading. The audio is kept; the text comes to the
 * card as soon as the model is ready. ✕ hides the bar (the take still
 * arrives). No model name is shown.
 */
import { useTranslation } from "react-i18next";
import { commands, type ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { CloseIcon } from "./icons";

function Ring({ percent }: { percent: number | null }) {
  const r = 6.25;
  const c = 2 * Math.PI * r;
  const done = percent == null ? 0.25 : Math.max(0.02, percent / 100);
  return (
    <svg
      className={`wb-pill-ring${percent == null ? " wb-pill-ring-spin" : ""}`}
      viewBox="0 0 16 16"
      aria-hidden
      focusable={false}
    >
      <circle cx="8" cy="8" r={r} className="wb-pill-ring-track" />
      <circle
        cx="8"
        cy="8"
        r={r}
        className="wb-pill-ring-fill"
        strokeDasharray={`${done * c} ${c}`}
        transform="rotate(-90 8 8)"
      />
    </svg>
  );
}

export function SettingUp({ state }: { state: ForkPillState }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const percent = state.progress ?? null;
  const text =
    percent == null
      ? t("pill.settingUp")
      : t("pill.settingUpPercent", { percent });
  return (
    <>
      <span
        className="wb-pill-setup"
        role="status"
        aria-live="polite"
        data-testid="wb-pill-setup"
      >
        <Ring percent={percent} />
        <span className="wb-pill-text" title={t("pill.settingUpHint")}>
          {text}
        </span>
        <span className="wb-sr-only">{t("pill.settingUpHint")}</span>
      </span>
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
