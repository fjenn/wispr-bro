// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef } from "react";
import { CircleHelp, X } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";

export function HelpChip({
  open,
  onOpen,
  onClose,
}: {
  open: boolean;
  onOpen: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const element = dialog.current;
    if (open && element && !element.open) element.showModal();
  }, [open]);

  return (
    <>
      <button
        type="button"
        onClick={onOpen}
        className={`inline-flex shrink-0 cursor-pointer items-center gap-1.5 rounded-full border border-(--wb-line) bg-(--wb-card) px-3 py-1.5 text-xs font-medium text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
      >
        <CircleHelp size={16} aria-hidden="true" />
        {t("onboardingFlow.help.chip")}
      </button>
      {open && (
        <dialog
          ref={dialog}
          aria-labelledby="wb-onboarding-help-title"
          className="m-auto w-[min(440px,calc(100vw-32px))] max-w-none rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-canvas) p-0 text-(--wb-ink) shadow-(--wb-shadow) backdrop:bg-black/45"
          onCancel={(event) => {
            event.preventDefault();
            onClose();
          }}
        >
          <div className="flex items-center justify-between border-b border-(--wb-line) px-5 py-4">
            <h2
              id="wb-onboarding-help-title"
              className="m-0 text-lg font-semibold"
            >
              {t("onboardingFlow.help.title")}
            </h2>
            <button
              type="button"
              aria-label={t("onboardingFlow.help.close")}
              onClick={onClose}
              className={`grid size-8 cursor-pointer place-items-center rounded-lg text-(--wb-ink-2) hover:bg-(--wb-hover) ${FOCUS_RING}`}
            >
              <X size={18} aria-hidden="true" />
            </button>
          </div>
          <div className="flex flex-col gap-3 px-5 py-4 text-sm leading-6 text-(--wb-ink-2)">
            <p className="m-0">{t("onboardingFlow.help.body")}</p>
            <p className="m-0">{t("onboardingFlow.help.download")}</p>
          </div>
          <div className="flex justify-end border-t border-(--wb-line) px-5 py-3">
            <Button variant="primary" onClick={onClose}>
              {t("onboardingFlow.help.done")}
            </Button>
          </div>
        </dialog>
      )}
    </>
  );
}
