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
      className="flex flex-col gap-3 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) px-5 py-4"
    >
      <h2
        id="wb-voice-profile-title"
        className="font-(family-name:--wb-font-display) text-[17px] leading-tight font-medium text-(--wb-ink)"
      >
        {t("home.voiceProfile.title")}
      </h2>
      <div
        className="flex flex-col gap-1.5"
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
          className="h-2 bg-(--wb-fill-active)! [&>i]:bg-(--wb-accent)!"
        />
        {/* Read through aria-valuetext; shown for sighted users. */}
        <span
          aria-hidden="true"
          className="text-[12.5px] font-medium text-(--wb-ink) tabular-nums"
        >
          {progressText}
        </span>
      </div>
      <p className="text-[12.5px] leading-5 text-(--wb-ink-2)">
        {unlocked
          ? t("home.voiceProfile.unlocked")
          : t("home.voiceProfile.locked", {
              goal,
              count: VOICE_PROFILE_GOAL,
            })}
      </p>
      {unlocked && (
        <div>
          <Button
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
