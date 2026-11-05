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
import { Mail, Mic } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { events as handyEvents } from "@/bindings";
import { commands, events, type ForkSpeechSetup } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { CentredLayout, SkipLink, Title } from "./Layout";
import { chordLabel } from "../settings/hotkeys/model";

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
  // Any other dictation key (⌥Space, ⌘⇧): hands-free by double-tap.
  const custom = trigger && trigger !== "fn" && !fallback ? trigger : null;
  const keys = custom ? chordLabel(custom) : "";
  const status =
    phase === "listening"
      ? t("onboardingFlow.practice.listening", {
          context: custom ? "custom" : fallback ? "fallback" : undefined,
          keys,
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
    <CentredLayout
      title={<Title i18nKey="onboardingFlow.practice.title" />}
      subtitle={
        <span data-testid="wb-practice-instruction">
          {trigger
            ? t(
                custom
                  ? "onboardingFlow.practice.instructionCustom"
                  : fallback
                    ? "onboardingFlow.practice.instructionFallback"
                    : "onboardingFlow.practice.instruction",
                { keys },
              )
            : t("onboardingFlow.practice.instructionUnbound")}
        </span>
      }
      skip={
        <SkipLink onClick={onSkip} testId="wb-practice-skip">
          {t("onboardingFlow.practice.skip")}
        </SkipLink>
      }
      action={
        <Button
          variant="primary"
          size="lg"
          className="w-full max-w-[300px]"
          disabled={!filled && !preview}
          onClick={onDone}
          data-testid="wb-practice-continue"
        >
          {t("onboardingFlow.continue")}
        </Button>
      }
    >
      <div
        className="mt-9 flex w-full max-w-[480px] flex-col overflow-hidden rounded-(--wb-radius-md) bg-(--wb-surface) text-start shadow-(--wb-elev-2)"
        data-testid="wb-practice-composer"
      >
        <div className="flex items-center gap-2 bg-(--wb-surface-tint) px-4 py-3">
          <Mail size={16} aria-hidden="true" className="text-(--wb-ink-2)" />
          <span className="type-card-title">
            {t("onboardingFlow.practice.composer.title")}
          </span>
        </div>
        <dl className="m-0 px-4 text-[14px]">
          <div className="flex gap-2 border-b border-(--wb-line) py-2.5">
            <dt className="font-medium text-(--wb-ink-2)">
              {t("onboardingFlow.practice.composer.to")}
            </dt>
            <dd className="m-0 text-(--wb-ink-2)">
              {t("onboardingFlow.practice.composer.recipient")}
            </dd>
          </div>
          <div className="flex gap-2 border-b border-(--wb-line) py-2.5">
            <dt className="font-medium text-(--wb-ink-2)">
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
        <div className="p-3">
          <textarea
            id="wb-practice-body"
            value={body}
            onChange={(e) => setBody(e.target.value)}
            placeholder={t("onboardingFlow.practice.composer.placeholder")}
            data-testid="wb-practice-body"
            rows={5}
            className={`min-h-[132px] w-full resize-none rounded-(--wb-radius-sm) border bg-transparent px-3 py-2.5 text-[15px] leading-[22px] text-(--wb-ink) outline-none select-text placeholder:text-(--wb-ink-3) placeholder:italic focus-visible:border-(--wb-accent) ${
              phase === "listening"
                ? "border-(--wb-accent-2)"
                : "border-(--wb-line-2)"
            }`}
          />
        </div>
      </div>

      <p
        className="mt-4 mb-0 flex items-center gap-2 text-center text-[13px] leading-[18px] text-(--wb-ink-2)"
        role="status"
        data-testid="wb-practice-status"
        data-phase={phase}
      >
        <Mic
          size={14}
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
          className="mt-2 mb-0 max-w-[460px] text-center text-[13px] leading-[18px] text-(--wb-ink-3)"
          data-testid="wb-practice-queued-note"
        >
          {t("onboardingFlow.practice.queuedNote")}
        </p>
      )}
    </CentredLayout>
  );
}
