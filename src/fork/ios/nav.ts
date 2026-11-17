// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * iPhone navigation on top of the Hub's own (`../hub/nav`): the same
 * `navigate(page, section)` calls and URL hashes, read as five tabs and a
 * few pushed pages.
 *
 *   tab         Hub location
 *   Home        dictation
 *   History     dictation / history   (the Mac shows History on Home)
 *   My Voice    dictation / voice     (dictionary, snippets, style are pushed)
 *   Scratchpad  scratchpad            (scratchpad / <note id> is the editor)
 *   Settings    settings              (settings / <section> is pushed)
 *
 * My Voice is three rows; each pushes its page, and Back (or the tab
 * again) returns to the rows. Insights is a pushed page, opened from Home;
 * Back returns to the tab it was opened from. A note's editor covers the tab bar. Pages the iPhone
 * does not have (Transforms, Share) open Home.
 */
import type { HubLocation, HubPageId } from "../hub/nav";

export const IOS_TABS = [
  "home",
  "history",
  "voice",
  "scratchpad",
  "settings",
] as const;
export type IosTab = (typeof IOS_TABS)[number];

/** The three pages My Voice opens, in the order of its rows. */
export const VOICE_PAGES = ["dictionary", "snippets", "style"] as const;
export type VoicePage = (typeof VOICE_PAGES)[number];

/** Where a tab opens. */
export function tabLocation(tab: IosTab): HubLocation {
  switch (tab) {
    case "home":
      return { page: "dictation", section: null };
    case "history":
      return { page: "dictation", section: "history" };
    case "voice":
      return { page: "dictation", section: "voice" };
    case "scratchpad":
      return { page: "scratchpad", section: null };
    case "settings":
      return { page: "settings", section: null };
  }
}

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
  | { kind: "voice"; page: VoicePage }
  | { kind: "note"; id: number }
  | { kind: "insights" }
  | { kind: "settings"; page: SettingsPageId };

const HIDDEN: readonly HubPageId[] = ["transforms", "share"];

/** `scratchpad / <id>`: the note to edit, or null for the list. */
function noteId(section: string | null): number | null {
  const id = Number(section);
  return section && Number.isSafeInteger(id) && id > 0 ? id : null;
}

export function screenFor({ page, section }: HubLocation): IosScreen {
  if (HIDDEN.includes(page)) return { kind: "tab", tab: "home" };
  switch (page) {
    case "dictation":
      return {
        kind: "tab",
        tab:
          section === "history"
            ? "history"
            : section === "voice"
              ? "voice"
              : "home",
      };
    case "dictionary":
    case "snippets":
    case "style":
      return { kind: "voice", page };
    case "scratchpad": {
      const id = noteId(section);
      return id ? { kind: "note", id } : { kind: "tab", tab: "scratchpad" };
    }
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
  if (screen.kind === "voice") return screen.page;
  return screen.kind;
}
