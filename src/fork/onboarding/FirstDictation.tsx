// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The first dictation, celebrated (S-29b, B-27): a dark screen (the flow
 * applies `.wb-stage-dark`) with one burst of confetti (canvas-confetti
 * 1.9.4, ISC, adopted). No confetti at all with reduced motion; the burst
 * runs on this page's own canvas without a worker (no blob: script under
 * the app's CSP) and is cleared when the screen goes.
 */
import { useEffect, useRef } from "react";
import { PartyPopper } from "../ui/icons";
import { useTranslation } from "react-i18next";
import confetti from "canvas-confetti";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

function reducedMotion(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function FirstDictation({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const el = canvas.current;
    if (!el || reducedMotion()) return;
    const burst = confetti.create(el, {
      resize: true,
      useWorker: false,
      disableForReducedMotion: true,
    });
    const colors = ["#f3f0ea", "#b9b4ff", "#f1dca6", "#8fd1b4", "#f2a28f"];
    void burst({
      particleCount: preview ? 40 : 110,
      spread: 75,
      startVelocity: 42,
      origin: { x: 0.5, y: 0.65 },
      colors,
      ticks: 220,
    });
    el.dataset.fired = "true";
    return () => burst.reset();
  }, [preview]);

  return (
    <div className="relative mx-auto flex max-w-[560px] flex-col items-center gap-6 text-center">
      <canvas
        ref={canvas}
        aria-hidden="true"
        data-testid="wb-first-dictation-confetti"
        className="pointer-events-none fixed inset-0 z-10 size-full"
      />
      <div
        className="grid size-20 place-items-center rounded-[24px] bg-(--wb-banner) text-(--wb-accent)"
        aria-hidden="true"
      >
        <PartyPopper size={40} strokeWidth={1.6} />
      </div>
      <div className="flex flex-col gap-3">
        <h1 className="m-0 font-(--wb-font-display) text-[40px] leading-[1.08] font-semibold tracking-[-0.02em] text-balance">
          {t("onboardingFlow.firstDictation.title")}
        </h1>
        <p className="m-0 text-[15px] leading-6 text-pretty text-(--wb-ink-2)">
          {t("onboardingFlow.firstDictation.subtitle")}
        </p>
      </div>
      <Button
        variant="primary"
        className="relative z-20 px-5 py-2.5"
        onClick={onContinue}
        data-testid="wb-first-dictation-continue"
      >
        {t("onboardingFlow.continue")}
      </Button>
    </div>
  );
}
