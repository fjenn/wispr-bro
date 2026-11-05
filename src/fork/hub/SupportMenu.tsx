// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Sidebar Support item (OT2-E, owner: "Support / donate should be more
 * prominent"): its own item at the bottom of the sidebar, beside Share with
 * a friend, Settings and Help, opening a menu of ways to help a free app
 * with no paid plan: Sponsor on GitHub (only once `SPONSOR_PUBLISHED`),
 * Star on GitHub, Contribute and Send feedback. Help › Get in touch has a
 * "Support Wispr Bro" entry that opens the same menu ("Or just both!").
 *
 * Every link is on GitHub and opens in the user's browser on a click only;
 * the app fetches nothing (R5 §1).
 */
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { create } from "zustand";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Menu, type MenuItem } from "../ui";
import { CodeXml, GitHubMark, HandHeart, MessageSquareText } from "../ui/icons";
import {
  CONTRIBUTING_URL,
  REPO_URL,
  SPONSOR_PUBLISHED,
  SPONSOR_URL,
} from "./links";

/** Open state, so Help › Get in touch can open the Support menu. */
export const useSupportMenu = create<{
  open: boolean;
  setOpen: (open: boolean) => void;
}>((set) => ({ open: false, setOpen: (open) => set({ open }) }));

function open(url: string): void {
  void openUrl(url).catch(() => undefined);
}

/** The menu's items; `sponsor` only when a Sponsors profile exists. */
export function supportItems(
  t: (key: string, opts?: Record<string, unknown>) => string,
  sponsor = SPONSOR_PUBLISHED,
): MenuItem[] {
  return [
    ...(sponsor
      ? [
          {
            id: "sponsor",
            label: t("hub.support.sponsor"),
            icon: HandHeart,
            onSelect: () => open(SPONSOR_URL),
          },
        ]
      : []),
    {
      id: "star",
      label: t("hub.support.star"),
      icon: GitHubMark,
      onSelect: () => open(REPO_URL),
    },
    {
      id: "contribute",
      label: t("hub.support.contribute"),
      icon: CodeXml,
      onSelect: () => open(CONTRIBUTING_URL),
    },
    {
      id: "support-feedback",
      label: t("hub.help.sendFeedback"),
      icon: MessageSquareText,
      onSelect: () => void commands.forkOpenIssue("feedback"),
    },
  ];
}

export function SupportMenu({ trigger }: { trigger: ReactElement }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const appName = t("rust.appName");
  const isOpen = useSupportMenu((s) => s.open);
  const setOpen = useSupportMenu((s) => s.setOpen);
  return (
    <Menu
      label={t("hub.support.title", { appName })}
      side="right"
      align="end"
      testId="wb-support-menu"
      open={isOpen}
      onOpenChange={setOpen}
      groups={[
        {
          label: t("hub.support.free"),
          items: supportItems(t),
        },
      ]}
      trigger={trigger}
    />
  );
}
