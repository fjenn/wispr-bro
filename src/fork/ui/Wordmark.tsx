// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Wispr Bro wordmark (Design Phase 0b; REFERENCE-SPEC §1 "Logo row":
 * a 20 pt wordmark at the top of the sidebar). Our own glyph, five rounded
 * waveform bars from the app icon (`src-tauri/icons-fork/src/app-icon.svg`),
 * drawn in `currentColor`, beside the product name in the UI sans. No
 * Wispr Flow artwork.
 *
 * - `Wordmark`: glyph + name; `compact` shows the glyph alone (collapsed
 *   sidebar), still named for screen readers.
 * - `BrandGlyph`: the bars alone, for loading cards and the About page.
 *
 * The name comes from `rust.appName`, the one place it is spelled.
 */
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";

/** The five bars on a 20 × 20 grid. */
const BARS = [
  { x: 2, h: 6 },
  { x: 5.75, h: 11 },
  { x: 9.5, h: 15 },
  { x: 13.25, h: 9 },
  { x: 17, h: 5 },
] as const;

export function BrandGlyph({
  size = 20,
  className = "",
}: {
  size?: number;
  className?: string;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 21 20"
      aria-hidden="true"
      focusable="false"
      className={`shrink-0 ${className}`}
    >
      {BARS.map((b) => (
        <rect
          key={b.x}
          x={b.x}
          y={(20 - b.h) / 2}
          width="2.25"
          height={b.h}
          rx="1.125"
          fill="currentColor"
        />
      ))}
    </svg>
  );
}

export interface WordmarkProps {
  /** Glyph only (collapsed sidebar). */
  compact?: boolean;
  /** Glyph height in pt; the name scales with it (20 → 17 px text). */
  size?: number;
  className?: string;
  testId?: string;
}

export function Wordmark({
  compact = false,
  size = 20,
  className = "",
  testId = "wb-wordmark",
}: WordmarkProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const name = t("rust.appName");
  return (
    <span
      data-testid={testId}
      role={compact ? "img" : undefined}
      aria-label={compact ? name : undefined}
      className={`inline-flex items-center gap-2 text-(--wb-ink) select-none ${className}`}
    >
      <BrandGlyph size={size} />
      {!compact && (
        <span
          className="font-semibold tracking-[-0.01em] whitespace-nowrap"
          style={{
            fontSize: `${Math.round(size * 0.85)}px`,
            lineHeight: `${size}px`,
          }}
        >
          {name}
        </span>
      )}
    </span>
  );
}
