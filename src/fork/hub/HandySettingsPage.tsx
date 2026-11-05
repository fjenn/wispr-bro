// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Handlers Handy's Debug section needs from `App.tsx` (onboarding control).
 * S-14 mounts that section inside Settings › Advanced.
 */
import { createContext } from "react";
import type { OnboardingStep } from "@/components/onboarding/onboardingStore";

export interface HandyAppHandlers {
  onPreviewOnboarding: (step: OnboardingStep) => void;
  onRunOnboarding: () => void;
  onRunModelMigration: () => void;
}

const noop = () => undefined;

export const HandyAppHandlersContext = createContext<HandyAppHandlers>({
  onPreviewOnboarding: noop,
  onRunOnboarding: noop,
  onRunModelMigration: noop,
});
