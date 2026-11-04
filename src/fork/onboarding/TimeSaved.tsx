// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Time saved (S-29b, B-28, DEC-50): "Dictating could save you about N
 * hours a week". The counter starts at 0 and counts up once there is a
 * typing speed: the test's (fork setting `typing_wpm`), or, after "Skip the
 * test", the 40 wpm average with a link back to the test. The slider sets
 * the typing time a day (`typing_hours_per_day`, default 2 h). The math is
 * in `typing.ts`.
 */
import { useEffect, useRef, useState, type CSSProperties } from "react";
import { ArrowRight } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { Button, FOCUS_RING, StatNumber } from "../ui";
import { CentredLayout, Title } from "./Layout";
import {
  clampHours,
  DEFAULT_TYPING_WPM,
  hoursSavedPerWeek,
  HOURS_MAX,
  HOURS_MIN,
  HOURS_STEP,
  SPEAKING_WPM,
} from "./typing";

export function TimeSaved({
  preview,
  onTakeTest,
  onContinue,
}: {
  preview: boolean;
  onTakeTest: () => void;
  onContinue: () => void;
}) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  const measured = settings?.typing_wpm ?? null;
  const [skipped, setSkipped] = useState(false);
  const [hours, setHours] = useState(() =>
    clampHours(settings?.typing_hours_per_day),
  );
  const seeded = useRef(false);
  useEffect(() => {
    if (seeded.current || !settings) return;
    seeded.current = true;
    setHours(clampHours(settings.typing_hours_per_day));
  }, [settings]);

  const revealed = measured != null || skipped;
  const wpm = measured ?? DEFAULT_TYPING_WPM;
  const target = revealed ? hoursSavedPerWeek(wpm, hours) : 0;
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
    if (!preview) void setSetting("typing_hours_per_day", value);
  };

  return (
    <CentredLayout
      tone="dark"
      width={620}
      action={
        revealed ? (
          <div className="flex w-full flex-col items-center gap-4">
            {measured == null && (
              <button
                type="button"
                onClick={onTakeTest}
                data-testid="wb-time-saved-take-test"
                className={`inline-flex cursor-pointer items-center gap-2 rounded-(--wb-radius-sm) px-2 py-1 text-[15px] font-medium text-(--wb-ink) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-fill-hover) ${FOCUS_RING}`}
              >
                {t("onboardingFlow.timeSaved.takeTest")}
                <span className="grid size-6 place-items-center rounded-(--wb-radius-xs) bg-(--wb-secondary)">
                  <ArrowRight
                    size={14}
                    aria-hidden="true"
                    className="rtl:-scale-x-100"
                  />
                </span>
              </button>
            )}
            <Button
              variant="cream"
              size="lg"
              className="w-full max-w-[300px]"
              onClick={onContinue}
              data-testid="wb-time-saved-continue"
            >
              {t("onboardingFlow.continue")}
            </Button>
          </div>
        ) : (
          <div className="flex w-full flex-col items-center gap-2">
            <Button
              variant="cream"
              size="lg"
              className="w-full max-w-[300px]"
              onClick={onTakeTest}
              data-testid="wb-time-saved-test"
            >
              {t("onboardingFlow.timeSaved.howFast")}
            </Button>
            <Button
              variant="ghost"
              size="lg"
              className="w-full max-w-[300px]"
              onClick={() => setSkipped(true)}
              data-testid="wb-time-saved-skip"
            >
              {t("onboardingFlow.timeSaved.skipTest")}
            </Button>
          </div>
        )
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
        {revealed ? headline : ""}
      </p>

      {revealed ? (
        <>
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
              aria-valuetext={t("onboardingFlow.timeSaved.hoursPerDay", {
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
              {t("onboardingFlow.timeSaved.hoursPerDay", {
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
        </>
      ) : (
        <p className="mt-8 mb-0 max-w-[440px] text-[15px] leading-[22px] text-pretty text-(--wb-ink-2)">
          {t("onboardingFlow.timeSaved.intro")}
        </p>
      )}
    </CentredLayout>
  );
}
