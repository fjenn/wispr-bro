// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hub navigation (S-13, C-01, C-204).
 *
 * One store holds the current page and an optional section inside it.
 * `navigate(page, section?)` is the deep-link API every slice uses (Help ›
 * Shortcuts, a checklist item, a notification, the tray): it switches the
 * page, and the page scrolls to or selects `section` (`useHubSection`).
 *
 * The location is mirrored in the URL hash, `#/hub/<page>[/<section>]`, so a
 * reload keeps the page and a link can open one. `#/fork/debug` (the debug
 * panel) and any other hash are left alone.
 *
 * The page list is `HUB_PAGES` in `pages.tsx`; this file knows only the ids.
 */
import { create } from "zustand";

export const HUB_PAGE_IDS = [
  "dictation",
  "insights",
  "dictionary",
  "snippets",
  "style",
  "transforms",
  "scratchpad",
  "share",
  "settings",
] as const;

export type HubPageId = (typeof HUB_PAGE_IDS)[number];

export const DEFAULT_HUB_PAGE: HubPageId = "dictation";

export function isHubPageId(value: unknown): value is HubPageId {
  return (
    typeof value === "string" &&
    (HUB_PAGE_IDS as readonly string[]).includes(value)
  );
}

export interface HubLocation {
  page: HubPageId;
  /** A section inside the page (an anchor or a tab id), or null. */
  section: string | null;
}

const HASH_PREFIX = "#/hub/";

/** `#/hub/<page>[/<section>]` → location; null for any other hash. */
export function parseHubHash(hash: string): HubLocation | null {
  if (!hash.startsWith(HASH_PREFIX)) return null;
  const [page, ...rest] = hash.slice(HASH_PREFIX.length).split("/");
  if (!isHubPageId(page)) return null;
  const section = rest.length > 0 ? decodeURIComponent(rest.join("/")) : "";
  return { page, section: section || null };
}

export function hubHash({ page, section }: HubLocation): string {
  return section
    ? `${HASH_PREFIX}${page}/${encodeURIComponent(section)}`
    : `${HASH_PREFIX}${page}`;
}

interface HubNavState extends HubLocation {
  /** Bumps on every navigate, so a page re-applies the same section. */
  seq: number;
  navigate: (page: HubPageId, section?: string | null) => void;
}

export const useHubNav = create<HubNavState>()((set, get) => ({
  page: DEFAULT_HUB_PAGE,
  section: null,
  seq: 0,
  navigate: (page, section = null) => {
    set({ page, section, seq: get().seq + 1 });
    writeHash({ page, section });
  },
}));

/** Opens `page` (and `section` inside it). Usable outside React. */
export function navigate(page: HubPageId, section?: string | null): void {
  useHubNav.getState().navigate(page, section);
}

function writeHash(location: HubLocation): void {
  if (typeof window === "undefined") return;
  const next = hubHash(location);
  if (window.location.hash === next) return;
  // replaceState: page switches are not browser history (no Back button in
  // the app), and it does not fire `hashchange`.
  history.replaceState(
    null,
    "",
    window.location.pathname + window.location.search + next,
  );
}

/**
 * Follows the URL hash: applies it once at mount and on every `hashchange`
 * (a link, the tray, a test). Mounted once by `ForkApp`.
 */
export function syncHubWithHash(): () => void {
  const apply = () => {
    const location = parseHubHash(window.location.hash);
    if (!location) return;
    const state = useHubNav.getState();
    if (state.page === location.page && state.section === location.section) {
      return;
    }
    useHubNav.setState({ ...location, seq: state.seq + 1 });
  };
  apply();
  window.addEventListener("hashchange", apply);
  return () => window.removeEventListener("hashchange", apply);
}
