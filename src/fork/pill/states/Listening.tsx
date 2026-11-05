// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Held (push to talk) and hands-free: the waveform; hands-free adds ✕
 * (cancel, a dark disc) and ✓ (finish, a white disc) around it. The
 * microphone name, "Sends automatically" and warnings float above the
 * capsule in the label pill (`Label.tsx`); screen readers hear them here,
 * in the capsule's status line.
 */
import { useTranslation } from "react-i18next";
import { commands, type ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { usePillLevels } from "../usePill";
import { CheckIcon, CloseIcon } from "./icons";
import type { BarLabelContent } from "./Label";
import { Waveform } from "./Waveform";

export function Listening({
  state,
  label,
}: {
  state: ForkPillState;
  label: BarLabelContent | null;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const levels = usePillLevels(true);
  const locked = state.phase === "locked";

  return (
    <div className="wb-pill-listening" data-locked={locked || undefined}>
      {locked && (
        <button
          type="button"
          className="wb-pill-disc wb-pill-cancel"
          data-testid="wb-pill-cancel"
          aria-label={t("pill.cancel")}
          title={t("pill.cancel")}
          onClick={() => void commands.forkPillAction("cancel")}
        >
          <CloseIcon />
        </button>
      )}
      <span className="wb-pill-middle" role="status" aria-live="polite">
        <span className="wb-sr-only">{label?.text ?? t("pill.listening")}</span>
        <Waveform levels={levels} />
      </span>
      {locked && (
        <button
          type="button"
          className="wb-pill-disc wb-pill-finish"
          data-testid="wb-pill-finish"
          aria-label={t("pill.finish")}
          title={t("pill.finish")}
          onClick={() => void commands.forkPillAction("stop")}
        >
          <CheckIcon />
        </button>
      )}
    </div>
  );
}
