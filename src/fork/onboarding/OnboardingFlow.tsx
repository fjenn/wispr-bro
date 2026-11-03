// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * First-run onboarding (S-29a): Welcome › Permissions › Setup › Practice,
 * with a Back link and a Help chip. Replaces Handy's flow through the one
 * `App.tsx` import (ci/hooks.txt); Handy's onboarding files are untouched.
 *
 * - Every step change is persisted first (`fork_onboarding_go_to`, fork
 *   settings › onboarding), so a relaunch resumes at the same step.
 * - No step reads or waits for the speech-model download (DEC-52: it runs
 *   from Welcome on; takes made before it is ready are queued by S-07a/c).
 *   No screen names a model (B-207).
 * - Preview (debug panel) is inert: no prompts, monitoring, moves or
 *   setting writes.
 * - S-29b appends Practice; until then the shortcut test finishes setup.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { OnboardingStep as HandyPreviewStep } from "@/components/onboarding/onboardingStore";
import { usePermissions } from "@/components/onboarding/usePermissions";
import { commands as handyCommands } from "@/bindings";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { FOCUS_RING } from "../ui";
import { HelpChip } from "./Help";
import { Languages } from "./Languages";
import { MicTest } from "./MicTest";
import { MoveToApplications } from "./MoveToApplications";
import { Permissions } from "./Permissions";
import { ShortcutTest } from "./ShortcutTest";
import { Stepper } from "./Stepper";
import { BACK, isOnboardingStep, type OnboardingStep } from "./steps";
import { Welcome } from "./Welcome";
import "../styles/fonts.css";
import "../styles/tokens.css";

/** The debug panel's preview entries (Handy's step ids) → our steps. */
const PREVIEW_STEPS: Record<HandyPreviewStep, OnboardingStep> = {
  welcome: "welcome",
  permissions: "permissions",
  model: "languages",
  demo: "mic_test",
  try: "shortcut_test",
  done: "shortcut_test",
};

export interface OnboardingFlowProps {
  /** Inert debug preview: no prompts, downloads, monitoring or setting writes. */
  preview?: boolean;
  previewStep?: HandyPreviewStep;
  onComplete: () => void;
}

