// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The home's one dismissible promo banner (S-15a, C-29): how to dictate,
 * with three things to try. Built on the Hub `Banner` (S-13, C-23), whose
 * dismissal persists in `dismissed_banners`. The id is the one S-13's
 * placeholder home used, so a banner dismissed there stays dismissed.
 */
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Banner } from "../ui";

export const PROMO_BANNER_ID = "hub.dictation.start";

export function PromoBanner() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const appName = t("rust.appName");
  return (
    <Banner
      id={PROMO_BANNER_ID}
      title={t("hub.pages.dictation.banner.title")}
      body={t("hub.pages.dictation.banner.body", { appName })}
      examplesLabel={t("hub.pages.dictation.banner.examplesLabel")}
      examples={[
        t("hub.pages.dictation.banner.examples.reply"),
        t("hub.pages.dictation.banner.examples.notes"),
        t("hub.pages.dictation.banner.examples.prompt"),
      ]}
    />
  );
}
