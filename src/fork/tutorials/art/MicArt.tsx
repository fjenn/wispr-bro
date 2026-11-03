// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Our own drawings for the whisper lesson and the microphone quiz (S-32,
 * P-1): four generic kinds of microphone and the "about 30 cm" picture.
 * Plain line art in `currentColor` with the accent for the microphone
 * itself, so they follow the app theme; no product's shape or logo.
 * Decorative: the text next to each drawing says what it shows.
 */
import type { ComponentType, SVGProps } from "react";
import type { MicKind } from "../micTable";

const BASE: SVGProps<SVGSVGElement> = {
  viewBox: "0 0 72 72",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2.5,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
  focusable: false,
};
const MIC = "var(--wb-accent-fill)";

function BuiltIn() {
  return (
    <svg {...BASE}>
      {/* A laptop: screen, base; the mic is the dot above the screen. */}
      <rect x="14" y="16" width="44" height="30" rx="3" />
      <path d="M8 52h56l-4 5H12z" />
      <circle cx="36" cy="20" r="1.8" fill={MIC} stroke={MIC} />
      <path d="M30 30c2 3 10 3 12 0" opacity="0.5" />
    </svg>
  );
}

function WiredHeadset() {
  return (
    <svg {...BASE}>
      {/* Headband, two ear cups, a boom arm ending at the mouth, a cable. */}
      <path d="M18 38v-6a18 18 0 0 1 36 0v6" />
      <rect x="13" y="36" width="9" height="14" rx="4" />
      <rect x="50" y="36" width="9" height="14" rx="4" />
      <path d="M18 50c0 8 6 10 14 10" />
      <circle cx="35" cy="60" r="3.5" fill={MIC} stroke={MIC} />
      <path d="M54 50c4 4 4 10 2 16" strokeDasharray="2 4" />
    </svg>
  );
}

function WirelessEarbuds() {
  return (
    <svg {...BASE}>
      {/* Two earbuds with stems; the mic sits at each stem's end. */}
      <circle cx="24" cy="26" r="8" />
      <path d="M24 34v18" />
      <circle cx="24" cy="54" r="2.2" fill={MIC} stroke={MIC} />
      <circle cx="48" cy="26" r="8" />
      <path d="M48 34v18" />
      <circle cx="48" cy="54" r="2.2" fill={MIC} stroke={MIC} />
      <path d="M33 14c2-2 4-2 6 0M30 10c4-4 8-4 12 0" opacity="0.6" />
    </svg>
  );
}

function DeskMic() {
  return (
    <svg {...BASE}>
      {/* A capsule on a short stand with a round foot. */}
      <rect
        x="27"
        y="10"
        width="18"
        height="28"
        rx="9"
        fill={MIC}
        stroke={MIC}
      />
      <path d="M22 30a14 14 0 0 0 28 0" />
      <path d="M36 44v12" />
      <path d="M24 60h24" />
    </svg>
  );
}

const ART: Record<MicKind, ComponentType> = {
  built_in: BuiltIn,
  wired_headset: WiredHeadset,
  wireless_earbuds: WirelessEarbuds,
  desk_usb: DeskMic,
};

export function MicArt({
  kind,
  className,
}: {
  kind: MicKind;
  className?: string;
}) {
  const Art = ART[kind];
  return (
    <span className={className} data-testid={`wb-mic-art-${kind}`}>
      <Art />
    </span>
  );
}

/**
 * A head in profile, a dashed line and a laptop: whisper at about a
 * forearm's length (30 cm) from the built-in microphone. The label is drawn
 * by the caller (translated), not in the picture.
 */
export function WhisperDistanceArt({ className }: { className?: string }) {
  return (
    <svg
      {...BASE}
      viewBox="0 0 220 96"
      className={className}
      data-testid="wb-whisper-distance-art"
    >
      {/* Head: forehead, nose, lips, chin, back of the head. */}
      <path d="M40 18c-14 0-24 10-24 24 0 10 4 16 8 20v16h22v-8h8a4 4 0 0 0 4-4v-7l5-2-5-6c0-18-6-33-18-33z" />
      {/* Soft breath: short arcs toward the laptop. */}
      <path d="M66 48c3 2 3 6 0 8" opacity="0.7" />
      <path d="M72 45c5 4 5 10 0 14" opacity="0.45" />
      {/* The distance. */}
      <path d="M84 52h62" strokeDasharray="3 6" />
      <path d="M84 46v12M146 46v12" />
      {/* Laptop with the mic dot. */}
      <rect x="156" y="22" width="48" height="34" rx="3" />
      <path d="M150 62h60l-4 6h-52z" />
      <circle cx="180" cy="26" r="2" fill={MIC} stroke={MIC} />
    </svg>
  );
}
