// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Use it in your own apps" (S-30): after the first-use chooser, the home
 * shows this banner in the promo banner's place until the first take is
 * pasted into another app (the checklist's `try_in_app` tick) or the user
 * dismisses it. While the bar spotlight is on screen it says so and offers
 * to hide it; otherwise it repeats the hint and can show it again or reopen
 * the chooser.
 */
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { TRY_IN_APP } from "../hub/checklistItems";
import { useForkSettingsStore } from "../settings/store";
import { Banner } from "../ui";
import { isFirstUseChoice, OWN_APPS_BANNER_ID } from "../onboarding/firstUse";
import {
  hideHint,
  howKey,
  openChooser,
  showHint,
  useFirstUseStore,
} from "../onboarding/firstUseStore";
import { photos } from "@/assets/fork/art";

export function FirstUseBanner({ fallback }: { fallback: ReactNode }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const settings = useForkSettingsStore((s) => s.settings);
  const spotlight = useFirstUseStore((s) => s.spotlight);
  const unavailable = useFirstUseStore((s) => s.unavailable);
  const trigger = useFirstUseStore((s) => s.trigger);
  const choice = settings?.first_use_choice;
  const applies =
    settings != null &&
    isFirstUseChoice(choice) &&
    !settings.onboarding.checklist_done.includes(TRY_IN_APP) &&
    !settings.dismissed_banners.includes(OWN_APPS_BANNER_ID);
  if (!applies || !isFirstUseChoice(choice)) return <>{fallback}</>;

  const active = spotlight?.active === true;
  const body = active
    ? t("firstUse.banner.active")
    : unavailable
      ? t("firstUse.noBar")
      : t("firstUse.banner.body", {
          hint: t(`rust.spotlight.hint.${choice}`),
          how: t(howKey(trigger)),
        });
  return (
    <Banner
      id={OWN_APPS_BANNER_ID}
      image={photos.home.src}
      // The same photo as the promo banner: keep the text off the mic
      // (OT2-E.6, Mac wave 7).
      imageSubjectAt={0.58}
      title={t("firstUse.banner.title")}
      body={body}
      live
      action={
        active
          ? {
              label: t("firstUse.banner.stop"),
              onClick: hideHint,
              testId: "wb-first-use-stop",
            }
          : {
              label: t("firstUse.banner.again"),
              onClick: () => void showHint(choice),
              testId: "wb-first-use-again",
            }
      }
      secondaryAction={{
        label: t("firstUse.banner.other"),
        onClick: openChooser,
        testId: "wb-first-use-other",
      }}
    />
  );
}
