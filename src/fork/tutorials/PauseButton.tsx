// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The pause/play control of a looping scene or ticker (WCAG 2.2.2; Design
 * W5): a small icon button that sits inside the scene, like a player's
 * control, instead of a text link under it. Its name says what a press
 * does ("Pause animation" / "Play animation"); `aria-pressed` says whether
 * the loop is paused. Hidden under Reduce Motion by its callers (nothing
 * moves then).
 */
import { IconButton } from "../ui";
import { Pause, Play } from "../ui/icons";

export function PauseButton({
  paused,
  onToggle,
  pauseLabel,
  playLabel,
  testId,
  tone = "light",
  className = "",
}: {
  paused: boolean;
  onToggle: () => void;
  pauseLabel: string;
  playLabel: string;
  testId: string;
  /** `hud` on a dark stage. */
  tone?: "light" | "hud";
  className?: string;
}) {
  const label = paused ? playLabel : pauseLabel;
  return (
    <IconButton
      aria-label={label}
      title={label}
      aria-pressed={paused}
      variant={tone === "hud" ? "hud" : "ghost"}
      size="sm"
      round
      onClick={onToggle}
      data-testid={testId}
      className={className}
    >
      {paused ? <Play size={14} /> : <Pause size={14} />}
    </IconButton>
  );
}
