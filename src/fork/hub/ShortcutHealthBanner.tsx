// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hub banner while the dictation shortcut is lost (S-02, A-208): one line
 * and Fix, which opens the Fix sheet. Not dismissible: it goes away when
 * the shortcut works again. The tray item of the same problem shows the
 * Hub, so this is what the user lands on.
 */
import { useTranslation } from "react-i18next";
import { TriangleAlert } from "../ui/icons";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { useShortcutHealth } from "../settings/shortcutHealth";

export function ShortcutHealthBanner() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const status = useShortcutHealth((s) => s.status);
  const openSheet = useShortcutHealth((s) => s.openSheet);
  if (!status || status.state !== "lost") return null;
  const key =
    status.reason === "tap_stopped"
      ? "shortcutHealth.banner.stopped"
      : "shortcutHealth.banner.lost";
  return (
    <div
      role="alert"
      data-testid="wb-shortcut-banner"
      className="flex items-center gap-3 rounded-(--wb-radius-md) border border-(--wb-line-strong) bg-(--wb-surface) px-4 py-3 text-(--wb-ink)"
    >
      <TriangleAlert size={18} aria-hidden className="shrink-0" />
      <p className="min-w-0 flex-1 text-[14px] leading-5 font-medium">
        {t(key)}
      </p>
      <Button
        variant="primary"
        onClick={openSheet}
        data-testid="wb-shortcut-banner-fix"
      >
        {t("shortcutHealth.banner.fix")}
      </Button>
    </div>
  );
}
