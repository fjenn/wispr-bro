// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Hub's one notice slot, above every page. Shell notices never stack:
 * the highest-priority one shows (shortcut broken > Secure Input > the
 * one-time Apple Dictation note) with "N more", which steps to the next
 * one in turn; dismissing or fixing the shown one brings up the next. Each
 * notice keeps its own actions and dismissal rule.
 */
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { useShortcutHealthNotice } from "./ShortcutHealthBanner";
import { useSecureInputNotice } from "./SecureInputNotice";
import { useAppleDictationNotice } from "./AppleDictationNote";

export interface HubNotice {
  /** Stable id within the slot. */
  id: string;
  testId: string;
  /** `alert` only for a problem that stops dictation now. */
  role: "alert" | "status";
  icon: ReactNode;
  message: ReactNode;
  /** Medium weight for problems; regular for information. */
  strong?: boolean;
  actions: ReactNode;
}

export function HubNotices() {
  const { t } = useTranslation(FORK_NAMESPACE);
  // Priority order. All hooks run every render.
  const shortcut = useShortcutHealthNotice();
  const secure = useSecureInputNotice();
  const apple = useAppleDictationNotice();
  const notices = [shortcut, secure, apple].filter(
    (n): n is HubNotice => n !== null,
  );
  const [picked, setPicked] = useState<string | null>(null);

  if (notices.length === 0) return null;
  const index = Math.max(
    0,
    notices.findIndex((n) => n.id === picked),
  );
  const notice = notices[index];
  const others = notices.length - 1;
  const next = notices[(index + 1) % notices.length];

  return (
    <div
      role={notice.role}
      data-testid={notice.testId}
      data-notice={notice.id}
      className="flex flex-wrap items-center gap-3 rounded-(--wb-radius-md) border border-(--wb-line-strong) bg-(--wb-surface) px-4 py-3 text-(--wb-ink)"
    >
      {notice.icon}
      <p
        className={`m-0 min-w-0 flex-1 text-[14px] leading-5 ${notice.strong ? "font-medium" : ""}`}
      >
        {notice.message}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        {notice.actions}
        {others > 0 && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setPicked(next.id)}
            data-testid="wb-hub-notice-next"
          >
            <span className="sr-only">{t("hub.notices.nextLabel")} </span>
            {t("hub.notices.more", { count: others })}
          </Button>
        )}
      </div>
    </div>
  );
}
