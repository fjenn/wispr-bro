// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Dictation home (S-15a, C-02): the Hub's landing page.
 *
 *   wide (page ≥ 680 px)                 narrow
 *   ┌ greeting ─────────────────────┐    ┌ greeting ─────────┐
 *   ├ banner ───────────┬ stats ────┤    ├ stats (3 across) ─┤
 *   │ history           │ voice     │    ├ voice profile ────┤
 *   │                   │ profile   │    ├ banner ───────────┤
 *   └───────────────────┴───────────┘    └ history ──────────┘
 *
 * The layout follows the page's own width (a container query), not the
 * window's, so it also holds with the sidebar expanded at the minimum size.
 * The DOM order is greeting, rail, main column, which is also the narrow
 * reading order. The history column is S-15b's; until it lands an empty
 * state links to Settings › History.
 */
import { AudioLines } from "lucide-react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { navigate } from "../hub/nav";
import { EmptyState } from "../ui";
import { Greeting } from "./Greeting";
import { PromoBanner } from "./PromoBanner";
import { StatsRail } from "./StatsRail";
import { useStatsSummary } from "./useStatsSummary";
import { VoiceProfileBar } from "./VoiceProfileBar";

function HistorySlot({ hasDictations }: { hasDictations: boolean }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const keys = hasDictations
    ? {
        title: "home.history.title",
        body: "home.history.body",
        action: "home.history.action",
      }
    : {
        title: "hub.pages.dictation.empty.title",
        body: "hub.pages.dictation.empty.body",
        action: "hub.pages.dictation.empty.action",
      };
  return (
    <EmptyState
      testId="wb-empty-dictation"
      icon={AudioLines}
      title={t(keys.title)}
      body={t(keys.body)}
      action={{
        label: t(keys.action),
        onClick: () => navigate("settings", "history"),
      }}
    />
  );
}

export function Home() {
  const { summary, status } = useStatsSummary();
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div data-testid="wb-home" className="@container">
      <div className="grid grid-cols-1 gap-6 [grid-template-areas:'greet'_'rail'_'main'] @min-[680px]:grid-cols-[minmax(0,1fr)_240px] @min-[680px]:[grid-template-areas:'greet_greet'_'main_rail']">
        <div className="[grid-area:greet]">
          <Greeting />
        </div>
        <aside
          aria-label={t("home.stats.label")}
          data-testid="wb-home-rail"
          className="flex flex-col gap-4 [grid-area:rail] @min-[680px]:self-start"
        >
          <StatsRail summary={summary} status={status} />
          <VoiceProfileBar totalWords={summary?.total_words ?? null} />
        </aside>
        <div className="flex min-w-0 flex-col gap-6 [grid-area:main]">
          <PromoBanner />
          <HistorySlot hasDictations={(summary?.total_dictations ?? 0) > 0} />
        </div>
      </div>
    </div>
  );
}
