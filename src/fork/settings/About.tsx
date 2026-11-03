// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ShowWhatsNewOnUpdate } from "@/components/settings/ShowWhatsNewOnUpdate";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { UpdatePanel } from "../hub/UpdateStatus";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

export function About() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);
  return (
    <div className="flex flex-col gap-6" data-testid="wb-settings-about">
      <section className="rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-6">
        <h3 className="font-(--wb-font-display) text-2xl">
          {t("rust.appName")}
        </h3>
        <p className="mt-1 text-sm text-(--wb-ink-2)">
          {t("settingsModal.about.tagline")}
        </p>
        <p className="mt-4 font-mono text-sm" data-testid="wb-about-version">
          {t("settingsModal.about.version", { version: version ?? "…" })}
        </p>
        <Button
          className="mt-4"
          onClick={() => void openUrl("https://github.com/fjenn/wispr-bro")}
        >
          {t("settingsModal.about.source")}
        </Button>
      </section>
      <section
        aria-labelledby="wb-about-updates"
        className="flex flex-col gap-3 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-6"
      >
        <h3 id="wb-about-updates" className="text-base font-semibold">
          {t("settingsModal.about.updatesTitle")}
        </h3>
        <UpdatePanel />
      </section>
      <div data-handy-content>
        <SettingsGroup title={t("settingsModal.about.updates")}>
          <ShowWhatsNewOnUpdate descriptionMode="tooltip" grouped />
        </SettingsGroup>
      </div>
    </div>
  );
}
