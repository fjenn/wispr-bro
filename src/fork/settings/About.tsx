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
import { Bug, ExternalLink, MessageSquareText } from "../ui/icons";
import { ShowWhatsNewOnUpdate } from "@/components/settings/ShowWhatsNewOnUpdate";
import { commands } from "../bindings";
import { HANDY_URL, LICENSE_URL, REPO_URL } from "../hub/links";
import { UpdatePanel } from "../hub/UpdateStatus";
import { FORK_NAMESPACE } from "../i18n";
import { Button, SettingGroup, SettingRow } from "../ui";
import { HandyRow, PanelCard } from "./Controls";

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
    <div className="flex flex-col gap-8" data-testid="wb-settings-about">
      <PanelCard>
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="flex min-w-0 flex-col gap-1">
            <h3 className="type-card-title">{appName}</h3>
            <p
              className="text-[14px] text-(--wb-ink-2) tabular-nums"
              data-testid="wb-about-version"
            >
              {t("settingsModal.about.version", { version: version ?? "…" })}
            </p>
            <p
              className="mt-2 max-w-md text-[14px] leading-5"
              data-testid="wb-about-license"
            >
              {t("settingsModal.about.license")}
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => open(REPO_URL)}
              data-testid="wb-about-source"
            >
              <ExternalLink size={14} />
              {t("settingsModal.about.source")}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => open(LICENSE_URL)}
              data-testid="wb-about-license-link"
            >
              <ExternalLink size={14} />
              {t("settingsModal.about.licenseLink")}
            </Button>
          </div>
        </div>
      </PanelCard>
      <SettingGroup label={t("settingsModal.about.updatesTitle")}>
        <PanelCard>
          <UpdatePanel />
        </PanelCard>
        <HandyRow>
          <ShowWhatsNewOnUpdate descriptionMode="inline" grouped />
        </HandyRow>
      </SettingGroup>
      <div data-testid="wb-about-support">
        <SettingRow
          title={t("settingsModal.about.supportTitle")}
          description={t("settingsModal.about.supportBody", { appName })}
          control={
            <>
              <Button
                variant="outline"
                size="sm"
                onClick={() => void commands.forkOpenIssue("problem")}
                data-testid="wb-about-report"
              >
                <Bug size={14} />
                {t("hub.help.reportProblem")}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => void commands.forkOpenIssue("feedback")}
                data-testid="wb-about-feedback"
              >
                <MessageSquareText size={14} />
                {t("hub.help.sendFeedback")}
              </Button>
            </>
          }
        />
      </div>
      <div data-testid="wb-about-credit">
        <SettingRow
          title={t("settingsModal.about.creditLink")}
          description={t("settingsModal.about.credit")}
          control={
            <Button
              variant="outline"
              size="sm"
              onClick={() => open(HANDY_URL)}
              data-testid="wb-about-handy"
            >
              <ExternalLink size={14} />
              {t("settingsModal.about.creditLink")}
            </Button>
          }
        />
      </div>
    </div>
  );
}
