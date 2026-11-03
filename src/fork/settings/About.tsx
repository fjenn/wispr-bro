// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › About (S-14; S-26b, R2 C-104; S-28 updates): version, GPL-3.0
 * with the licence text, the source, the update panel (S-28), credit to
 * Handy (MIT) and how to support development (no paid plan: report a
 * problem or send feedback on GitHub).
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Bug, ExternalLink, MessageSquareText } from "lucide-react";
import { ShowWhatsNewOnUpdate } from "@/components/settings/ShowWhatsNewOnUpdate";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { commands } from "../bindings";
import { HANDY_URL, LICENSE_URL, REPO_URL } from "../hub/links";
import { UpdatePanel } from "../hub/UpdateStatus";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

const CARD =
  "rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-6";

function open(url: string): void {
  void openUrl(url).catch(() => undefined);
}

export function About() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const appName = t("rust.appName");
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);
  return (
    <div className="flex flex-col gap-6" data-testid="wb-settings-about">
      <section className={CARD}>
        <h3 className="font-(--wb-font-display) text-2xl">{appName}</h3>
        <p className="mt-1 text-sm text-(--wb-ink-2)">
          {t("settingsModal.about.tagline")}
        </p>
        <p className="mt-4 font-mono text-sm" data-testid="wb-about-version">
          {t("settingsModal.about.version", { version: version ?? "…" })}
        </p>
        <p className="mt-3 text-sm" data-testid="wb-about-license">
          {t("settingsModal.about.license")}
        </p>
        <div className="mt-4 flex flex-wrap gap-2">
          <Button onClick={() => open(REPO_URL)} data-testid="wb-about-source">
            <ExternalLink size={14} aria-hidden />
            {t("settingsModal.about.source")}
          </Button>
          <Button
            onClick={() => open(LICENSE_URL)}
            data-testid="wb-about-license-link"
          >
            <ExternalLink size={14} aria-hidden />
            {t("settingsModal.about.licenseLink")}
          </Button>
        </div>
      </section>
      <section
        aria-labelledby="wb-about-updates"
        className={`flex flex-col gap-3 ${CARD}`}
      >
        <h3 id="wb-about-updates" className="text-base font-semibold">
          {t("settingsModal.about.updatesTitle")}
        </h3>
        <UpdatePanel />
      </section>
      <section className={CARD} data-testid="wb-about-credit">
        <p className="text-sm">{t("settingsModal.about.credit")}</p>
        <Button
          className="mt-4"
          onClick={() => open(HANDY_URL)}
          data-testid="wb-about-handy"
        >
          <ExternalLink size={14} aria-hidden />
          {t("settingsModal.about.creditLink")}
        </Button>
      </section>
      <section className={CARD} data-testid="wb-about-support">
        <h3 className="text-[15px] font-semibold">
          {t("settingsModal.about.supportTitle")}
        </h3>
        <p className="mt-1 text-sm text-(--wb-ink-2)">
          {t("settingsModal.about.supportBody", { appName })}
        </p>
        <div className="mt-4 flex flex-wrap gap-2">
          <Button
            onClick={() => void commands.forkOpenIssue("problem")}
            data-testid="wb-about-report"
          >
            <Bug size={14} aria-hidden />
            {t("hub.help.reportProblem")}
          </Button>
          <Button
            onClick={() => void commands.forkOpenIssue("feedback")}
            data-testid="wb-about-feedback"
          >
            <MessageSquareText size={14} aria-hidden />
            {t("hub.help.sendFeedback")}
          </Button>
        </div>
      </section>
      <div data-handy-content>
        <SettingsGroup title={t("settingsModal.about.updates")}>
          <ShowWhatsNewOnUpdate descriptionMode="tooltip" grouped />
        </SettingsGroup>
      </div>
    </div>
  );
}
