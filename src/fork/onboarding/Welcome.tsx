// SPDX-License-Identifier: GPL-3.0-or-later
import { ArrowRight, LockKeyhole, AudioLines } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

export function Welcome({ onContinue }: { onContinue: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div className="mx-auto flex max-w-[590px] flex-col items-center gap-6 text-center">
      <div
        className="grid size-20 place-items-center rounded-[24px] bg-(--wb-accent-soft) text-(--wb-accent)"
        aria-hidden="true"
      >
        <AudioLines size={40} strokeWidth={1.6} />
      </div>
      <div className="flex flex-col gap-3">
        <h1 className="m-0 font-(--wb-font-display) text-[38px] leading-[1.08] font-semibold tracking-[-0.02em] text-balance">
          {t("onboardingFlow.welcome.title")}
        </h1>
        <p className="m-0 text-[15px] leading-6 text-(--wb-ink-2) text-pretty">
          {t("onboardingFlow.welcome.subtitle")}
        </p>
      </div>
      <div className="flex w-full items-start gap-3 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-4 text-start shadow-(--wb-elev-1)">
        <LockKeyhole
          size={20}
          aria-hidden="true"
          className="mt-0.5 shrink-0 text-(--wb-accent)"
        />
        <div>
          <h2 className="m-0 text-sm font-semibold">
            {t("onboardingFlow.welcome.privacyTitle")}
          </h2>
          <p className="mt-1 mb-0 text-[13px] leading-5 text-(--wb-ink-2)">
            {t("onboardingFlow.welcome.privacy")}
          </p>
        </div>
      </div>
      <p className="m-0 text-xs leading-5 text-(--wb-ink-3)">
        {t("onboardingFlow.welcome.download")}
      </p>
      <Button variant="primary" className="px-5 py-2.5" onClick={onContinue}>
        {t("onboardingFlow.welcome.start")}
        <ArrowRight size={16} aria-hidden="true" className="rtl:-scale-x-100" />
      </Button>
    </div>
  );
}
