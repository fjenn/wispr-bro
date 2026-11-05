// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Insights › Voice (S-25, C-57): the voice profile.
 *
 * Locked until 1,700 lifetime words (DEC-50, the Home bar's count): a
 * progress bar and the four cards blurred. The blurred content is generic
 * sample text, not the user's data (nothing to uncover), and is hidden
 * from screen readers; each card's title and "Locked" stay readable.
 *
 * Unlocked: your phrase, peak time and app, most used word and most
 * corrected word, from `fork_insights_voice` (worked out on this Mac from
 * History). A card with too little data says so.
 */
import { Lock } from "../ui/icons";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ProgressLine } from "@/components/shared/ProgressLine";
import type { VoiceProfile } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { formatCount } from "../home/StatsRail";
import { Hero } from "../ui";
import { photos } from "@/assets/fork/art";
import { Card } from "./UsageTab";

type CardId = "phrase" | "peak" | "word" | "corrected";

const VALUE = "type-serif-title text-(--wb-ink) break-words";
const NOTE = "text-[15px] leading-[22px] text-(--wb-ink-2)";

/** "2 – 3 PM" in the app language for the hour starting at `hour`. */
export function formatHourRange(hour: number, language: string): string {
  const from = new Date(2026, 0, 5, hour);
  const to = new Date(2026, 0, 5, hour + 1);
  try {
    const fmt = new Intl.DateTimeFormat(language, { hour: "numeric" });
    return fmt.formatRange(from, to);
  } catch {
    return `${hour}:00–${(hour + 1) % 24}:00`;
  }
}

function VoiceCard({
  id,
  locked,
  children,
}: {
  id: CardId;
  locked: boolean;
  children: ReactNode;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <Card
      testId={`wb-voice-card-${id}`}
      title={t(`insights.voice.${id}`)}
      small
      className="min-h-36 gap-3!"
    >
      {locked ? (
        <>
          <span className="inline-flex items-center gap-1.5 text-[13px] font-medium text-(--wb-ink-2)">
            <Lock size={14} aria-hidden="true" />
            {t("insights.voice.locked")}
          </span>
          <p
            aria-hidden="true"
            data-testid={`wb-voice-card-${id}-sample`}
            className={`${VALUE} pointer-events-none blur-[6px] select-none`}
          >
            {t(`insights.voice.sample.${id}`)}
          </p>
        </>
      ) : (
        children
      )}
    </Card>
  );
}

function NotEnough() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return <p className={NOTE}>{t("insights.voice.notEnough")}</p>;
}

function LockedHeader({ voice }: { voice: VoiceProfile }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const language = i18n.language;
  const current = Math.min(voice.total_words, voice.goal);
  const goal = formatCount(voice.goal, language);
  const progress = t("home.voiceProfile.progress", {
    current: formatCount(current, language),
    goal,
    count: voice.goal,
  });
  // The W7 Insights photo as the locked profile's hero; the bar sits on
  // its right. The hero's heading names the progress bar.
  return (
    <div data-testid="wb-voice-locked">
      <Hero
        id="voice-locked"
        image={photos.insights.src}
        title={t("insights.voice.lockedTitle", { goal, count: voice.goal })}
        body={t("insights.voice.lockedBody", {
          current: formatCount(current, language),
          goal,
          count: voice.goal,
          appName: t("rust.appName"),
        })}
        aside={
          <div
            className="flex w-44 flex-col gap-2"
            role="progressbar"
            data-testid="wb-voice-locked-bar"
            aria-labelledby="wb-banner-voice-locked-title"
            aria-valuemin={0}
            aria-valuemax={voice.goal}
            aria-valuenow={current}
            aria-valuetext={progress}
          >
            <ProgressLine
              value={current / voice.goal}
              className="h-1 rounded-full bg-white/25! [&>i]:rounded-full [&>i]:bg-(--wb-on-hero)!"
            />
            <span
              aria-hidden="true"
              className="text-[12px] font-medium text-(--wb-on-hero-2) tabular-nums"
            >
              {progress}
            </span>
          </div>
        }
      />
    </div>
  );
}

export function VoiceTab({ voice }: { voice: VoiceProfile }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const language = i18n.language;
  const locked = !voice.unlocked;
  const { phrase, peak, word, corrected } = voice;
  return (
    <div
      data-testid="wb-insights-voice"
      data-unlocked={voice.unlocked}
      className="@container flex flex-col gap-6"
    >
      {locked && <LockedHeader voice={voice} />}
      <div className="grid grid-cols-1 gap-6 @min-[560px]:grid-cols-2">
        <VoiceCard id="phrase" locked={locked}>
          {phrase ? (
            <>
              <p data-testid="wb-voice-phrase" className={VALUE}>
                {`“${phrase.phrase}”`}
              </p>
              <p className={NOTE}>
                {t("insights.voice.phraseIn", { count: phrase.dictations })}
              </p>
            </>
          ) : (
            <NotEnough />
          )}
        </VoiceCard>
        <VoiceCard id="peak" locked={locked}>
          {peak ? (
            <>
              <p data-testid="wb-voice-peak-hour" className={VALUE}>
                {formatHourRange(peak.hour, language)}
              </p>
              {peak.app_name && (
                <p data-testid="wb-voice-peak-app" className={NOTE}>
                  {t("insights.voice.peakApp", { app: peak.app_name })}
                </p>
              )}
            </>
          ) : (
            <NotEnough />
          )}
        </VoiceCard>
        <VoiceCard id="word" locked={locked}>
          {word ? (
            <>
              <p data-testid="wb-voice-word" className={VALUE}>
                {word.word}
              </p>
              <p className={NOTE}>
                {t("insights.voice.uses", { count: word.uses })}
              </p>
            </>
          ) : (
            <NotEnough />
          )}
        </VoiceCard>
        <VoiceCard id="corrected" locked={locked}>
          {corrected ? (
            <>
              <p data-testid="wb-voice-corrected" className={VALUE}>
                {t("insights.voice.pair", {
                  heard: corrected.heard,
                  written: corrected.written,
                })}
              </p>
              <p className={NOTE}>
                {t("insights.voice.fixed", { count: corrected.times })}
              </p>
            </>
          ) : (
            <NotEnough />
          )}
        </VoiceCard>
      </div>
      {!locked && (
        <p className={NOTE} data-testid="wb-voice-footnote">
          {t("insights.voice.footnote", { count: voice.dictations_read })}
        </p>
      )}
    </div>
  );
}
