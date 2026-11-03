// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Practice (S-29b, B-25): dictate an email hands-free into a drawn
 * composer. While this screen is open the backend's practice mode is on
 * (`fork_onboarding_practice`, renewed every minute, off on leaving): a take
 * started with Wispr Bro in front comes back as `fork-practice-take` and
 * fills the body here, instead of being pasted. Its source is `practice`,
 * so it is in History but not in the stats (DEC-11).
 *
 * If no speech model is ready yet, the take still records and waits in the
 * queue (S-07c): the screen shows "Setting up… N%" from `fork-speech-setup`
 * and fills the body when the queued take is transcribed. After 90 s of
 * waiting it moves on by itself; the take stays in History. Skip is always
 * offered. The take's way (hold or hands-free) is not enforced: any take
 * counts, the copy teaches hands-free.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { Mic } from "lucide-react";
import { useTranslation } from "react-i18next";
import { events as handyEvents } from "@/bindings";
import { commands, events, type ForkSpeechSetup } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

/** The practice mode renews itself before the backend's two-minute expiry. */
const RENEW_MS = 60_000;
/** A queued practice take waits this long before the step moves on. */
export const PRACTICE_WAIT_MS = 90_000;
/** No text, no queue this long after the take ended: say so. */
const NOTHING_MS = 20_000;

type Phase = "idle" | "listening" | "working" | "queued" | "nothing";

