// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hub header (S-13, C-16, C-25): a window-drag strip across the top of the
 * page area with the theme toggle and the notifications bell on the right.
 * No profile menu: there are no accounts (R5 §1).
 *
 * The theme toggle flips between light and dark in one click and saves
 * Handy's `theme` setting (Settings › About keeps the System choice). From
 * System it switches to the opposite of what the OS shows now. The bell and
 * its panel are S-18's (`NotificationsPanel.tsx`).
 */
import { useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Theme } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { applyTheme, getStoredTheme } from "@/lib/utils/theme";
import { FORK_NAMESPACE } from "../i18n";
import { FOCUS_RING } from "../ui";
import { NotificationsBell } from "./NotificationsPanel";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function useSystemDark(): boolean {
  const [dark, setDark] = useState(
    () =>
      typeof window !== "undefined" && window.matchMedia(DARK_QUERY).matches,
  );
  useEffect(() => {
    const query = window.matchMedia(DARK_QUERY);
    const onChange = () => setDark(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);
  return dark;
}

const iconButton = `grid size-7 cursor-pointer place-items-center rounded-full text-(--wb-ink-2) transition-colors hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`;

function ThemeToggle() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, updateSetting } = useSettings();
  const systemDark = useSystemDark();
  const theme: Theme = settings?.theme ?? getStoredTheme();
  const dark = theme === "system" ? systemDark : theme === "dark";
  const label = dark ? t("hub.header.toLight") : t("hub.header.toDark");

  const toggle = () => {
    const next: Theme = dark ? "light" : "dark";
    applyTheme(next);
    void updateSetting("theme", next);
  };

  return (
    <button
      type="button"
      data-testid="wb-theme-toggle"
      data-theme-showing={dark ? "dark" : "light"}
      aria-label={label}
      title={label}
      onClick={toggle}
      className={iconButton}
    >
      {dark ? (
        <Sun size={17} aria-hidden="true" />
      ) : (
        <Moon size={17} aria-hidden="true" />
      )}
    </button>
  );
}

export function Header({ inset }: { inset: boolean }) {
  return (
    <header
      data-tauri-drag-region
      data-testid="wb-header"
      className={`flex h-(--wb-titlebar-height) shrink-0 items-center justify-end gap-1 pe-4 ${inset ? "pl-28" : "ps-4"}`}
    >
      <ThemeToggle />
      <NotificationsBell />
    </header>
  );
}
