// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The onboarding steps (S-29a) and the stepper stage each one belongs to.
 * The ids are the ones `fork/onboarding/state.rs` persists, in the same
 * order (a Rust test reads this list). S-29b added the Practice stage:
 * the hands-free practice, the celebration, time saved, the typing test and
 * the summary.
 */
export const ONBOARDING_STEPS = [
  "welcome",
  "move_to_applications",
  "languages",
  "permissions",
  "mic_test",
  "shortcut_test",
  "practice",
  "first_dictation",
  "time_saved",
  "typing_test",
  "summary",
] as const;

export type OnboardingStep = (typeof ONBOARDING_STEPS)[number];
export type OnboardingStage =
  | "welcome"
  | "permissions"
  | "setup"
  | "practice"
  | "finish";

/** The header's five stages (Design W1: the reference's five-stage stepper). */
export const STAGES: readonly OnboardingStage[] = [
  "welcome",
  "permissions",
  "setup",
  "practice",
  "finish",
];

/** Steps on the always-dark stage (the celebration and what follows it). */
export const DARK_STEPS: readonly OnboardingStep[] = [
  "first_dictation",
  "typing_test",
  "time_saved",
];

const STAGE_OF: Record<OnboardingStep, OnboardingStage> = {
  welcome: "welcome",
  move_to_applications: "welcome",
  languages: "welcome",
  permissions: "permissions",
  mic_test: "setup",
  shortcut_test: "setup",
  practice: "practice",
  first_dictation: "practice",
  time_saved: "finish",
  typing_test: "practice",
  summary: "finish",
};

export function isOnboardingStep(value: unknown): value is OnboardingStep {
  return ONBOARDING_STEPS.includes(value as OnboardingStep);
}

export function stageFor(step: OnboardingStep): OnboardingStage {
  return STAGE_OF[step];
}

/**
 * Where Back goes. Move to Applications is kept only for resume
 * compatibility (alpha.3 handles the location at startup and maps a saved
 * step to Languages), so it is never a Back target.
 */
export const BACK: Partial<Record<OnboardingStep, OnboardingStep>> = {
  move_to_applications: "welcome",
  languages: "welcome",
  permissions: "languages",
  mic_test: "permissions",
  shortcut_test: "mic_test",
  practice: "shortcut_test",
  first_dictation: "practice",
  time_saved: "practice",
  typing_test: "first_dictation",
  summary: "time_saved",
};
