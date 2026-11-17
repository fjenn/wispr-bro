// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * First-run onboarding on iPhone. The frame, pieces and tone are the Mac
 * flow's (`../../onboarding`); the steps are the phone's:
 *
 *   welcome › languages › microphone › keyboard › model › practice › done
 *
 * - The speech model starts downloading at Welcome (the Mac's DEC-02), so
 *   the Model step is usually short.
 * - Keyboard, Model and Practice can be skipped: Home keeps a setup card
 *   until microphone, keyboard, Full Access and model are all in place.
 * - The step is kept in `localStorage` so a relaunch (the trip to iOS
 *   Settings can end the app) resumes where the user was.
 * - `preview` (debug) is inert: no prompts and no setting writes.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, type Variants } from "motion/react";
import { useTranslation } from "react-i18next";
import { commands as handyCommands } from "@/bindings";
import { ForkArt } from "@/assets/fork/art";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { commands } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { Languages } from "../../onboarding/Languages";
import { Title } from "../../onboarding/Layout";
import { useForkSettings } from "../../settings/useForkSettings";
import {
  BrandGlyph,
  Button,
  ProgressLine,
  TextArea,
  toast,
  useMotionPresets,
} from "../../ui";
import { Check, ChevronLeft, LockKeyhole, Mic } from "../../ui/icons";
import { iosCommands } from "../commands";
import { FullAccessNote, KeyboardSteps } from "../KeyboardSteps";
import { setupProgress, useIosLive, useIosStore } from "../useIos";
import { useKeyboardInset } from "../useKeyboardInset";
import { PhoneLayout } from "./PhoneLayout";
import "../../onboarding/onboarding.css";
import "../../styles/fonts.css";
import "../../styles/tokens.css";
import "../ios.css";

export const IOS_ONBOARDING_STEPS = [
  "welcome",
  "languages",
  "microphone",
  "keyboard",
  "model",
  "practice",
  "done",
] as const;
export type IosOnboardingStep = (typeof IOS_ONBOARDING_STEPS)[number];

export const IOS_ONBOARDING_STEP_KEY = "wb.ios.onboardingStep";

function isStep(value: unknown): value is IosOnboardingStep {
  return IOS_ONBOARDING_STEPS.includes(value as IosOnboardingStep);
}

function savedStep(): IosOnboardingStep {
  try {
    const value = localStorage.getItem(IOS_ONBOARDING_STEP_KEY);
    return isStep(value) ? value : "welcome";
  } catch {
    return "welcome";
  }
}

function saveStep(step: IosOnboardingStep | null): void {
  try {
    if (step) localStorage.setItem(IOS_ONBOARDING_STEP_KEY, step);
    else localStorage.removeItem(IOS_ONBOARDING_STEP_KEY);
  } catch {
    // Resuming is a convenience; the flow still runs.
  }
}

export interface IosOnboardingProps {
  /** Inert debug preview: no prompts, downloads or setting writes. */
  preview?: boolean;
  previewStep?: IosOnboardingStep;
  onComplete: () => void;
}

