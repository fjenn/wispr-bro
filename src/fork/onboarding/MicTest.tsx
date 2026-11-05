// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Test your microphone (S-29a; Design W1 centred layout): one white test
 * card with the question, the live meter as large bars on a tinted well
 * (the meter starts by itself), and Change microphone, No and Yes. Yes
 * goes on; No opens the microphone list in the S-08 order.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { MicLevelMeter, MicrophonePicker } from "../settings/MicrophonePicker";
import { Button } from "../ui";
import { CentredLayout, TestCard } from "./Layout";

export function MicTest({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [answer, setAnswer] = useState<"no" | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);

  return (
    <CentredLayout
      title={t("onboardingFlow.mic.title")}
      subtitle={t("onboardingFlow.mic.guidance")}
    >
      <TestCard
        title={
          <h2 className="type-heading m-0" id="wb-mic-question">
            {t("onboardingFlow.mic.question")}
          </h2>
        }
        footer={
          <>
            <Button
              variant="secondary"
              size="sm"
              aria-expanded={pickerOpen}
              aria-controls="wb-onboarding-mic-list"
              onClick={() => setPickerOpen((open) => !open)}
              data-testid="wb-mic-change"
            >
              {t("onboardingFlow.mic.change")}
            </Button>
            <span className="flex-1" />
            <Button
              variant="secondary"
              size="sm"
              aria-pressed={answer === "no"}
              aria-describedby="wb-mic-question"
              onClick={() => {
                setAnswer("no");
                setPickerOpen(true);
              }}
              data-testid="wb-mic-no"
            >
              {t("onboardingFlow.no")}
            </Button>
            <Button
              variant="primary"
              size="sm"
              aria-describedby="wb-mic-question"
              onClick={onContinue}
              data-testid="wb-mic-yes"
            >
              {t("onboardingFlow.yes")}
            </Button>
          </>
        }
      >
        <MicLevelMeter autoStart={!preview} inert={preview} variant="well" />
        {answer === "no" && (
          <p
            className="m-0 text-[13px] leading-[18px] text-(--wb-ink-2)"
            role="status"
          >
            {t("onboardingFlow.mic.noHelp")}
          </p>
        )}
        {pickerOpen && (
          <div
            id="wb-onboarding-mic-list"
            className="max-h-[200px] w-full overflow-y-auto border-t border-(--wb-line) pt-4 text-start"
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
      </TestCard>
    </CentredLayout>
  );
}
