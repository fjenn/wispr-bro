// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Which websites a style group shows. The built-in site list
 * (`categories.json`, about 80 hosts) keeps matching silently; on screen a
 * group lists only what matters to this user: sites they added or moved
 * there, sites they actually dictated on, and a few well-known headline
 * sites so the tab shows what it covers. A headline site is left out when
 * an app of the same name already shows in the group's icon row.
 */
import type { StyleApp, StyleGroup, StyleSite } from "../bindings";

/** At most four per group, in this order. */
export const HEADLINE_SITES: Record<StyleGroup, readonly string[]> = {
  personal: [
    "web.whatsapp.com",
    "web.telegram.org",
    "discord.com",
    "instagram.com",
  ],
  work: ["app.slack.com", "teams.microsoft.com", "linkedin.com"],
  email: ["mail.google.com", "outlook.live.com"],
  other: ["docs.google.com", "notion.so"],
};

export function shownSites(
  sites: StyleSite[],
  group: StyleGroup,
  apps: StyleApp[],
): StyleSite[] {
  const inGroup = sites.filter((s) => s.group === group);
  const appNames = new Set(
    apps.filter((a) => a.group === group).map((a) => a.name.toLowerCase()),
  );
  const chosen = inGroup.filter((s) => s.user_group === group);
  const used = inGroup.filter((s) => s.user_group !== group && s.recent);
  const headline = HEADLINE_SITES[group]
    .map((host) => inGroup.find((s) => s.host === host))
    .filter(
      (s): s is StyleSite =>
        !!s &&
        s.user_group !== group &&
        !s.recent &&
        !appNames.has(s.name.toLowerCase()),
    );
  // One chip per name (Outlook has several hosts): the first one wins.
  const names = new Set<string>();
  return [...chosen, ...used, ...headline].filter((s) => {
    const key = s.name.toLowerCase();
    if (s.builtin && names.has(key)) return false;
    names.add(key);
    return true;
  });
}
