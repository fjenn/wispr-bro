// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The header's stage list (Design W1): Welcome › Permissions › Setup ›
 * Practice › Finish in the shared `Stepper` (uppercase, tracked, chevrons;
 * current and past stages in ink). Past stages say "done" to screen
 * readers.
 */
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Stepper as StageList } from "../ui";
import { STAGES, stageFor, type OnboardingStep } from "./steps";

export function Stepper({ current }: { current: OnboardingStep }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <nav
      aria-label={t("onboardingFlow.stepper.label")}
      className="flex min-w-0 justify-center"
      data-testid="wb-onboarding-stepper"
    >
      <StageList
        label={t("onboardingFlow.stepper.label")}
        current={stageFor(current)}
        doneLabel={t("onboardingFlow.stepper.done")}
        stages={STAGES.map((id) => ({
          id,
          label: t(`onboardingFlow.stepper.${id}`),
        }))}
        className="m-0 min-w-0 list-none gap-3 p-0"
      />
    </nav>
  );
}