export function IosOnboarding({
  preview = false,
  previewStep = "welcome",
  onComplete,
}: IosOnboardingProps) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const [step, setStep] = useState<IosOnboardingStep>(() =>
    preview ? previewStep : savedStep(),
  );
  const [busy, setBusy] = useState(false);
  const direction = useRef<1 | -1>(1);
  const stage = useRef<HTMLDivElement>(null);
  const m = useMotionPresets();
  const rtl = getLanguageDirection(i18n.language) === "rtl";

  useForkSettings();
  useIosLive();
  useKeyboardInset();

  // The first speech model starts downloading now; no step waits for it
  // except Model itself.
  useEffect(() => {
    if (!preview) void commands.forkSpeechSetupRefresh();
  }, [preview]);

  const go = useCallback(
    (next: IosOnboardingStep) => {
      direction.current =
        IOS_ONBOARDING_STEPS.indexOf(next) < IOS_ONBOARDING_STEPS.indexOf(step)
          ? -1
          : 1;
      if (!preview) saveStep(next);
      setStep(next);
    },
    [preview, step],
  );

  // Each step starts at its heading (VoiceOver lands on the new screen).
  useEffect(() => {
    const h1 = stage.current?.querySelector("h1");
    if (h1) {
      h1.setAttribute("tabindex", "-1");
      h1.focus({ preventScroll: true });
    }
  }, [step]);

  const finish = async () => {
    if (preview) {
      onComplete();
      return;
    }
    setBusy(true);
    // The fork's own onboarding record is the Mac flow's; a phone build
    // that does not keep one must not hold the user here.
    try {
      const fork = await commands.forkOnboardingComplete();
      if (fork.status === "error") {
        console.warn("onboarding: fork completion failed", fork.error);
      }
    } catch (e) {
      console.warn("onboarding: fork completion failed", e);
    }
    try {
      const handy = await handyCommands.completeOnboarding();
      if (handy.status === "error") throw new Error(String(handy.error));
    } catch (e) {
      console.warn("onboarding: cannot complete", e);
      toast.error(t("onboardingFlow.navigationFailed"));
      setBusy(false);
      return;
    }
    saveStep(null);
    setBusy(false);
    onComplete();
  };

  const index = IOS_ONBOARDING_STEPS.indexOf(step);
  const back = index > 0 ? IOS_ONBOARDING_STEPS[index - 1] : null;
  const stepVariants: Variants = {
    hidden: (d: 1 | -1) => m.step(d).hidden as never,
    shown: (d: 1 | -1) => m.step(d).shown as never,
    gone: (d: 1 | -1) => m.step(d).gone as never,
  };

  let content;
  switch (step) {
    case "welcome":
      content = <Welcome onContinue={() => go("languages")} />;
      break;
    case "languages":
      content = (
        <Languages preview={preview} onContinue={() => go("microphone")} />
      );
      break;
    case "microphone":
      content = (
        <Microphone preview={preview} onContinue={() => go("keyboard")} />
      );
      break;
    case "keyboard":
      content = (
        <KeyboardStep preview={preview} onContinue={() => go("model")} />
      );
      break;
    case "model":
      content = (
        <Model onContinue={() => go("practice")} onSkip={() => go("done")} />
      );
      break;
    case "practice":
      content = <Practice preview={preview} onContinue={() => go("done")} />;
      break;
    case "done":
      content = <Done busy={busy} onFinish={() => void finish()} />;
      break;
  }

  const custom = direction.current * (rtl ? -1 : 1);
  return (
    <div
      dir={getLanguageDirection(i18n.language)}
      className="wb-ios-root wb-ios-onboarding"
      data-testid="wb-onboarding"
      data-step={step}
    >
      <header className="wb-ios-onboarding-bar">
        {back ? (
          <button
            type="button"
            className="wb-ios-back static!"
            aria-label={t("onboardingFlow.back")}
            data-testid="wb-onboarding-back"
            onClick={() => go(back)}
          >
            <ChevronLeft
              size={24}
              aria-hidden="true"
              className="rtl:-scale-x-100"
            />
          </button>
        ) : (
          <span className="w-11" />
        )}
        <div className="mx-auto w-[132px]">
          <ProgressLine
            value={(index + 1) / IOS_ONBOARDING_STEPS.length}
            label={t("onboardingFlow.stepper.label")}
            thickness={4}
            className="rounded-full"
          />
        </div>
        <span className="w-11" />
      </header>
      <main ref={stage} className="relative min-h-0 flex-1 overflow-hidden">
        {m.reduced ? (
          <motion.div
            key={step}
            className="h-full"
            {...m.page}
            exit={undefined}
          >
            {content}
          </motion.div>
        ) : (
          <AnimatePresence mode="wait" initial={false} custom={custom}>
            <motion.div
              key={step}
              className="h-full"
              custom={custom}
              variants={stepVariants}
              initial="hidden"
              animate="shown"
              exit="gone"
            >
              {content}
            </motion.div>
          </AnimatePresence>
        )}
      </main>
    </div>
  );
}

/* ── Welcome ───────────────────────────────────────────────────────── */

function Welcome({ onContinue }: { onContinue: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <PhoneLayout
      testId="wb-ios-onboarding-welcome"
      art={<ForkArt name="onboarding-welcome" width={230} />}
      title={<Title i18nKey="onboardingFlow.welcome.title" />}
      subtitle={t("ios.onboarding.welcome.subtitle")}
      action={
        <Button
          variant="primary"
          size="lg"
          block
          onClick={onContinue}
          data-testid="wb-ios-onboarding-start"
        >
          {t("onboardingFlow.welcome.start")}
        </Button>
      }
    >
      <div className="flex items-start gap-3 rounded-(--wb-radius-lg) border border-(--wb-line) bg-(--wb-surface-tint) p-4">
        <LockKeyhole
          size={20}
          aria-hidden="true"
          className="mt-0.5 shrink-0 text-(--wb-ink-2)"
        />
        <div>
          <h2 className="type-card-title m-0">
            {t("ios.onboarding.welcome.privacyTitle")}
          </h2>
          <p className="mt-1 mb-0 text-[15px] leading-[21px] text-(--wb-ink-2)">
            {t("onboardingFlow.welcome.privacy")}
          </p>
        </div>
      </div>
    </PhoneLayout>
  );
}

