// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Your voice profile" progress (S-15a, C-33): fills with the lifetime word
 * count and unlocks at 1,700 words (DEC-50). Once unlocked it links to
 * Insights, where S-25 shows the profile ("Your voice").
 *
 * The bar is Handy's `ProgressLine` (decorative, R2 C-33) recoloured with
 * the Hub accent; the accessible progress bar is the wrapper, with the
 * count as its value text.
 */
import { useTranslation } from "react-i18next";
import { ProgressLine } from "@/components/shared/ProgressLine";
import { FORK_NAMESPACE } from "../i18n";
import { navigate } from "../hub/nav";
import { Button } from "../ui";
import { formatCount } from "./StatsRail";

/** Words that unlock the voice profile (DEC-50). */
export const VOICE_PROFILE_GOAL = 1_700;

export function VoiceProfileBar({ totalWords }: { totalWords: number | null }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const words = Math.max(0, totalWords ?? 0);
  const current = Math.min(words, VOICE_PROFILE_GOAL);
  const unlocked = words >= VOICE_PROFILE_GOAL;
  const goal = formatCount(VOICE_PROFILE_GOAL, i18n.language);
  const progressText = t("home.voiceProfile.progress", {
    current: formatCount(current, i18n.language),
    goal,
    count: VOICE_PROFILE_GOAL,
  });

  return (
    <section
      aria-labelledby="wb-voice-profile-title"
      data-testid="wb-voice-profile"
      data-unlocked={unlocked}
      className="flex flex-col gap-2 border-t border-(--wb-line) px-6 pt-5 pb-6"
    >
      <h2
        id="wb-voice-profile-title"
        className="type-card-title text-(--wb-ink)"
      >
        {t("home.voiceProfile.title")}
      </h2>
      <p className="text-[13px] leading-[18px] text-(--wb-ink-2)">
        {unlocked
          ? t("home.voiceProfile.unlocked")
          : t("home.voiceProfile.locked", {
              goal,
              count: VOICE_PROFILE_GOAL,
            })}
      </p>
      <div
        className="mt-1 flex items-center gap-3"
        role="progressbar"
        data-testid="wb-voice-profile-bar"
        aria-labelledby="wb-voice-profile-title"
        aria-valuemin={0}
        aria-valuemax={VOICE_PROFILE_GOAL}
        aria-valuenow={current}
        aria-valuetext={progressText}
      >
        <ProgressLine
          value={current / VOICE_PROFILE_GOAL}
          className="h-1 min-w-12 flex-1 rounded-full bg-(--wb-line-2)! [&>i]:rounded-full [&>i]:bg-(--wb-accent)!"
        />
        {/* Read through aria-valuetext; shown for sighted users. */}
        <span
          aria-hidden="true"
          className="shrink-0 text-[12px] font-medium text-(--wb-ink) tabular-nums"
        >
          {progressText}
        </span>
      </div>
      {unlocked && (
        <div className="mt-2">
          <Button
            size="sm"
            variant="secondary"
            data-testid="wb-voice-profile-open"
            onClick={() => navigate("insights", "voice")}
          >
            {t("home.voiceProfile.open")}
          </Button>
        </div>
      )}
    </section>
  );
}
