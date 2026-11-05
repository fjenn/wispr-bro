// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The home's one dismissible promo banner (S-15a, C-29): how to dictate,
 * on the W7 home photo (the reference banner carries no example chips). Built on the Hub `Banner` (S-13, C-23), whose
 * dismissal persists in `dismissed_banners`. The id is the one S-13's
 * placeholder home used, so a banner dismissed there stays dismissed.
 * Its "Show me how" opens S-32's whisper lesson (C-29: dictating quietly
 * around other people).
 */
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { openLesson } from "../tutorials/lessonRegistry";
import { WHISPER_LESSON } from "../tutorials/lessons/whisper";
import { Banner } from "../ui";
import { photos } from "@/assets/fork/art";

export const PROMO_BANNER_ID = "hub.dictation.start";

export function PromoBanner() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const appName = t("rust.appName");
  return (
    <Banner
      id={PROMO_BANNER_ID}
      image={photos.home.src}
      // The mic starts at 58 % of the photo: keep the text clear of it
      // (OT2-E, Mac wave 6 observation).
      imageSubjectAt={0.58}
      title={t("hub.pages.dictation.banner.title")}
      body={t("hub.pages.dictation.banner.body", { appName })}
      action={{
        label: t("hub.pages.dictation.banner.quiet"),
        onClick: () => openLesson(WHISPER_LESSON, "try"),
        testId: "wb-home-whisper",
      }}
    />
  );
}
