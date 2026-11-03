// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The onboarding steps (S-29a) and the stepper stage each one belongs to.
 * The ids are the ones `fork/onboarding/state.rs` persists, in the same
 * order (a Rust test reads this list). S-29b appends its Practice steps.
 */
export const ONBOARDING_STEPS = [
  "welcome",
  "move_to_applications",
  "languages",
  "permissions",
  "mic_test",
  "shortcut_test",
] as const;

export type OnboardingStep = (typeof ONBOARDING_STEPS)[number];
export type OnboardingStage = "welcome" | "permissions" | "setup" | "practice";

export const STAGES: readonly OnboardingStage[] = [
  "welcome",
  "permissions",
  "setup",
  "practice",
];

const STAGE_OF: Record<OnboardingStep, OnboardingStage> = {
  welcome: "welcome",
  move_to_applications: "welcome",
  languages: "welcome",
  permissions: "permissions",
  mic_test: "setup",
  shortcut_test: "setup",
};

export function isOnboardingStep(value: unknown): value is OnboardingStep {
  return ONBOARDING_STEPS.includes(value as OnboardingStep);
}

export function stageFor(step: OnboardingStep): OnboardingStage {
  return STAGE_OF[step];
}

/**
 * Where Back goes. Move to Applications is never a Back target: the app
 * either moved (and relaunched) or did not need to.
 */
export const BACK: Partial<Record<OnboardingStep, OnboardingStep>> = {
  move_to_applications: "welcome",
  languages: "welcome",
  permissions: "languages",
  mic_test: "permissions",
  shortcut_test: "mic_test",
};
