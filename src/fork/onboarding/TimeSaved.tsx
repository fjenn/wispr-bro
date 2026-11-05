// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Time saved (S-29b, B-28, DEC-50): "Dictating could save you about N
 * hours a week". The slider (hours of typing a week, 4 by default, owner's
 * rc9 call) is always there and the number follows it at once. The typing
 * speed is the test's (fork setting `typing_wpm`; the test is offered on
 * the celebration step before this one), else the 45 wpm average. The
 * setting is stored per day (`typing_hours_per_day`); the math is in
 * `typing.ts`.
 */
import { useEffect, useRef, useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { Button, StatNumber } from "../ui";
import { CentredLayout, Title } from "./Layout";
import {
  dailyHours,
  DEFAULT_TYPING_WPM,
  hoursSavedPerWeek,
  HOURS_MAX,
  HOURS_MIN,
  HOURS_STEP,
  SPEAKING_WPM,
  weeklyHours,
} from "./typing";

export function TimeSaved({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  const measured = settings?.typing_wpm ?? null;
  const [hours, setHours] = useState(() =>
    weeklyHours(settings?.typing_hours_per_day),
  );
  const seeded = useRef(false);
  useEffect(() => {
    if (seeded.current || !settings) return;
    seeded.current = true;
    setHours(weeklyHours(settings.typing_hours_per_day));
  }, [settings]);

  const wpm = measured ?? DEFAULT_TYPING_WPM;
  const target = hoursSavedPerWeek(wpm, hours);
  const number = new Intl.NumberFormat(i18n.language, {
    maximumFractionDigits: 1,
  });
  const hoursLabel = number.format(hours);
  const fill = ((hours - HOURS_MIN) / (HOURS_MAX - HOURS_MIN)) * 100;
  const headline = t("onboardingFlow.timeSaved.headline", {
    count: target,
    hours: number.format(target),
  });

  const saveHours = (value: number) => {
    setHours(value);
    if (!preview) void setSetting("typing_hours_per_day", dailyHours(value));
  };

  return (
    <CentredLayout
      tone="dark"
      width={620}
      action={
        <Button
          variant="cream"
          size="lg"
          className="w-full max-w-[300px]"
          onClick={onContinue}
          data-testid="wb-time-saved-continue"
        >
          {t("onboardingFlow.continue")}
        </Button>
      }
    >
      <p
        aria-hidden="true"
        className="mt-6 mb-0 text-[20px] leading-7 font-medium text-(--wb-ink)"
      >
        {t("onboardingFlow.timeSaved.lead")}
      </p>
      <h1
        className="m-0 mt-2"
        data-testid="wb-time-saved-headline"
        data-hours={target}
      >
        <span
          aria-hidden="true"
          className="type-display block text-[72px] leading-[80px] whitespace-nowrap text-(--wb-ink) [&_em]:text-(--wb-accent)"
        >
          <Title
            i18nKey="onboardingFlow.timeSaved.perWeek"
            values={{ count: target }}
            components={{ n: <StatNumber value={target} /> }}
          />
        </span>
        <span className="sr-only" data-testid="wb-time-saved-headline-text">
          {headline}
        </span>
      </h1>
      {/* The final number once, for screen readers. */}
      <p className="sr-only" role="status">
        {headline}
      </p>

      <div className="mt-10 flex w-full max-w-[460px] flex-col gap-1">
        <label
          htmlFor="wb-time-saved-hours"
          className="text-[15px] font-medium text-(--wb-ink)"
        >
          {t("onboardingFlow.timeSaved.sliderLabel")}
        </label>
        <input
          id="wb-time-saved-hours"
          type="range"
          min={HOURS_MIN}
          max={HOURS_MAX}
          step={HOURS_STEP}
          value={hours}
          aria-valuetext={t("onboardingFlow.timeSaved.hoursPerWeek", {
            count: hours,
            hours: hoursLabel,
          })}
          onChange={(e) => saveHours(Number(e.target.value))}
          data-testid="wb-time-saved-slider"
          className="wb-onb-range mt-2"
          style={{ "--wb-fill": `${fill}%` } as CSSProperties}
        />
        <span
          className="text-[15px] text-(--wb-ink-2) tabular-nums"
          data-testid="wb-time-saved-hours-value"
        >
          {t("onboardingFlow.timeSaved.hoursPerWeek", {
            count: hours,
            hours: hoursLabel,
          })}
        </span>
      </div>
      <p
        className="mt-6 mb-0 max-w-[460px] text-[13px] leading-[18px] text-pretty text-(--wb-ink-3)"
        data-testid="wb-time-saved-basis"
      >
        {measured != null
          ? t("onboardingFlow.timeSaved.basisMeasured", {
              count: measured,
              wpm: measured,
              speaking: SPEAKING_WPM,
            })
          : t("onboardingFlow.timeSaved.basisAverage", {
              count: DEFAULT_TYPING_WPM,
              wpm: DEFAULT_TYPING_WPM,
              speaking: SPEAKING_WPM,
            })}
      </p>
    </CentredLayout>
  );
}
