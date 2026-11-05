// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hub notice while the dictation shortcut is lost (S-02, A-208): one line
 * and Fix, which opens the Fix sheet. Not dismissible: it goes away when
 * the shortcut works again. The tray item of the same problem shows the
 * Hub, so this is what the user lands on. It has the top priority in the
 * Hub's one notice slot (`HubNotices.tsx`).
 */
import { useTranslation } from "react-i18next";
import { TriangleAlert } from "../ui/icons";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { useShortcutHealth } from "../settings/shortcutHealth";
import type { HubNotice } from "./HubNotices";

export function useShortcutHealthNotice(): HubNotice | null {
  const { t } = useTranslation(FORK_NAMESPACE);
  const status = useShortcutHealth((s) => s.status);
  const openSheet = useShortcutHealth((s) => s.openSheet);
  if (!status || status.state !== "lost") return null;
  const key =
    status.reason === "tap_stopped"
      ? "shortcutHealth.banner.stopped"
      : "shortcutHealth.banner.lost";
  return {
    id: "shortcut",
    testId: "wb-shortcut-banner",
    role: "alert",
    icon: <TriangleAlert size={18} aria-hidden className="shrink-0" />,
    message: t(key),
    strong: true,
    actions: (
      <Button
        variant="primary"
        onClick={openSheet}
        data-testid="wb-shortcut-banner-fix"
      >
        {t("shortcutHealth.banner.fix")}
      </Button>
    ),
  };
}
