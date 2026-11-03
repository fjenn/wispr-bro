// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Tabs on Radix Tabs (R4 "UI primitives", MIT): arrow keys move between
 * tabs, Home/End jump, and the panel is labelled by its tab. Styled with the
 * Hub tokens; the selected tab is underlined in ink.
 *
 * ```tsx
 * <Tabs value={tab} onValueChange={setTab} label={t("...")}
 *   tabs={[{ value: "all", label: t("...") }, …]}>
 *   <TabPanel value="all">…</TabPanel>
 * </Tabs>
 * ```
 */
import * as RadixTabs from "@radix-ui/react-tabs";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { FOCUS_RING } from "./Button";

export interface TabItem {
  value: string;
  label: string;
}

export interface TabsProps {
  value: string;
  onValueChange: (value: string) => void;
  tabs: TabItem[];
  /** Accessible name of the tab list. */
  label: string;
  children: ReactNode;
  className?: string;
}

export function Tabs({
  value,
  onValueChange,
  tabs,
  label,
  children,
  className = "",
}: TabsProps) {
  // Radix sets `dir="ltr"` on the root unless told otherwise, which undid
  // the Hub's right-to-left layout inside every tab set (found in S-34).
  const { i18n } = useTranslation();
  return (
    <RadixTabs.Root
      dir={getLanguageDirection(i18n.language)}
      value={value}
      onValueChange={onValueChange}
      className={`flex flex-col ${className}`}
    >
      <RadixTabs.List
        aria-label={label}
        className="flex flex-wrap gap-1 border-b border-(--wb-line)"
      >
        {tabs.map((tab) => (
          <RadixTabs.Trigger
            key={tab.value}
            value={tab.value}
            className={`-mb-px cursor-pointer rounded-t-(--wb-radius-control) border-b-2 border-transparent px-3 py-2 text-[13px] font-medium text-(--wb-ink-2) hover:text-(--wb-ink) data-[state=active]:border-(--wb-ink) data-[state=active]:text-(--wb-ink) ${FOCUS_RING}`}
          >
            {tab.label}
          </RadixTabs.Trigger>
        ))}
      </RadixTabs.List>
      {children}
    </RadixTabs.Root>
  );
}

export function TabPanel({
  value,
  children,
  className = "",
}: {
  value: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <RadixTabs.Content
      value={value}
      className={`outline-none ${FOCUS_RING} ${className}`}
    >
      {children}
    </RadixTabs.Content>
  );
}
