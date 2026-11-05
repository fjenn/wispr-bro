// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Retake typing test" (owner request after S-29b): onboarding's 30-second
 * typing test (`onboarding/TypingTest.tsx`) in a dialog, opened from
 * Insights › Usage and Settings › General. Recalculate saves the result to
 * `typing_wpm`, which Insights' speed comparison and time saved read; ✕,
 * Esc or a click outside leave the stored speed as it was. Esc closes only
 * this dialog, not Settings under it.
 */
import { useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Title } from "../onboarding/Layout";
import { TypingTestPanel } from "../onboarding/TypingTest";
import { Dialog } from "../ui";

export function TypingTestDialog({
  open,
  onOpenChange,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** After `typing_wpm` was saved, before the dialog closes. */
  onSaved?: (wpm: number) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  // Opened inside Settings' native modal <dialog>, it must portal into it.
  const anchor = useRef<HTMLSpanElement>(null);
  const [container, setContainer] = useState<HTMLElement | null>(null);
  useLayoutEffect(() => {
    setContainer(anchor.current?.closest("dialog") ?? null);
  }, []);
  return (
    <>
      <span ref={anchor} hidden />
      <Dialog
        container={container}
        open={open}
        onOpenChange={onOpenChange}
        testId="wb-typing-dialog"
        closeTestId="wb-typing-dialog-close"
        closeLabel={t("onboardingFlow.typing.skip")}
        className="max-w-[640px]"
        contentFades={false}
        title={
          <span className="type-sheet block text-[28px] leading-[34px] font-normal text-balance">
            <Title i18nKey="onboardingFlow.typing.title" />
          </span>
        }
      >
        {open && (
          <TypingTestPanel
            preview={false}
            heading={false}
            onDone={(wpm) => {
              onSaved?.(wpm);
              onOpenChange(false);
            }}
          />
        )}
      </Dialog>
    </>
  );
}
