// SPDX-License-Identifier: GPL-3.0-or-later
import { useState } from "react";
import { ChevronDown, ChevronUp, Mic } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { MicLevelMeter, MicrophonePicker } from "../settings/MicrophonePicker";
import { Button } from "../ui";

export function MicTest({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [answer, setAnswer] = useState<"yes" | "no" | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);

  return (
    <div className="mx-auto flex w-full max-w-[570px] flex-col items-center gap-5">
      <div className="text-center">
        <span className="mb-3 inline-grid size-12 place-items-center rounded-2xl bg-(--wb-accent-soft) text-(--wb-accent)">
          <Mic size={24} aria-hidden="true" />
        </span>
        <h1 className="m-0 font-(--wb-font-display) text-3xl font-semibold text-balance">
          {t("onboardingFlow.mic.title")}
        </h1>
        <p className="mt-2 mb-0 text-sm leading-6 text-(--wb-ink-2) text-pretty">
          {t("onboardingFlow.mic.guidance")}
        </p>
      </div>
      <div className="flex w-full flex-col items-center gap-4 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-5">
        <MicLevelMeter autoStart={!preview} inert={preview} />
        <fieldset className="m-0 flex flex-col items-center gap-3 border-0 p-0">
          <legend className="mb-1 text-center text-sm font-medium">
            {t("onboardingFlow.mic.question")}
          </legend>
          <div className="flex gap-2">
            <Button
              variant={answer === "yes" ? "primary" : "secondary"}
              aria-pressed={answer === "yes"}
              onClick={() => setAnswer("yes")}
              data-testid="wb-mic-yes"
            >
              {t("onboardingFlow.yes")}
            </Button>
            <Button
              variant={answer === "no" ? "primary" : "secondary"}
              aria-pressed={answer === "no"}
              onClick={() => {
                setAnswer("no");
                setPickerOpen(true);
              }}
              data-testid="wb-mic-no"
            >
              {t("onboardingFlow.no")}
            </Button>
          </div>
        </fieldset>
        {answer === "no" && (
          <p
            className="m-0 text-center text-xs leading-5 text-(--wb-ink-2)"
            role="status"
          >
            {t("onboardingFlow.mic.noHelp")}
          </p>
        )}
        <button
          type="button"
          aria-expanded={pickerOpen}
          aria-controls="wb-onboarding-mic-list"
          onClick={() => setPickerOpen((open) => !open)}
          className="inline-flex cursor-pointer items-center gap-1.5 rounded-lg px-2 py-1 text-xs font-medium text-(--wb-ink-2) underline decoration-(--wb-line-strong) underline-offset-4 hover:text-(--wb-ink) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--wb-accent)"
        >
          {pickerOpen ? (
            <ChevronUp size={14} aria-hidden="true" />
          ) : (
            <ChevronDown size={14} aria-hidden="true" />
          )}
          {t("onboardingFlow.mic.change")}
        </button>
        {pickerOpen && (
          <div
            id="wb-onboarding-mic-list"
            className="max-h-[240px] w-full overflow-y-auto border-t border-(--wb-line) pt-4 text-start"
            data-testid="wb-onboarding-mic-list"
          >
            {preview ? (
              <p className="m-0 text-sm text-(--wb-ink-2)">
                {t("onboardingFlow.mic.previewList")}
              </p>
            ) : (
              <MicrophonePicker showMeter={false} />
            )}
          </div>
        )}
      </div>
      <Button
        variant="primary"
        disabled={answer !== "yes"}
        onClick={onContinue}
        data-testid="wb-mic-continue"
      >
        {t("onboardingFlow.continue")}
      </Button>
    </div>
  );
}