export function HandsFreePractice({
  preview,
  onDone,
  onSkip,
}: {
  preview: boolean;
  /** The practice produced a take (filled, or waited the full 90 s). */
  onDone: () => void;
  onSkip: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [trigger, setTrigger] = useState<string | null>("fn");
  const [phase, setPhase] = useState<Phase>("idle");
  const [body, setBody] = useState("");
  const [filled, setFilled] = useState(false);
  const [percent, setPercent] = useState<number | null>(null);
  const nothingTimer = useRef<number>();
  const waitTimer = useRef<number>();
  const doneRef = useRef(onDone);
  doneRef.current = onDone;

  const clearTimers = () => {
    window.clearTimeout(nothingTimer.current);
    window.clearTimeout(waitTimer.current);
  };

  const onSetup = useCallback((setup: ForkSpeechSetup) => {
    setPercent(setup.phase === "ready" ? 100 : setup.percent);
  }, []);

  // Practice mode, its renewal, and the key the copy names.
  useEffect(() => {
    if (preview) return;
    let alive = true;
    const on = () => void commands.forkOnboardingPractice(true);
    on();
    const renew = window.setInterval(on, RENEW_MS);
    void commands.forkHotkeyStatus().then((result) => {
      if (alive && result.status === "ok") setTrigger(result.data.trigger);
    });
    return () => {
      alive = false;
      window.clearInterval(renew);
      void commands.forkOnboardingPractice(false);
    };
  }, [preview]);

  // Takes: their phases, their text, and a queued take's wait.
  useEffect(() => {
    if (preview) return;
    const offPhase = handyEvents.recordingStateEvent.listen(({ payload }) => {
      const p = payload.phase;
      if (p === "started" || p === "locked") {
        window.clearTimeout(nothingTimer.current);
        setPhase((now) => (now === "queued" ? now : "listening"));
      } else if (p === "cancelled") {
        setPhase((now) => (now === "queued" ? now : "idle"));
      } else {
        setPhase((now) => (now === "queued" ? now : "working"));
        window.clearTimeout(nothingTimer.current);
        nothingTimer.current = window.setTimeout(() => {
          setPhase((now) => (now === "working" ? "nothing" : now));
        }, NOTHING_MS);
      }
    });
    const offTake = events.forkPracticeTake.listen(({ payload }) => {
      if (payload.kind === "queued") {
        window.clearTimeout(nothingTimer.current);
        setPhase("queued");
        // Ask once for the progress; the event keeps it current.
        void commands.forkSpeechSetupStatus().then(onSetup);
        window.clearTimeout(waitTimer.current);
        waitTimer.current = window.setTimeout(
          () => doneRef.current(),
          PRACTICE_WAIT_MS,
        );
        return;
      }
      const text = payload.text?.trim();
      if (!text) {
        setPhase((now) => (now === "queued" ? now : "nothing"));
        return;
      }
      clearTimers();
      setBody((now) => (now.trim() ? `${now.trimEnd()} ${text}` : text));
      setFilled(true);
      setPhase("idle");
    });
    const offSetup = events.forkSpeechSetup.listen(({ payload }) =>
      onSetup(payload),
    );
    return () => {
      clearTimers();
      for (const off of [offPhase, offTake, offSetup]) {
        void off.then((f) => f()).catch(() => undefined);
      }
    };
  }, [preview, onSetup]);

  const fallback = trigger === "ctrl+option";
  const status =
    phase === "listening"
      ? t("onboardingFlow.practice.listening", {
          context: fallback ? "fallback" : undefined,
        })
      : phase === "working"
        ? t("onboardingFlow.practice.working")
        : phase === "queued"
          ? percent != null && percent < 100
            ? t("onboardingFlow.practice.settingUpPercent", { percent })
            : t("onboardingFlow.practice.settingUp")
          : phase === "nothing"
            ? t("onboardingFlow.practice.nothing")
            : filled
              ? t("onboardingFlow.practice.filled")
              : t("onboardingFlow.practice.waiting");

  return (
    <div className="mx-auto flex w-full max-w-[620px] flex-col items-center gap-5">
      <div className="flex w-full items-start justify-between gap-3">
        <div className="min-w-0 flex-1 text-start">
          <h1 className="m-0 font-(--wb-font-display) text-3xl font-semibold text-balance">
            {t("onboardingFlow.practice.title")}
          </h1>
          <p
            className="mt-2 mb-0 text-sm leading-6 text-pretty text-(--wb-ink-2)"
            data-testid="wb-practice-instruction"
          >
            {trigger
              ? t(
                  fallback
                    ? "onboardingFlow.practice.instructionFallback"
                    : "onboardingFlow.practice.instruction",
                )
              : t("onboardingFlow.practice.instructionUnbound")}
          </p>
        </div>
        <button
          type="button"
          onClick={onSkip}
          data-testid="wb-practice-skip"
          className="shrink-0 cursor-pointer rounded-lg px-3 py-1.5 text-sm font-medium text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--wb-accent)"
        >
          {t("onboardingFlow.practice.skip")}
        </button>
      </div>

      <div
        className="flex w-full flex-col overflow-hidden rounded-(--wb-radius-card) border border-(--wb-line-strong) bg-(--wb-card) text-start shadow-(--wb-shadow)"
        data-testid="wb-practice-composer"
      >
        <div className="flex items-center justify-between border-b border-(--wb-line) px-4 py-2.5">
          <span className="text-sm font-semibold">
            {t("onboardingFlow.practice.composer.title")}
          </span>
        </div>
        <dl className="m-0 border-b border-(--wb-line) text-sm">
          <div className="flex gap-3 border-b border-(--wb-line) px-4 py-2">
            <dt className="w-16 shrink-0 text-(--wb-ink-3)">
              {t("onboardingFlow.practice.composer.to")}
            </dt>
            <dd className="m-0 text-(--wb-ink-2)">
              {t("onboardingFlow.practice.composer.recipient")}
            </dd>
          </div>
          <div className="flex gap-3 px-4 py-2">
            <dt className="w-16 shrink-0 text-(--wb-ink-3)">
              {t("onboardingFlow.practice.composer.subject")}
            </dt>
            <dd className="m-0 text-(--wb-ink-2)">
              {t("onboardingFlow.practice.composer.subjectText")}
            </dd>
          </div>
        </dl>
        <label className="sr-only" htmlFor="wb-practice-body">
          {t("onboardingFlow.practice.composer.body")}
        </label>
        <textarea
          id="wb-practice-body"
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder={t("onboardingFlow.practice.composer.placeholder")}
          data-testid="wb-practice-body"
          rows={6}
          className="min-h-[150px] w-full resize-none select-text border-0 bg-transparent px-4 py-3 text-[15px] leading-6 text-(--wb-ink) outline-none placeholder:text-(--wb-ink-3) focus-visible:bg-(--wb-hover)"
        />
      </div>

      <p
        className="m-0 flex items-center gap-2 text-center text-sm text-(--wb-ink-2)"
        role="status"
        data-testid="wb-practice-status"
        data-phase={phase}
      >
        <Mic
          size={15}
          aria-hidden="true"
          className={
            phase === "listening"
              ? "text-(--wb-accent) motion-safe:animate-pulse"
              : "text-(--wb-ink-3)"
          }
        />
        {status}
      </p>
      {phase === "queued" && (
        <p
          className="m-0 max-w-[480px] text-center text-xs leading-5 text-(--wb-ink-3)"
          data-testid="wb-practice-queued-note"
        >
          {t("onboardingFlow.practice.queuedNote")}
        </p>
      )}

      <Button
        variant="primary"
        disabled={!filled && !preview}
        onClick={onDone}
        data-testid="wb-practice-continue"
      >
        {t("onboardingFlow.continue")}
      </Button>
    </div>
  );
}
