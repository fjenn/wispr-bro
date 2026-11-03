// SPDX-License-Identifier: GPL-3.0-or-later
import { Check } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { STAGES, stageFor, type OnboardingStep } from "./steps";

/** Welcome › Permissions › Setup › Practice; done stages carry a tick. */
export function Stepper({ current }: { current: OnboardingStep }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const stage = stageFor(current);
  const active = STAGES.indexOf(stage);
  return (
    <nav
      aria-label={t("onboardingFlow.stepper.label")}
      className="min-w-0 flex-1"
      data-testid="wb-onboarding-stepper"
    >
      <ol className="m-0 flex list-none items-center justify-center gap-1 p-0 sm:gap-2">
        {STAGES.map((item, index) => (
          <li
            key={item}
            aria-current={item === stage ? "step" : undefined}
            className={`flex min-w-0 items-center gap-1 sm:gap-2 ${
              item === stage
                ? "font-semibold text-(--wb-ink)"
                : index < active
                  ? "text-(--wb-ink-2)"
                  : "text-(--wb-ink-3)"
            }`}
          >
            {index > 0 && (
              <span aria-hidden="true" className="mx-0.5 text-xs sm:mx-1">
                ›
              </span>
            )}
            {index < active && (
              <Check size={14} aria-hidden="true" className="shrink-0" />
            )}
            <span className="truncate text-xs">
              {t(`onboardingFlow.stepper.${item}`)}
              {index < active && (
                <span className="sr-only">
                  {" "}
                  {t("onboardingFlow.stepper.done")}
                </span>
              )}
            </span>
          </li>
        ))}
      </ol>
    </nav>
  );
}