/* ── Microphone ────────────────────────────────────────────────────── */

function Microphone({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const permission = useIosStore((s) => s.setup?.micPermission ?? 0);
  const refresh = useIosStore((s) => s.refresh);
  const [busy, setBusy] = useState(false);
  const granted = permission === 2;
  const denied = permission === 1;

  const run = async (action: () => Promise<void>) => {
    if (preview) return;
    setBusy(true);
    try {
      await action();
    } catch (e) {
      console.warn("onboarding: microphone command failed", e);
      toast.error(t("ios.setup.failed"));
    }
    await refresh();
    setBusy(false);
  };

  return (
    <PhoneLayout
      testId="wb-ios-onboarding-microphone"
      title={t("ios.onboarding.microphone.title")}
      subtitle={t("ios.onboarding.microphone.subtitle")}
      action={
        granted ? (
          <Button
            variant="primary"
            size="lg"
            block
            onClick={onContinue}
            data-testid="wb-ios-onboarding-continue"
          >
            {t("onboardingFlow.continue")}
          </Button>
        ) : denied ? (
          <Button
            variant="primary"
            size="lg"
            block
            disabled={busy}
            onClick={() => void run(iosCommands.openSettings)}
            data-testid="wb-ios-onboarding-mic-settings"
          >
            {t("ios.setup.openSettings")}
          </Button>
        ) : (
          <Button
            variant="primary"
            size="lg"
            block
            disabled={busy}
            onClick={() => void run(iosCommands.requestMic)}
            data-testid="wb-ios-onboarding-mic-allow"
          >
            {t("ios.onboarding.microphone.allow")}
          </Button>
        )
      }
    >
      <div
        className="flex flex-1 flex-col items-center justify-center gap-5 pb-6"
        data-testid="wb-ios-onboarding-mic-state"
        data-permission={permission}
      >
        <span
          aria-hidden="true"
          className={`relative grid size-[120px] place-items-center rounded-full ${
            granted
              ? "bg-(--wb-accent-soft) text-(--wb-accent)"
              : "bg-(--wb-surface-tint) text-(--wb-ink-2)"
          }`}
        >
          <Mic size={48} />
          {granted && (
            <span className="absolute end-1 bottom-1 grid size-8 place-items-center rounded-full bg-(--wb-success) text-(--wb-surface) ring-4 ring-(--wb-panel)">
              <Check size={18} />
            </span>
          )}
        </span>
        <p
          role="status"
          className={`max-w-[300px] text-center text-[15px] leading-[21px] ${denied ? "text-(--wb-danger)" : "text-(--wb-ink-2)"}`}
        >
          {granted
            ? t("ios.onboarding.microphone.granted")
            : denied
              ? t("ios.onboarding.microphone.denied")
              : t("ios.onboarding.microphone.prompt")}
        </p>
      </div>
    </PhoneLayout>
  );
}

/* ── Keyboard ──────────────────────────────────────────────────────── */

function KeyboardStep({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const setup = useIosStore((s) => s.setup);
  const refresh = useIosStore((s) => s.refresh);
  const appName = t("rust.appName");
  const keyboard = !!setup?.keyboardEnabled;
  const fullAccess = !!setup?.fullAccess;
  const ready = keyboard && fullAccess;

  const openSettings = async () => {
    if (preview) return;
    try {
      await iosCommands.openSettings();
    } catch (e) {
      console.warn("onboarding: cannot open Settings", e);
      toast.error(t("ios.setup.failed"));
    }
    await refresh();
  };

  return (
    <PhoneLayout
      testId="wb-ios-onboarding-keyboard"
      title={t("ios.onboarding.keyboard.title", { appName })}
      subtitle={
        ready
          ? t("ios.onboarding.keyboard.ready")
          : t("ios.onboarding.keyboard.subtitle")
      }
      action={
        ready ? (
          <Button
            variant="primary"
            size="lg"
            block
            onClick={onContinue}
            data-testid="wb-ios-onboarding-continue"
          >
            {t("onboardingFlow.continue")}
          </Button>
        ) : (
          <Button
            variant="primary"
            size="lg"
            block
            onClick={() => void openSettings()}
            data-testid="wb-ios-onboarding-open-settings"
          >
            {t("ios.setup.openSettings")}
          </Button>
        )
      }
      secondary={
        !ready && (
          <button
            type="button"
            className="wb-ios-link self-center"
            onClick={onContinue}
            data-testid="wb-ios-onboarding-skip"
          >
            {t("ios.onboarding.later")}
          </button>
        )
      }
    >
      <div className="flex flex-col gap-4">
        <KeyboardSteps keyboardEnabled={keyboard} fullAccess={fullAccess} />
        <FullAccessNote compact />
      </div>
    </PhoneLayout>
  );
}

/* ── Speech model ──────────────────────────────────────────────────── */

function megabytes(bytes: number): string {
  return String(Math.max(1, Math.round(bytes / 1_000_000)));
}

function Model({
  onContinue,
  onSkip,
}: {
  onContinue: () => void;
  onSkip: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const speech = useIosStore((s) => s.speech);
  const refreshSpeech = useIosStore((s) => s.refreshSpeech);
  const phase = speech?.phase ?? "checking";
  const ready = phase === "ready";
  const failed = phase === "failed";
  const percent = ready ? 100 : Math.round(speech?.percent ?? 0);
  const minutes =
    speech?.eta_seconds != null
      ? Math.max(1, Math.round(speech.eta_seconds / 60))
      : null;

  const status = ready
    ? t("ios.onboarding.model.ready")
    : failed
      ? speech?.problem === "offline" || speech?.problem === "check_connection"
        ? t("ios.onboarding.model.offline")
        : speech?.problem === "no_space"
          ? t("ios.onboarding.model.noSpace", { mb: speech.needed_mb ?? 0 })
          : t("ios.onboarding.model.failed")
      : phase === "verifying"
        ? t("ios.onboarding.model.verifying")
        : phase === "downloading"
          ? t("ios.onboarding.model.downloading")
          : t("ios.onboarding.model.starting");

  return (
    <PhoneLayout
      testId="wb-ios-onboarding-model"
      title={
        ready
          ? t("ios.onboarding.model.titleReady")
          : t("ios.onboarding.model.title")
      }
      subtitle={t("ios.onboarding.model.subtitle")}
      action={
        failed ? (
          <Button
            variant="primary"
            size="lg"
            block
            onClick={() =>
              void commands.forkSpeechSetupRetry().then(refreshSpeech)
            }
            data-testid="wb-ios-onboarding-model-retry"
          >
            {t("ios.setup.model.retry")}
          </Button>
        ) : (
          <Button
            variant="primary"
            size="lg"
            block
            disabled={!ready}
            onClick={onContinue}
            data-testid="wb-ios-onboarding-continue"
          >
            {t("onboardingFlow.continue")}
          </Button>
        )
      }
      secondary={
        !ready && (
          <button
            type="button"
            className="wb-ios-link self-center"
            onClick={onSkip}
            data-testid="wb-ios-onboarding-skip"
          >
            {t("ios.onboarding.model.background")}
          </button>
        )
      }
    >
      <div
        className="rounded-(--wb-radius-lg) border border-(--wb-line) bg-(--wb-surface-tint) p-5"
        data-testid="wb-ios-onboarding-model-card"
        data-phase={phase}
      >
        <div className="flex items-baseline justify-between gap-3">
          <p
            role="status"
            className={`text-[16px] leading-[22px] font-medium ${failed ? "text-(--wb-danger)" : "text-(--wb-ink)"}`}
          >
            {status}
          </p>
          {!failed && (
            <p className="type-serif-title shrink-0 text-(--wb-ink) tabular-nums">
              {t("ios.onboarding.model.percent", { percent })}
            </p>
          )}
        </div>
        <ProgressLine
          value={percent / 100}
          label={t("ios.setup.model.title")}
          thickness={4}
          className="mt-4 rounded-full"
          testId="wb-ios-onboarding-model-progress"
        />
        <p className="mt-3 min-h-[18px] text-[13px] leading-[18px] text-(--wb-ink-3) tabular-nums">
          {!ready && !failed && speech && speech.total_bytes > 0
            ? [
                t("ios.onboarding.model.bytes", {
                  done: megabytes(speech.downloaded_bytes),
                  total: megabytes(speech.total_bytes),
                }),
                minutes != null
                  ? t("ios.onboarding.model.eta", { count: minutes })
                  : null,
              ]
                .filter(Boolean)
                .join(" · ")
            : ready
              ? t("ios.onboarding.model.offlineReady")
              : ""}
        </p>
      </div>
    </PhoneLayout>
  );
}

/* ── Practice ──────────────────────────────────────────────────────── */

function Practice({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const session = useIosStore((s) => s.session);
  const setup = useIosStore((s) => s.setup);
  const speech = useIosStore((s) => s.speech);
  const refresh = useIosStore((s) => s.refresh);
  const [text, setText] = useState("");
  const appName = t("rust.appName");
  const progress = setupProgress(setup, speech);
  const state = session?.state ?? "noSession";

  // The keyboard's mic needs a running session: start one for the take.
  const started = useRef(false);
  useEffect(() => {
    if (preview || started.current || !progress.done) return;
    started.current = true;
    void iosCommands
      .startSession()
      .catch((e) => console.warn("onboarding: cannot start a session", e))
      .then(refresh);
  }, [preview, progress.done, refresh]);

  const steps = [
    t("ios.onboarding.practice.steps.tap"),
    t("ios.onboarding.practice.steps.switch", { appName }),
    t("ios.onboarding.practice.steps.speak"),
  ];

  return (
    <PhoneLayout
      testId="wb-ios-onboarding-practice"
      title={t("ios.onboarding.practice.title")}
      subtitle={
        progress.done
          ? t("ios.onboarding.practice.subtitle")
          : t("ios.onboarding.practice.notReady")
      }
      action={
        <Button
          variant="primary"
          size="lg"
          block
          disabled={!text.trim()}
          onClick={onContinue}
          data-testid="wb-ios-onboarding-continue"
        >
          {t("onboardingFlow.continue")}
        </Button>
      }
      secondary={
        !text.trim() && (
          <button
            type="button"
            className="wb-ios-link self-center"
            onClick={onContinue}
            data-testid="wb-ios-onboarding-skip"
          >
            {t("ios.onboarding.practice.skip")}
          </button>
        )
      }
    >
      <div className="flex flex-col gap-4">
        <div className="relative">
          <TextArea
            value={text}
            onChange={(event) => setText(event.currentTarget.value)}
            rows={4}
            placeholder={t("ios.onboarding.practice.placeholder")}
            aria-label={t("ios.onboarding.practice.fieldLabel")}
            className="w-full resize-none rounded-(--wb-radius-lg)! pb-9!"
            data-testid="wb-ios-onboarding-practice-field"
          />
          {(state === "listening" || state === "transcribing") && (
            <p
              role="status"
              className="pointer-events-none absolute start-3 bottom-2.5 flex items-center gap-2 text-[13px] leading-[18px] font-semibold text-(--wb-accent)"
            >
              <span aria-hidden="true" className="wb-ios-dot wb-ios-dot-live" />
              {t(`ios.session.status.${state}`)}
            </p>
          )}
        </div>
        <ol className="flex flex-col gap-3">
          {steps.map((line, index) => (
            <li key={line} className="flex items-start gap-3">
              <span
                aria-hidden="true"
                className="grid size-6 shrink-0 place-items-center rounded-full bg-(--wb-secondary) text-[13px] font-semibold text-(--wb-ink) tabular-nums"
              >
                {index + 1}
              </span>
              <span className="text-[16px] leading-[24px] text-(--wb-ink)">
                {line}
              </span>
            </li>
          ))}
        </ol>
      </div>
    </PhoneLayout>
  );
}

/* ── Done ──────────────────────────────────────────────────────────── */

function Done({ busy, onFinish }: { busy: boolean; onFinish: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const setup = useIosStore((s) => s.setup);
  const speech = useIosStore((s) => s.speech);
  const progress = setupProgress(setup, speech);
  const appName = t("rust.appName");
  const points = [
    t("ios.onboarding.done.points.session"),
    t("ios.onboarding.done.points.keyboard", { appName }),
    t("ios.onboarding.done.points.private"),
  ];
  return (
    <PhoneLayout
      testId="wb-ios-onboarding-done"
      art={<ForkArt name="sticker-rocket" width={132} />}
      title={
        progress.done || !progress.known
          ? t("ios.onboarding.done.title")
          : t("ios.onboarding.done.titlePending")
      }
      subtitle={
        progress.done || !progress.known
          ? t("ios.onboarding.done.subtitle")
          : t("ios.onboarding.done.subtitlePending")
      }
      action={
        <Button
          variant="primary"
          size="lg"
          block
          disabled={busy}
          onClick={onFinish}
          data-testid="wb-ios-onboarding-finish"
        >
          {t("ios.onboarding.done.finish", { appName })}
        </Button>
      }
    >
      <ul className="flex flex-col gap-3">
        {points.map((line) => (
          <li key={line} className="flex items-start gap-3">
            <span
              aria-hidden="true"
              className="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-(--wb-ink) text-(--wb-on-ink)"
            >
              <BrandGlyph size={11} />
            </span>
            <span className="text-[16px] leading-[22px] text-(--wb-ink)">
              {line}
            </span>
          </li>
        ))}
      </ul>
    </PhoneLayout>
  );
}
