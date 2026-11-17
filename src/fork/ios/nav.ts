// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * iPhone navigation on top of the Hub's own (`../hub/nav`): the same
 * `navigate(page, section)` calls and URL hashes, read as five tabs and a
 * few pushed pages.
 *
 *   tab         Hub location
 *   Home        dictation
 *   History     dictation / history   (the Mac shows History on Home)
 *   Dictionary  dictionary
 *   Snippets    snippets
 *   Settings    settings              (settings / <section> is pushed)
 *
 * Style and Insights are pushed pages, opened from Home or Settings; Back
 * returns to the tab they were opened from. Pages the iPhone does not have
 * (Transforms, Scratchpad, Share) open Home.
 */
import type { HubLocation, HubPageId } from "../hub/nav";

export const IOS_TABS = [
  "home",
  "history",
  "dictionary",
  "snippets",
  "settings",
] as const;
export type IosTab = (typeof IOS_TABS)[number];

export const TAB_LOCATION: Record<IosTab, HubLocation> = {
  home: { page: "dictation", section: null },
  history: { page: "dictation", section: "history" },
  dictionary: { page: "dictionary", section: null },
  snippets: { page: "snippets", section: null },
  settings: { page: "settings", section: null },
};

/** Settings sub-pages (`#/hub/settings/<id>`). */
export const SETTINGS_PAGES = [
  "languages",
  "cleanup",
  "session",
  "sync",
  "privacy",
  "about",
  "licences",
] as const;
export type SettingsPageId = (typeof SETTINGS_PAGES)[number];

export type IosScreen =
  | { kind: "tab"; tab: IosTab }
  | { kind: "style" }
  | { kind: "insights" }
  | { kind: "settings"; page: SettingsPageId };

const HIDDEN: readonly HubPageId[] = ["transforms", "scratchpad", "share"];

export function screenFor({ page, section }: HubLocation): IosScreen {
  if (HIDDEN.includes(page)) return { kind: "tab", tab: "home" };
  switch (page) {
    case "dictation":
      return { kind: "tab", tab: section === "history" ? "history" : "home" };
    case "dictionary":
      return { kind: "tab", tab: "dictionary" };
    case "snippets":
      return { kind: "tab", tab: "snippets" };
    case "style":
      return { kind: "style" };
    case "insights":
      return { kind: "insights" };
    case "settings":
      return (SETTINGS_PAGES as readonly string[]).includes(section ?? "")
        ? { kind: "settings", page: section as SettingsPageId }
        : { kind: "tab", tab: "settings" };
    default:
      return { kind: "tab", tab: "home" };
  }
}

/** A stable id for a screen (page transitions, scroll reset, test ids). */
export function screenId(screen: IosScreen): string {
  if (screen.kind === "tab") return screen.tab;
  if (screen.kind === "settings") return `settings-${screen.page}`;
  return screen.kind;
}
