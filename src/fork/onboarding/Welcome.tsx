// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Welcome (S-29a; Design W1 split layout): the promise and the privacy
 * line on the left, a live demo on the right (the first-use chooser's email
 * scene: the bar listens and the clean text lands), the black Get started
 * pinned low.
 */
import { LockKeyhole } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { SplitLayout } from "./Layout";
import { Scene } from "./scenes/Scene";

export function Welcome({ onContinue }: { onContinue: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <SplitLayout
      title={t("onboardingFlow.welcome.title")}
      subtitle={t("onboardingFlow.welcome.subtitle")}
      asideLabel={t("onboardingFlow.welcome.demoLabel")}
      aside={
        <div className="w-full max-w-[460px]">
          <Scene choice="email" />
        </div>
      }
      action={
        <Button variant="primary" size="lg" block onClick={onContinue}>
          {t("onboardingFlow.welcome.start")}
        </Button>
      }
    >
      <div className="flex items-start gap-3 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-4">
        <LockKeyhole
          size={18}
          aria-hidden="true"
          className="mt-0.5 shrink-0 text-(--wb-ink-2)"
        />
        <div>
          <h2 className="type-card-title m-0">
            {t("onboardingFlow.welcome.privacyTitle")}
          </h2>
          <p className="mt-1 mb-0 text-[15px] leading-5 text-(--wb-ink-2)">
            {t("onboardingFlow.welcome.privacy")}
          </p>
        </div>
      </div>
    </SplitLayout>
  );
}
