// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Transcribing…" / "Cleaning up…" (Design W4): a split capsule, the text
 * on a dark segment and a spinner on the black one (still under Reduce
 * Motion). S-20: the context glyph after the text when nearby text helped.
 */
import { useTranslation } from "react-i18next";
import type { ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { ContextGlyph } from "./ContextGlyph";

/** Eight short spokes; the brightest leads (our own drawing). */
function Spinner() {
  return (
    <svg
      className="wb-pill-spinner"
      viewBox="0 0 16 16"
      aria-hidden
      focusable={false}
    >
      {Array.from({ length: 8 }, (_, i) => (
        <line
          key={i}
          x1="8"
          y1="2"
          x2="8"
          y2="4.6"
          transform={`rotate(${i * 45} 8 8)`}
          style={{ opacity: 0.25 + (0.75 * i) / 7 }}
          className={i % 2 ? "wb-pill-spoke-a" : "wb-pill-spoke-b"}
        />
      ))}
    </svg>
  );
}

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
      <span className="wb-pill-working-text">
        <span className="wb-pill-text">{text}</span>
        {state.context && <ContextGlyph />}
      </span>
      <span className="wb-pill-working-spin">
        <Spinner />
      </span>
    </span>
  );
}
