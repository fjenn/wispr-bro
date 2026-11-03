// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Insights page (S-25, C-50): tabs Usage and Voice, and "Save as
 * image" (C-56). The tab is the Hub section, so `navigate("insights",
 * "voice")` (the Home voice-profile button) opens the Voice tab and a
 * reload keeps it.
 *
 * Everything is computed on this Mac (`fork_insights_usage`,
 * `fork_insights_voice`) and refreshes live after every take. "Save as
 * image" renders the visible tab with a dated heading; the buttons are
 * left out of the picture.
 */
import { ChartColumn, ImageDown } from "../ui/icons";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { FORK_NAMESPACE } from "../i18n";
import { navigate, useHubNav } from "../hub/nav";
import { PageHeader } from "../hub/PageHeader";
import { Button, EmptyState, TabPanel, Tabs } from "../ui";
import { formatDay } from "./format";
import { saveAsImage } from "./saveImage";
import { UsageTab } from "./UsageTab";
import { useInsights } from "./useInsights";
import { VoiceTab } from "./VoiceTab";
import "./insights.css";
import { ForkArt } from "@/assets/fork/art";

type TabId = "usage" | "voice";

const isTab = (v: unknown): v is TabId => v === "usage" || v === "voice";

export function InsightsPage() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const requested = useHubNav((s) => s.section);
  const tab: TabId = isTab(requested) ? requested : "usage";
  const { usage, voice, status } = useInsights();
  const exportRef = useRef<HTMLDivElement>(null);
  const [saving, setSaving] = useState(false);
  const appName = t("rust.appName");
  const today = usage?.today ?? null;

  const save = async () => {
    const node = exportRef.current;
    if (!node || !today) return;
    setSaving(true);
    try {
      const outcome = await saveAsImage(
        node,
        t("insights.save.fileName", { appName, date: today }),
        t("insights.save.filter"),
      );
      if (outcome === "saved") toast.success(t("insights.save.done"));
      if (outcome === "failed") toast.error(t("insights.save.failed"));
    } finally {
      setSaving(false);
    }
  };

  const tabs = (["usage", "voice"] as TabId[]).map((id) => ({
    value: id,
    label: t(`insights.tabs.${id}`),
  }));

  return (
    <div
      data-testid="wb-insights"
      data-status={status}
      className="flex flex-col gap-5"
    >
      <PageHeader title={t("hub.pages.insights.title")}>
        <Button
          data-testid="wb-insights-save"
          onClick={() => void save()}
          disabled={!usage || saving || (tab === "voice" && !voice)}
          aria-busy={saving}
        >
          <ImageDown size={16} aria-hidden="true" />
          {t("insights.save.button")}
        </Button>
      </PageHeader>

      {!usage ? (
        status === "unavailable" ? (
          <EmptyState
            testId="wb-insights-unavailable"
            art={<ForkArt name="empty-insights" width={160} />}
            icon={ChartColumn}
            title={t("hub.pages.insights.empty.title")}
            body={t("insights.unavailable")}
            action={{
              label: t("hub.pages.insights.empty.action"),
              onClick: () => navigate("dictation"),
            }}
          />
        ) : (
          <div
            aria-busy="true"
            data-testid="wb-insights-loading"
            className="h-40"
          />
        )
      ) : (
        <Tabs
          value={tab}
          onValueChange={(v) => navigate("insights", v)}
          tabs={tabs}
          label={t("insights.tabs.label")}
        >
          <div
            ref={exportRef}
            data-testid="wb-insights-export"
            className="flex flex-col gap-4 pt-4"
          >
            <p
              className="text-[12.5px] text-(--wb-ink-2)"
              data-testid="wb-insights-asof"
            >
              {t("insights.asOf", {
                date: formatDay(usage.today, i18n.language),
              })}
            </p>
            <TabPanel value="usage">
              <UsageTab usage={usage} />
            </TabPanel>
            <TabPanel value="voice">
              {voice ? (
                <VoiceTab voice={voice} />
              ) : (
                <div aria-busy="true" className="h-40" />
              )}
            </TabPanel>
          </div>
        </Tabs>
      )}
    </div>
  );
}
