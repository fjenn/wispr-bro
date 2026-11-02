// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Hub's pages (S-13, C-01): sidebar order, icons, tags and the component
 * each page slot renders. A feature slice mounts its page by replacing the
 * `component` of its entry (S-15a Dictation: done, S-16 Dictionary: done,
 * S-17 Snippets: done, S-25 Insights: done, S-19a Style: done, S-22 Transforms, S-24 Scratchpad, S-26b Share,
 * S-14 Settings). Until then the slot shows a placeholder page with an empty
 * state and one clear action.
 */
import type { ComponentType } from "react";
import {
  AudioLines,
  BookA,
  ChartColumn,
  Heart,
  NotebookPen,
  Palette,
  Settings,
  TextQuote,
  WandSparkles,
  type LucideIcon,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { DictionaryPage } from "../dictionary/DictionaryPage";
import { Home } from "../home/Home";
import { InsightsPage } from "../insights/InsightsPage";
import { SnippetsPage } from "../snippets/SnippetsPage";
import { StylePage } from "../style/StylePage";
import { TransformsPage } from "../transforms/TransformsPage";
import { EmptyState, type TagKind } from "../ui";
import { SettingsModal } from "../settings/SettingsModal";
import { navigate, type HubPageId } from "./nav";
import { PageHeader } from "./PageHeader";

export interface HubPage {
  id: HubPageId;
  /** `main` pages are the feature list; `bottom` pages sit at the bottom. */
  group: "main" | "bottom";
  icon: LucideIcon;
  /** Optional tag beside the label. A `new` tag hides once the page is opened. */
  tag?: TagKind;
  component: ComponentType;
}

/** Key of a page's name in the `fork` namespace. */
export const pageLabelKey = (id: HubPageId) => `hub.pages.${id}.title`;

/** Placeholder until the page's own slice lands. */
function placeholder(
  id: HubPageId,
  icon: LucideIcon,
  action: { page: HubPageId; section?: string },
): ComponentType {
  function PlaceholderPage() {
    const { t } = useTranslation(FORK_NAMESPACE);
    const appName = t("rust.appName");
    return (
      <>
        <PageHeader title={t(pageLabelKey(id))} />
        <EmptyState
          testId={`wb-empty-${id}`}
          icon={icon}
          title={t(`hub.pages.${id}.empty.title`, { appName })}
          body={t(`hub.pages.${id}.empty.body`, { appName })}
          action={{
            label: t(`hub.pages.${id}.empty.action`),
            onClick: () => navigate(action.page, action.section),
          }}
        />
      </>
    );
  }
  PlaceholderPage.displayName = `PlaceholderPage(${id})`;
  return PlaceholderPage;
}

export const HUB_PAGES: HubPage[] = [
  {
    id: "dictation",
    group: "main",
    icon: AudioLines,
    component: Home,
  },
  {
    id: "insights",
    group: "main",
    icon: ChartColumn,
    tag: "new",
    component: InsightsPage,
  },
  {
    id: "dictionary",
    group: "main",
    icon: BookA,
    component: DictionaryPage,
  },
  {
    id: "snippets",
    group: "main",
    icon: TextQuote,
    component: SnippetsPage,
  },
  {
    id: "style",
    group: "main",
    icon: Palette,
    component: StylePage,
  },
  {
    id: "transforms",
    group: "main",
    icon: WandSparkles,
    tag: "beta",
    component: TransformsPage,
  },
  {
    id: "scratchpad",
    group: "main",
    icon: NotebookPen,
    tag: "beta",
    component: placeholder("scratchpad", NotebookPen, { page: "dictation" }),
  },
  {
    id: "share",
    group: "bottom",
    icon: Heart,
    component: placeholder("share", Heart, { page: "dictation" }),
  },
  {
    id: "settings",
    group: "bottom",
    icon: Settings,
    component: SettingsModal,
  },
];

export function hubPage(id: HubPageId): HubPage {
  return HUB_PAGES.find((p) => p.id === id) ?? HUB_PAGES[0];
}

/** Id stored in `seen_new_tags` when a page's "New!" tag has been seen. */
export const newTagId = (id: HubPageId) => `hub.page.${id}`;
