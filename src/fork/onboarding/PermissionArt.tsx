// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The pictures beside each permission row (S-29a): the macOS prompt and the
 * System Settings pane, one pair for macOS 13-15 and one for macOS 26's
 * redesigned settings. Inlined so they take the app's theme (`art.css`).
 */
import { version } from "@tauri-apps/plugin-os";
import "./art.css";
import accessibilityPane13 from "./art/accessibility-pane-13.svg?raw";
import accessibilityPane26 from "./art/accessibility-pane-26.svg?raw";
import accessibilityPrompt13 from "./art/accessibility-prompt-13.svg?raw";
import accessibilityPrompt26 from "./art/accessibility-prompt-26.svg?raw";
import microphonePane13 from "./art/microphone-pane-13.svg?raw";
import microphonePane26 from "./art/microphone-pane-26.svg?raw";
import microphonePrompt13 from "./art/microphone-prompt-13.svg?raw";
import microphonePrompt26 from "./art/microphone-prompt-26.svg?raw";

export type PermissionKind = "microphone" | "accessibility";
export type ArtKind = "prompt" | "pane";

const ART = {
  legacy: {
    microphone: { prompt: microphonePrompt13, pane: microphonePane13 },
    accessibility: { prompt: accessibilityPrompt13, pane: accessibilityPane13 },
  },
  current: {
    microphone: { prompt: microphonePrompt26, pane: microphonePane26 },
    accessibility: { prompt: accessibilityPrompt26, pane: accessibilityPane26 },
  },
} as const;

/** macOS 26 and later get the redesigned pictures. */
export function artFamily(osVersion: string): "legacy" | "current" {
  const major = Number.parseInt(osVersion.split(".")[0] ?? "", 10);
  return Number.isFinite(major) && major >= 26 ? "current" : "legacy";
}

function osVersion(): string {
  try {
    return version();
  } catch {
    return "";
  }
}

/** Our own static SVG (no user data), with its comment line removed. */
function svgMarkup(kind: PermissionKind, art: ArtKind): string {
  return ART[artFamily(osVersion())][kind][art].replace(/<!--.*?-->\s*/s, "");
}

export function PermissionArt({
  kind,
  art,
  label,
}: {
  kind: PermissionKind;
  art: ArtKind;
  label: string;
}) {
  return (
    <div
      role="img"
      aria-label={label}
      data-testid={`wb-permission-art-${kind}-${art}`}
      data-family={artFamily(osVersion())}
      className="aspect-[16/10] w-full overflow-hidden rounded-lg border border-(--wb-line) [&>svg]:block [&>svg]:h-full [&>svg]:w-full"
      dangerouslySetInnerHTML={{ __html: svgMarkup(kind, art) }}
    />
  );
}
