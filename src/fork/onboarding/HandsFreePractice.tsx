// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Practice (S-29b, B-25): dictate an email hands-free into a drawn
 * composer. While this screen is open the backend's practice mode is on
 * (`fork_onboarding_practice`, renewed every minute, off on leaving): a take
 * started with Wispr Bro in front comes back as `fork-practice-take` and
 * fills the body here, instead of being pasted. Its source is `practice`,
 * so it is in History but not in the stats (DEC-11).
 *
 * The suggested sentence and subject are in the user's language (rc9):
 * the first of their languages the first speech model hears, so the take
 * runs on it at once (`practiceSampleFor`); the labels stay in the app's.
 *
 * While no speech model can transcribe yet (Dutch alone: DEC-52's one
 * download; or a take held for the full model behind a partial starter),
 * the screen says so plainly (`ModelWait`): what is happening, that it
 * happens once, a real progress bar with megabytes and the time left from
 * the measured speed, "Checking the download…" while the file is checked
 * (never a stuck 99 %), and Continue stays enabled so the rest of the
 * onboarding goes on. A take made meanwhile records and waits in the queue
 * (S-07c): the composer says it will appear and fills in when the queued
 * take is transcribed. After 90 s of waiting it moves on by itself; the
 * take stays in History. Skip is always offered. The take's way (hold or
 * hands-free) is not enforced: any take counts, the copy teaches
 * hands-free.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { Mail, Mic } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { events as handyEvents } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { commands, events, type ForkSpeechSetup } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, ProgressLine } from "../ui";
import { CentredLayout, SkipLink, Title } from "./Layout";
import { practiceSampleFor } from "./samples";
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
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const [trigger, setTrigger] = useState<string | null>("fn");
  const [phase, setPhase] = useState<Phase>("idle");
  const [body, setBody] = useState("");
  const [filled, setFilled] = useState(false);
  const [setup, setSetup] = useState<ForkSpeechSetup | null>(null);
  const { settings: handy } = useSettings();
  const nothingTimer = useRef<number>();
  const waitTimer = useRef<number>();
  const doneRef = useRef(onDone);
  doneRef.current = onDone;

  const clearTimers = () => {
    window.clearTimeout(nothingTimer.current);
    window.clearTimeout(waitTimer.current);
  };

  const onSetup = useCallback((next: ForkSpeechSetup) => setSetup(next), []);

  // Practice mode, its renewal, and the key the copy names.
  useEffect(() => {
    if (preview) return;
    let alive = true;
    void commands.forkSpeechSetupStatus().then((s) => {
      if (alive) onSetup(s);
    });
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
  }, [preview, onSetup]);

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
          ? t("onboardingFlow.practice.queued")
          : phase === "nothing"
            ? t("onboardingFlow.practice.nothing")
            : filled
              ? t("onboardingFlow.practice.filled")
              : t("onboardingFlow.practice.waiting");

  const { practice: sample, language: sampleLanguage } = practiceSampleFor(
    setup?.starter_languages ?? [],
    handy?.spoken_languages ?? [],
    i18n.language,
  );
  // No model can transcribe yet: the wait is shown and never blocks.
  const waiting =
    setup != null && setup.phase !== "ready" && setup.phase !== "checking";

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
          disabled={!filled && !waiting && !preview}
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
            <dd className="m-0 text-(--wb-ink-2)" lang={sampleLanguage}>
              {sample.subject}
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
            placeholder={
              phase === "queued" && !body.trim()
                ? t("onboardingFlow.practice.composer.willAppear")
                : t("onboardingFlow.practice.composer.trySaying", {
                    text: sample.suggestion,
                  })
            }
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
      {waiting && setup && <ModelWait setup={setup} />}
    </CentredLayout>
  );
}

/**
 * "Getting your speech model ready": what happens, that it happens once,
 * and how far it is (bytes and the time left from the measured speed; the
 * checksum as "Checking the download…", never a stuck 99 %).
 */
export function ModelWait({ setup }: { setup: ForkSpeechSetup }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const mb = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 0 });
  const verifying = setup.phase === "verifying";
  const failed = setup.phase === "failed";
  const paused = setup.phase === "waiting";
  const known =
    setup.phase === "downloading" &&
    setup.percent != null &&
    setup.total_bytes > 0;
  const eta = setup.eta_seconds;
  const title = verifying
    ? t("onboardingFlow.practice.model.checking")
    : failed
      ? t("onboardingFlow.practice.model.failed")
      : paused
        ? t("onboardingFlow.practice.model.paused")
        : t("onboardingFlow.practice.model.title");
  const left =
    known && eta != null
      ? eta < 60
        ? t("onboardingFlow.practice.model.underMinute")
        : t("onboardingFlow.practice.model.minutesLeft", {
            count: Math.ceil(eta / 60),
          })
      : null;
  const bytes = known
    ? t("onboardingFlow.practice.model.bytes", {
        done: mb.format(setup.downloaded_bytes / 1_000_000),
        total: mb.format(setup.total_bytes / 1_000_000),
      })
    : null;
  return (
    <section
      className="mt-6 flex w-full max-w-[480px] flex-col gap-2 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) px-4 py-3 text-start"
      aria-labelledby="wb-practice-model-title"
      data-testid="wb-practice-model-wait"
      data-phase={setup.phase}
    >
      <div className="flex items-baseline justify-between gap-3">
        <h2
          id="wb-practice-model-title"
          className="type-card-title m-0 text-(--wb-ink)"
        >
          {title}
        </h2>
        {left && (
          <span
            className="shrink-0 text-[13px] text-(--wb-ink-2) tabular-nums"
            data-testid="wb-practice-model-eta"
          >
            {left}
          </span>
        )}
      </div>
      {known ? (
        <ProgressLine
          value={(setup.percent ?? 0) / 100}
          thickness={4}
          tone="ink"
          label={t("onboardingFlow.practice.model.progressLabel")}
          valueText={bytes ?? undefined}
          testId="wb-practice-model-progress"
        />
      ) : null}
      <p className="m-0 text-[13px] leading-[18px] text-(--wb-ink-2)">
        {bytes && (
          <span
            className="me-1 tabular-nums"
            data-testid="wb-practice-model-bytes"
          >
            {bytes} ·
          </span>
        )}
        {t("onboardingFlow.practice.model.body")}
      </p>
      {failed && (
        <div>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => void commands.forkSpeechSetupRetry()}
            data-testid="wb-practice-model-retry"
          >
            {t("onboardingFlow.practice.model.retry")}
          </Button>
        </div>
      )}
    </section>
  );
}
