// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Hub's title strip (S-13, C-16, C-25; Design Phase 0): one 46 pt
 * window-drag strip across the whole window, on the chrome colour. The macOS
 * traffic lights sit in it (centred by `fork/hub/mod.rs`), then the sidebar
 * toggle; on the right, background-job and update status (only while there
 * is something to say; the version and the full update panel live in
 * Settings › About), the theme toggle and the notifications bell. No profile
 * menu: there are no accounts (R5 §1).
 *
 * The theme toggle flips between light and dark in one click and saves
 * Handy's `theme` setting (Settings › About keeps the System choice). From
 * System it switches to the opposite of what the OS shows now. The bell and
 * its panel are S-18's (`NotificationsPanel.tsx`).
 */
import { useEffect, useState } from "react";
import { Moon, Sun } from "../ui/icons";
import { useTranslation } from "react-i18next";
import type { Theme } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { applyTheme, getStoredTheme } from "@/lib/utils/theme";
import { FORK_NAMESPACE } from "../i18n";
import TranscriptionStatus from "@/components/footer/TranscriptionStatus";
import { IconButton } from "../ui";
import { navigate } from "./nav";
import { NotificationsBell } from "./NotificationsPanel";
import { UpdateStatus } from "./UpdateStatus";

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
    <IconButton
      data-testid="wb-theme-toggle"
      data-theme-showing={dark ? "dark" : "light"}
      aria-label={label}
      title={label}
      onClick={toggle}
    >
      {dark ? <Sun size={20} /> : <Moon size={20} />}
    </IconButton>
  );
}

/** Background jobs and update status, right of the drag area. */
function StripStatus() {
  return (
    <div
      data-testid="wb-titlebar-status"
      className="flex items-center gap-3 text-[13px] text-(--wb-ink-3)"
    >
      <span data-testid="wb-titlebar-jobs" className="flex items-center">
        <TranscriptionStatus
          onShowHistory={() => navigate("dictation", "history")}
        />
      </span>
      <UpdateStatus quiet />
    </div>
  );
}

export function TitleStrip({ start }: { start?: React.ReactNode }) {
  return (
    <header
      data-tauri-drag-region
      data-testid="wb-header"
      // Physical left/right: the traffic lights stay top-left in every
      // language, so the strip's layout does not mirror.
      dir="ltr"
      className="relative z-20 flex h-(--wb-titlebar-height) shrink-0 items-center gap-1 ps-[78px] pe-3"
    >
      {start}
      <div data-tauri-drag-region className="h-full min-w-0 flex-1" />
      <StripStatus />
      <div className="ms-2 flex items-center gap-0.5">
        <ThemeToggle />
        <NotificationsBell />
      </div>
    </header>
  );
}
