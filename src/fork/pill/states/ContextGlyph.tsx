// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-20: the small "context" glyph on the bar while a take is processed,
 * when text near the cursor helped spell it (DEC-24: a visible indicator).
 * Our own drawing: a text line with a spark (16 × 16 grid).
 */
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../../i18n";

export function ContextGlyph() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const label = t("pill.contextUsed");
  return (
    <span
      className="wb-pill-context"
      role="img"
      aria-label={label}
      title={label}
      data-testid="wb-pill-context"
    >
      <svg
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden
        focusable={false}
      >
        <path d="M2.5 5h6M2.5 8.5h4.5M2.5 12h6.5" />
        <path d="M12 2.5v3M10.5 4h3" />
        <path d="M12.5 9l.6 1.4 1.4.6-1.4.6-.6 1.4-.6-1.4-1.4-.6 1.4-.6z" />
      </svg>
    </span>
  );
}