export function OnboardingFlow({
  preview = false,
  previewStep = "welcome",
  onComplete,
}: OnboardingFlowProps) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings } = useForkSettings();
  const permissions = usePermissions(preview);
  const [step, setStep] = useState<OnboardingStep | null>(
    preview ? PREVIEW_STEPS[previewStep] : null,
  );
  const [helpOpen, setHelpOpen] = useState(false);
  const [navBusy, setNavBusy] = useState(false);
  const [navigationFailed, setNavigationFailed] = useState(false);
  const initialized = useRef(false);
  const finished = useRef(false);
  const heading = useRef<HTMLElement>(null);

  const go = useCallback(
    async (next: OnboardingStep): Promise<boolean> => {
      setNavigationFailed(false);
      if (preview) {
        setStep(next);
        return true;
      }
      setNavBusy(true);
      const result = await commands.forkOnboardingGoTo(next);
      setNavBusy(false);
      if (result.status === "error") {
        console.warn("onboarding: cannot save the step", result.error);
        setNavigationFailed(true);
        return false;
      }
      setStep(next);
      return true;
    },
    [preview],
  );

  // Resume: the saved step while in progress, else Welcome.
  useEffect(() => {
    if (preview || initialized.current || !settings) return;
    initialized.current = true;
    const saved = settings.onboarding;
    if (
      saved.status === "in_progress" &&
      isOnboardingStep(saved.current_step)
    ) {
      setStep(saved.current_step);
    } else {
      void go("welcome");
    }
  }, [go, preview, settings]);

  // DEC-02: the first speech model starts downloading from Welcome on, from
  // the macOS languages; Languages re-picks it. Nothing here waits for it.
  useEffect(() => {
    if (!preview) void commands.forkSpeechSetupRefresh();
  }, [preview]);

  // Each new step moves focus to its heading (keyboard and VoiceOver users
  // land on the new screen, not on the button that is gone).
  useEffect(() => {
    if (!step) return;
    const h1 = heading.current?.querySelector("h1");
    if (h1) {
      h1.setAttribute("tabindex", "-1");
      h1.focus({ preventScroll: true });
    }
  }, [step]);

  const leaveWelcome = async () => {
    if (preview) {
      await go("languages");
      return;
    }
    const location = await commands.forkOnboardingAppLocation();
    await go(
      location.status === "ok" && location.data === "move_needed"
        ? "move_to_applications"
        : "languages",
    );
  };

  const toLanguages = useCallback(() => void go("languages"), [go]);

  const finish = async () => {
    if (preview) {
      onComplete();
      return;
    }
    if (finished.current) return;
    setNavBusy(true);
    setNavigationFailed(false);
    const forkResult = await commands.forkOnboardingComplete();
    const handyResult =
      forkResult.status === "ok"
        ? await handyCommands.completeOnboarding()
        : forkResult;
    setNavBusy(false);
    if (forkResult.status === "error" || handyResult.status === "error") {
      setNavigationFailed(true);
      return;
    }
    finished.current = true;
    onComplete();
  };

  let content = null;
  if (step && permissions.checked) {
    switch (step) {
      case "welcome":
        content = <Welcome onContinue={() => void leaveWelcome()} />;
        break;
      case "move_to_applications":
        content = <MoveToApplications preview={preview} onDone={toLanguages} />;
        break;
      case "languages":
        content = (
          <Languages
            preview={preview}
            onContinue={() => void go("permissions")}
          />
        );
        break;
      case "permissions":
        content = (
          <Permissions
            permissions={permissions}
            preview={preview}
            onContinue={() => void go("mic_test")}
          />
        );
        break;
      case "mic_test":
        content = (
          <MicTest
            preview={preview}
            onContinue={() => void go("shortcut_test")}
          />
        );
        break;
      case "shortcut_test":
        content = (
          <ShortcutTest preview={preview} onContinue={() => void finish()} />
        );
        break;
    }
  }

  const back = step ? BACK[step] : undefined;

  return (
    <div
      dir={getLanguageDirection(i18n.language)}
      className="flex h-screen w-full cursor-default flex-col overflow-hidden bg-(--wb-canvas) text-(--wb-ink) select-none"
      data-testid="wb-onboarding"
      data-step={step ?? "loading"}
    >
      <header className="flex shrink-0 items-center gap-3 px-4 py-4 sm:px-6">
        {step ? <Stepper current={step} /> : <span className="flex-1" />}
        <HelpChip
          open={helpOpen}
          onOpen={() => setHelpOpen(true)}
          onClose={() => setHelpOpen(false)}
        />
      </header>
      <main
        ref={heading}
        className="min-h-0 flex-1 overflow-y-auto px-5 py-4 sm:px-8"
      >
        {content ? (
          <div key={step} className="min-h-full content-center py-2">
            {content}
          </div>
        ) : (
          <p
            className="m-0 text-center text-sm text-(--wb-ink-2)"
            role="status"
          >
            {t("onboardingFlow.loading")}
          </p>
        )}
      </main>
      <footer className="flex min-h-16 shrink-0 items-center justify-between gap-3 border-t border-(--wb-line) px-5 py-3 sm:px-7">
        <div>
          {back && (
            <button
              type="button"
              disabled={navBusy}
              onClick={() => void go(back)}
              data-testid="wb-onboarding-back"
              className={`cursor-pointer rounded-lg px-3 py-2 text-sm font-medium text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) disabled:cursor-default disabled:opacity-50 ${FOCUS_RING}`}
            >
              <span
                aria-hidden="true"
                className="me-1 rtl:inline-block rtl:-scale-x-100"
              >
                ‹
              </span>
              {t("onboardingFlow.back")}
            </button>
          )}
        </div>
        {navigationFailed ? (
          <p className="m-0 text-end text-xs text-(--wb-ink)" role="alert">
            {t("onboardingFlow.navigationFailed")}
          </p>
        ) : (
          <p className="m-0 text-end text-xs text-(--wb-ink-2)">
            {t("onboardingFlow.downloadHint")}
          </p>
        )}
      </footer>
    </div>
  );
}
