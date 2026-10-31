// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Held (push to talk) and hands-free: the waveform, with the microphone
 * name for the first 1.5 s or a "stops soon" warning in its place.
 * Hands-free adds ✕ (cancel) and ✓ (finish) around it.
 */
import { useTranslation } from "react-i18next";
import { commands, type ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { usePillLevels } from "../usePill";
import { CheckIcon, CloseIcon, MicIcon } from "./icons";

function Waveform({ bars }: { bars: number[] }) {
  return (
    <div className="wb-pill-wave" data-testid="wb-pill-wave" aria-hidden>
      {bars.map((v, i) => (
        <i
          key={i}
          style={{
            height: `${Math.round(Math.max(3, Math.min(18, 3 + Math.pow(v, 0.7) * 15)))}px`,
          }}
        />
      ))}
    </div>
  );
}

export function Listening({ state }: { state: ForkPillState }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const bars = usePillLevels(true);
  const locked = state.phase === "locked";
  const label = state.warning
    ? t(`pill.warning.${state.warning}`)
    : state.mic
      ? t("pill.mic", { name: state.mic })
      : null;

  const middle = label ? (
    <span
      className={`wb-pill-label${state.warning ? " wb-pill-warning" : ""}`}
      data-testid={state.warning ? "wb-pill-warning" : "wb-pill-mic"}
    >
      {!state.warning && <MicIcon />}
      <span className="wb-pill-text">{label}</span>
    </span>
  ) : (
    <Waveform bars={bars} />
  );

  return (
    <>
      {locked && (
        <button
          type="button"
          className="wb-pill-btn wb-pill-cancel"
          data-testid="wb-pill-cancel"
          aria-label={t("pill.cancel")}
          title={t("pill.cancel")}
          onClick={() => void commands.forkPillAction("cancel")}
        >
          <CloseIcon />
        </button>
      )}
      <span className="wb-pill-middle" role="status" aria-live="polite">
        <span className="wb-sr-only">{label ?? t("pill.listening")}</span>
        {middle}
      </span>
      {locked && (
        <button
          type="button"
          className="wb-pill-btn wb-pill-finish"
          data-testid="wb-pill-finish"
          aria-label={t("pill.finish")}
          title={t("pill.finish")}
          onClick={() => void commands.forkPillAction("stop")}
        >
          <CheckIcon />
        </button>
      )}
    </>
  );
}
