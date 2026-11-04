// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The first dictation, celebrated (S-29b, B-27; Design W1 choreography,
 * SYSTEM-PLAN §5 rule 5). A dark stage in either theme with a staged
 * reveal: the serif headline, then the two speed bars (typing and speaking,
 * both the averages the estimate uses, labelled as averages), a hand-drawn
 * arrow drawing itself while the speed multiple rolls up, the sticker
 * popping in, the caption, then Continue. One burst of confetti
 * (canvas-confetti 1.9.4, ISC, adopted) after the numbers settle, on this
 * page's own canvas without a worker (no blob: script under the app's
 * CSP), cleared when the screen goes. Reduce Motion: everything is there
 * at once, no confetti, no drawing, no rolling.
 */
import { useEffect, useRef, useState } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import confetti from "canvas-confetti";
import { FORK_NAMESPACE } from "../i18n";
import { Button, StatNumber, useMotionPresets } from "../ui";
import { DUR, EASE } from "../ui/motion";
import { ArtSlot, CentredLayout, Title } from "./Layout";
import { DEFAULT_TYPING_WPM, SPEAKING_WPM } from "./typing";

function reducedMotion(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** How many times faster speaking is, to one decimal. */
const TIMES = Math.round((SPEAKING_WPM / DEFAULT_TYPING_WPM) * 10) / 10;

export function FirstDictation({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const m = useMotionPresets();
  const stages = m.celebration.stages;
  const canvas = useRef<HTMLCanvasElement>(null);
  const [times, setTimes] = useState(m.reduced ? TIMES : 1);

  // The multiple rolls up while the arrow draws (stage 2).
  useEffect(() => {
    if (m.reduced) {
      setTimes(TIMES);
      return;
    }
    const roll = window.setTimeout(() => setTimes(TIMES), stages[2] * 1000);
    return () => window.clearTimeout(roll);
  }, [m.reduced, stages]);

  // One burst once the numbers have settled (after the sticker).
  useEffect(() => {
    const el = canvas.current;
    if (!el || reducedMotion()) return;
    const burst = confetti.create(el, {
      resize: true,
      useWorker: false,
      disableForReducedMotion: true,
    });
    // The palette lives in tokens.css (`--wb-confetti`, comma-separated).
    const colors = getComputedStyle(el)
      .getPropertyValue("--wb-confetti")
      .split(",")
      .map((c) => c.trim())
      .filter(Boolean);
    const fire = window.setTimeout(() => {
      void burst({
        particleCount: preview ? 40 : 110,
        spread: 75,
        startVelocity: 42,
        origin: { x: 0.5, y: 0.6 },
        colors,
        ticks: 220,
      });
    }, stages[3] * 1000);
    el.dataset.fired = "true";
    return () => {
      window.clearTimeout(fire);
      burst.reset();
    };
  }, [preview, stages]);

  /** A part that fades up at stage `i`. */
  const at = (i: number) => ({
    initial: m.celebration.headline.initial,
    animate: {
      ...m.celebration.headline.animate,
      transition: {
        ...(m.celebration.headline.animate?.transition ?? {}),
        delay: stages[i],
      },
    },
  });

  const bar = (
    label: string,
    wpm: number,
    width: string,
    tone: "plain" | "accent",
    testId: string,
  ) => (
    <div className="flex flex-col gap-1.5" data-testid={testId}>
      <span className="type-micro text-(--wb-ink-2)">{label}</span>
      <motion.span
        className={`flex h-8 origin-left items-center rounded-(--wb-radius-xs) px-3 text-[14px] font-medium whitespace-nowrap rtl:origin-right ${
          tone === "accent"
            ? "bg-(--wb-accent) text-(--wb-on-ink)"
            : "bg-(--wb-secondary) text-(--wb-ink)"
        }`}
        style={{ width }}
        initial={m.reduced ? false : { scaleX: 0.2, opacity: 0 }}
        animate={{
          scaleX: 1,
          opacity: 1,
          transition: m.reduced
            ? { duration: 0 }
            : { duration: DUR.grow, ease: EASE.out, delay: stages[1] },
        }}
      >
        {t("onboardingFlow.typing.wpm", { count: wpm })}
      </motion.span>
    </div>
  );

  return (
    <CentredLayout
      tone="dark"
      align="start"
      width={820}
      action={
        <motion.div {...at(5)} className="flex w-full justify-center">
          <Button
            variant="cream"
            size="lg"
            className="relative z-20 w-full max-w-[300px]"
            onClick={onContinue}
            data-testid="wb-first-dictation-continue"
          >
            {t("onboardingFlow.continue")}
          </Button>
        </motion.div>
      }
    >
      <canvas
        ref={canvas}
        aria-hidden="true"
        data-testid="wb-first-dictation-confetti"
        className="pointer-events-none fixed inset-0 z-10 size-full"
      />
      <motion.h1
        {...at(0)}
        className="type-hero m-0 mt-2 text-balance text-(--wb-ink)"
      >
        {t("onboardingFlow.firstDictation.title")}
      </motion.h1>

      <div className="mt-10 grid w-full grid-cols-[minmax(0,1fr)_minmax(0,1fr)] items-end gap-10">
        <div className="relative flex flex-col gap-5">
          <motion.p
            {...at(1)}
            className="type-heading-xl m-0 max-w-[min(340px,calc(100%-88px))] text-balance text-(--wb-ink)"
            data-testid="wb-first-dictation-lead"
          >
            {t("onboardingFlow.firstDictation.lead")}
          </motion.p>
          {bar(
            t("onboardingFlow.firstDictation.typingLabel"),
            DEFAULT_TYPING_WPM,
            `${Math.max(32, (DEFAULT_TYPING_WPM / SPEAKING_WPM) * 100)}%`,
            "plain",
            "wb-first-dictation-typing",
          )}
          {bar(
            t("onboardingFlow.firstDictation.speakingLabel"),
            SPEAKING_WPM,
            "100%",
            "accent",
            "wb-first-dictation-speaking",
          )}
          {/* W7's hand-drawn arrow (`stickers/arrow.svg`: shaft, then
              head), drawn here so `pathLength` can animate it. It reaches
              80 px into this column; the lead keeps 88 px clear of it
              (mirrored in RTL, where longer leads used to run under it). */}
          <svg
            aria-hidden="true"
            viewBox="0 0 220 140"
            className="pointer-events-none absolute -end-24 -top-2 w-44 text-(--wb-accent) rtl:-scale-x-100"
            data-testid="wb-first-dictation-arrow"
            fill="none"
          >
            <motion.path
              d="M14 118c22-6 40-20 52-38 9-14 12-31 4-38-9-8-23 3-19 16 5 17 31 22 52 18 30-6 55-26 77-47"
              stroke="currentColor"
              strokeWidth="4"
              strokeLinecap="round"
              initial={m.celebration.draw.initial}
              animate={{
                ...m.celebration.draw.animate,
                transition: m.reduced
                  ? { duration: 0 }
                  : {
                      ...(m.celebration.draw.animate?.transition ?? {}),
                      delay: stages[2],
                    },
              }}
            />
            <motion.path
              d="M164 22c10-2 20-3 30-2-1 10-4 20-8 29"
              stroke="currentColor"
              strokeWidth="4"
              strokeLinecap="round"
              strokeLinejoin="round"
              initial={m.reduced ? false : { opacity: 0 }}
              animate={{
                opacity: 1,
                transition: m.reduced
                  ? { duration: 0 }
                  : { duration: DUR.base, delay: stages[2] + DUR.draw },
              }}
            />
          </svg>
        </div>

        <div className="flex flex-col items-center text-center">
          <motion.div
            initial={m.celebration.pop.initial}
            animate={{
              ...m.celebration.pop.animate,
              transition: m.reduced
                ? { duration: 0 }
                : {
                    ...(m.celebration.pop.animate?.transition ?? {}),
                    delay: stages[3],
                  },
            }}
          >
            <ArtSlot name="sticker-rocket" size="sm" />
          </motion.div>
          <motion.p
            {...at(2)}
            className="type-display m-0 text-[64px] leading-[68px] text-(--wb-ink) [&_em]:text-(--wb-accent)"
            data-testid="wb-first-dictation-times"
            data-times={TIMES}
          >
            <TimesPhrase times={times} />
          </motion.p>
          <motion.p
            {...at(4)}
            className="mt-2 mb-0 text-[15px] leading-[22px] text-(--wb-ink-2)"
          >
            {t("onboardingFlow.firstDictation.subtitle")}
          </motion.p>
        </div>
      </div>
    </CentredLayout>
  );
}

/** "3.3× faster": the rolling number inside the translated phrase. */
function TimesPhrase({ times }: { times: number }) {
  return (
    <Title
      i18nKey="onboardingFlow.firstDictation.faster"
      components={{
        n: (
          <StatNumber
            value={times}
            format={{ maximumFractionDigits: 1, minimumFractionDigits: 1 }}
          />
        ),
      }}
    />
  );
}
