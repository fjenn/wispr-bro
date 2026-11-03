// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Time saved (S-29b, B-28, DEC-50): "Dictating could save you about N
 * hours a week". The counter starts at 0 and counts up once there is a
 * typing speed: the test's (fork setting `typing_wpm`), or, after "Skip the
 * test", the 40 wpm average with a link back to the test. The slider sets
 * the typing time a day (`typing_hours_per_day`, default 2 h). The math is
 * in `typing.ts`.
 */
import { useEffect, useRef, useState } from "react";
import { Clock } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { Button } from "../ui";
import {
  clampHours,
  DEFAULT_TYPING_WPM,
  hoursSavedPerWeek,
  HOURS_MAX,
  HOURS_MIN,
  HOURS_STEP,
  SPEAKING_WPM,
} from "./typing";

const COUNT_MS = 900;

function reducedMotion(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** Counts from the shown value to `target` (at once with reduced motion). */
function useCountUp(target: number): number {
  const [shown, setShown] = useState(0);
  const from = useRef(0);
  useEffect(() => {
    const start = from.current;
    if (start === target || reducedMotion()) {
      from.current = target;
      setShown(target);
      return;
    }
    const began = performance.now();
    let frame = 0;
    const step = (now: number) => {
      const k = Math.min(1, (now - began) / COUNT_MS);
      const eased = 1 - (1 - k) ** 3;
      const value = start + (target - start) * eased;
      from.current = value;
      setShown(k < 1 ? Math.round(value * 2) / 2 : target);
      if (k < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [target]);
  return shown;
}

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
  const shown = useCountUp(target);
  const number = new Intl.NumberFormat(i18n.language, {
    maximumFractionDigits: 1,
  });
  const hoursLabel = new Intl.NumberFormat(i18n.language, {
    maximumFractionDigits: 1,
  }).format(hours);

  const saveHours = (value: number) => {
    setHours(value);
    if (!preview) void setSetting("typing_hours_per_day", value);
  };

  return (
    <div className="mx-auto flex w-full max-w-[580px] flex-col items-center gap-6 text-center">
      <div
        className="grid size-16 place-items-center rounded-[20px] bg-(--wb-banner) text-(--wb-accent)"
        aria-hidden="true"
      >
        <Clock size={32} strokeWidth={1.6} />
      </div>
      <h1
        className="m-0 font-(--wb-font-display) text-[34px] leading-[1.12] font-semibold tracking-[-0.02em] text-balance"
        data-testid="wb-time-saved-headline"
        data-hours={target}
      >
        {t("onboardingFlow.timeSaved.headline", {
          count: shown,
          hours: number.format(shown),
        })}
      </h1>
      {/* The final number once, for screen readers (the heading counts). */}
      <p className="sr-only" role="status">
        {revealed
          ? t("onboardingFlow.timeSaved.headline", {
              count: target,
              hours: number.format(target),
            })
          : ""}
      </p>

      {revealed ? (
        <>
          <p
            className="m-0 text-sm leading-6 text-pretty text-(--wb-ink-2)"
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
          <div className="flex w-full flex-col gap-2 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-4 text-start">
            <div className="flex items-baseline justify-between gap-3">
              <label
                htmlFor="wb-time-saved-hours"
                className="text-sm font-semibold"
              >
                {t("onboardingFlow.timeSaved.sliderLabel")}
              </label>
              <span
                className="text-sm text-(--wb-ink-2) tabular-nums"
                data-testid="wb-time-saved-hours-value"
              >
                {t("onboardingFlow.timeSaved.hoursPerDay", {
                  count: hours,
                  hours: hoursLabel,
                })}
              </span>
            </div>
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
              className="w-full accent-(--wb-accent-fill)"
            />
          </div>
          {measured == null && (
            <button
              type="button"
              onClick={onTakeTest}
              data-testid="wb-time-saved-take-test"
              className="cursor-pointer rounded-lg px-2 py-1 text-sm font-medium text-(--wb-ink-2) underline decoration-(--wb-line-strong) underline-offset-4 hover:text-(--wb-ink) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--wb-accent)"
            >
              {t("onboardingFlow.timeSaved.takeTest")}
            </button>
          )}
          <Button
            variant="primary"
            onClick={onContinue}
            data-testid="wb-time-saved-continue"
          >
            {t("onboardingFlow.continue")}
          </Button>
        </>
      ) : (
        <>
          <p className="m-0 text-sm leading-6 text-pretty text-(--wb-ink-2)">
            {t("onboardingFlow.timeSaved.intro")}
          </p>
          <div className="flex flex-wrap items-center justify-center gap-3">
            <Button
              variant="primary"
              onClick={onTakeTest}
              data-testid="wb-time-saved-test"
            >
              {t("onboardingFlow.timeSaved.howFast")}
            </Button>
            <Button
              onClick={() => setSkipped(true)}
              data-testid="wb-time-saved-skip"
            >
              {t("onboardingFlow.timeSaved.skipTest")}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
