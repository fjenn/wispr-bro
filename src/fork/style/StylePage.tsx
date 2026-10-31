// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Style page (S-19a, R2 D-18): tabs Personal / Work / Email / Other /
 * Cleanup. Each group tab lists its apps and sites and its three presets
 * with before/after examples; Cleanup holds the S-12a level picker. The tab is
 * the page's hub section, so `navigate("style", "email")` opens a tab.
 */
import { useTranslation } from "react-i18next";
import type { StyleGroup } from "../bindings";
import { navigate, useHubNav } from "../hub/nav";
import { PageHeader } from "../hub/PageHeader";
import { FORK_NAMESPACE } from "../i18n";
import { TabPanel, Tabs } from "../ui";
import { CleanupTab } from "./CleanupTab";
import { GroupTab } from "./GroupTab";
import { STYLE_GROUPS, useStyleApps } from "./useStyle";

const TABS = [...STYLE_GROUPS, "cleanup"] as const;
type StyleTab = (typeof TABS)[number];

const isTab = (v: unknown): v is StyleTab =>
  typeof v === "string" && (TABS as readonly string[]).includes(v);

export function StylePage() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const requested = useHubNav((s) => s.section);
  const tab: StyleTab = isTab(requested) ? requested : "personal";
  // Loads the apps once for all tabs and keeps them live.
  useStyleApps();

  return (
    <div data-testid="wb-style" className="flex flex-col gap-4">
      <PageHeader title={t("hub.pages.style.title")} />
      <Tabs
        value={tab}
        onValueChange={(value) => navigate("style", value)}
        label={t("style.tabsLabel")}
        tabs={TABS.map((value) => ({
          value,
          label:
            value === "cleanup"
              ? t("style.cleanup.tab")
              : t(`style.groups.${value as StyleGroup}.tab`),
        }))}
      >
        {TABS.map((value) => (
          <TabPanel key={value} value={value}>
            {value === tab &&
              (value === "cleanup" ? (
                <CleanupTab />
              ) : (
                <GroupTab group={value} />
              ))}
          </TabPanel>
        ))}
      </Tabs>
    </div>
  );
}
