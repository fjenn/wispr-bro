// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hub notice while macOS Secure Input blocks the shortcuts. The rule for
 * when to warn, the per-episode dismissal and the strings are Handy's
 * (`src/components/SecureInputWarning.tsx`, same repository and licence);
 * this renders it in the fork's notice style, second in the Hub's one
 * notice slot (`HubNotices.tsx`).
 */
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type SecureInputStatus } from "@/bindings";
import { SECURE_INPUT_HELP_URL } from "@/components/SecureInputWarning";
import { Button } from "../ui";
import { ExternalLink, TriangleAlert } from "../ui/icons";
import type { HubNotice } from "./HubNotices";

export function useSecureInputNotice(): HubNotice | null {
  const { t } = useTranslation();
  const [status, setStatus] = useState<SecureInputStatus | null>(null);
  const [dismissed, setDismissed] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await commands.getSecureInputStatus());
    } catch (e) {
      console.warn("fork: secure input status failed", e);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const unlisten = listen<SecureInputStatus>(
      "secure-input-changed",
      (event) => setStatus(event.payload),
    );
    return () => {
      void unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, [refresh]);

  const impacted =
    status !== null &&
    ((status.sustained &&
      ((status.degraded_bindings?.length ?? 0) > 0 ||
        (status.uncovered_bindings?.length ?? 0) > 0)) ||
      status.recorder_blocked === true);

  // A dismissal lasts for the current episode only.
  useEffect(() => {
    if (!impacted) setDismissed(false);
  }, [impacted]);

  if (!impacted || dismissed) return null;

  const affectedCount = new Set([
    ...(status.uncovered_bindings ?? []),
    ...(status.degraded_bindings ?? []),
  ]).size;
  const countSuffix = affectedCount === 1 ? "one" : "other";
  const culprit = status.culprit_name ?? null;
  const message =
    affectedCount > 0
      ? culprit !== null
        ? t(`secureInput.blockedWithCulprit_${countSuffix}`, {
            name: culprit,
            count: affectedCount,
          })
        : t(`secureInput.blockedNoCulprit_${countSuffix}`, {
            count: affectedCount,
          })
      : culprit !== null
        ? t("secureInput.recorderBlockedWithCulprit", { name: culprit })
        : t("secureInput.recorderBlockedNoCulprit");

  return {
    id: "secure-input",
    testId: "wb-secure-input-notice",
    role: "status",
    icon: <TriangleAlert size={18} aria-hidden className="shrink-0" />,
    message,
    strong: true,
    actions: (
      <>
        <Button
          variant="secondary"
          onClick={() => void openUrl(SECURE_INPUT_HELP_URL)}
          trailing={<ExternalLink size={14} aria-hidden />}
          data-testid="wb-secure-input-help"
        >
          {t("secureInput.learnMore")}
        </Button>
        <Button
          variant="primary"
          onClick={() => setDismissed(true)}
          data-testid="wb-secure-input-dismiss"
        >
          {t("secureInput.dismiss")}
        </Button>
      </>
    ),
  };
}
