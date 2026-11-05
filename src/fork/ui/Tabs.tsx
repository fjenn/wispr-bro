// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Tabs on Radix Tabs (R4 "UI primitives", MIT): arrow keys move between
 * tabs, Home/End jump, and the panel is labelled by its tab. Styled with the
 * Hub tokens (Design Phase 0): 15 px medium labels, the selected one in ink
 * over a 2 pt ink underline, the rest in ink-3.
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
  /** Beside the label (a Beta tag). */
  tag?: ReactNode;
}

export interface TabsProps {
  value: string;
  onValueChange: (value: string) => void;
  tabs: TabItem[];
  /** Accessible name of the tab list. */
  label: string;
  children: ReactNode;
  className?: string;
  /** Controls at the end of the tab row (search, sort, refresh). */
  end?: ReactNode;
}

export function Tabs({
  value,
  onValueChange,
  tabs,
  label,
  children,
  className = "",
  end,
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
      <div className="flex items-end gap-4 border-b border-(--wb-line)">
        <RadixTabs.List
          aria-label={label}
          className="flex min-w-0 flex-1 flex-wrap gap-6"
        >
          {tabs.map((tab) => (
            <RadixTabs.Trigger
              key={tab.value}
              value={tab.value}
              className={`-mb-px inline-flex cursor-pointer items-center gap-3 rounded-t-[4px] border-b-2 border-transparent pt-1 pb-3 text-[16px] font-medium text-(--wb-ink-2) transition-colors duration-(--wb-dur-fast) hover:text-(--wb-ink) data-[state=active]:border-(--wb-ink) data-[state=active]:text-(--wb-ink) ${FOCUS_RING}`}
            >
              {tab.label}
              {tab.tag}
            </RadixTabs.Trigger>
          ))}
        </RadixTabs.List>
        {end && (
          <div className="flex shrink-0 items-center gap-1 pb-1.5">{end}</div>
        )}
      </div>
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
    <RadixTabs.Content value={value} className={`${FOCUS_RING} ${className}`}>
      {children}
    </RadixTabs.Content>
  );
}
