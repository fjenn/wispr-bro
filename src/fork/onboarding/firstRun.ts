// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-07a: the fork's speech setup inside Handy's onboarding flow, until
 * S-29a replaces that flow.
 *
 * - The user never picks a model: when the backend manages the speech
 *   model (product builds; `fork-speech-setup` is past "checking"), Handy's
 *   Model screen is skipped. A jump to it (All set › speech model, "try a
 *   faster model") lands on Welcome, where the languages that decide the
 *   model are.
 * - Demo and Try it count as ready when the fork says a speech model is
 *   ready (the starter is not always one of Handy's curated models, whose
 *   download state Handy's screens follow).
 * - Leaving Welcome re-checks the pick: the languages may have changed.
 *
 * Hook in `src/components/onboarding/OnboardingFlow.tsx` (`ci/hooks.txt`).
 */
import { useEffect, useRef, useState } from "react";
import "./firstRun.css";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { commands, events, type ForkSpeechSetup } from "../bindings";

/** Handy's onboarding steps the fork takes over. */
export const FORK_SKIPPED_STEPS: readonly string[] = ["model"];

/** Whether Handy's onboarding shows `step` (pure; see the module docs). */
export function forkShowsStep(step: string, managed: boolean): boolean {
  return !managed || !FORK_SKIPPED_STEPS.includes(step);
}

export interface ForkFirstRun {
  /** The fork manages the speech model (skip Handy's Model screen). */
  managed: boolean;
  /** A speech model is ready. */
  ready: boolean;
}

/** The speech setup as Handy's onboarding needs it. Inert in a preview. */
export function useForkFirstRun(step: string, preview: boolean): ForkFirstRun {
  const [setup, setSetup] = useState<ForkSpeechSetup | null>(null);

  useEffect(() => {
    if (preview) return;
    let alive = true;
    let off: UnlistenFn | null = null;
    void (async () => {
      const unlisten = await events.forkSpeechSetup.listen((e) =>
        setSetup(e.payload),
      );
      if (!alive) {
        unlisten();
        return;
      }
      off = unlisten;
      try {
        const now = await commands.forkSpeechSetupStatus();
        if (alive) setSetup((prev) => prev ?? now);
      } catch (e) {
        console.warn("fork: speech setup status failed", e);
      }
    })();
    return () => {
      alive = false;
      off?.();
    };
  }, [preview]);

  // Leaving Welcome: the languages were just saved.
  const last = useRef(step);
  useEffect(() => {
    if (!preview && last.current === "welcome" && step !== "welcome") {
      void commands.forkSpeechSetupRefresh();
    }
    last.current = step;
  }, [step, preview]);

  const managed = !preview && !!setup && setup.phase !== "checking";
  return { managed, ready: managed && setup?.phase === "ready" };
}
