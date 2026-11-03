// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One-time popup for the feature a version introduces (S-18, C-202).
 *
 * Rust names the feature (`fork_notifications_feature`: the current
 * version's entry of `notify::catalog::FEATURES`, or null when Announcements
 * are off); the popup shows once, then its id (`feature.<id>`) goes into the
 * fork setting `dismissed_banners`, so it never returns, in any window or
 * after a relaunch. Closing it also marks its inbox entry read. "Show me"
 * runs the feature's action (a Hub page, or the notifications panel).
 *
 * It waits while a take runs, while View changes or any other dialog is
 * open, and briefly after the page loads (`featurePopupGate.ts`).
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/ui";
import { commands, type FeatureAnnouncement } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettingsStore } from "../settings/store";
import { Button } from "../ui";
import { useFeaturePopupDeferred } from "./featurePopupGate";
import { runHubAction, useInbox } from "./notifications";

export function FeaturePopup() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const status = useForkSettingsStore((s) => s.status);
  const dismissed = useForkSettingsStore((s) => s.settings?.dismissed_banners);
  const [feature, setFeature] = useState<FeatureAnnouncement | null>(null);
  const [closed, setClosed] = useState(false);
  const deferred = useFeaturePopupDeferred();

  useEffect(() => {
    if (status !== "ready") return;
    let cancelled = false;
    commands
      .forkNotificationsFeature()
      .then((result) => {
        if (!cancelled && result.status === "ok") setFeature(result.data);
      })
      .catch(() => {
        // No popup when the backend cannot say; the inbox still has it.
      });
    return () => {
      cancelled = true;
    };
  }, [status]);

  if (
    !feature ||
    closed ||
    deferred ||
    dismissed?.includes(feature.dismiss_id)
  ) {
    return null;
  }

  const close = (then?: () => void) => {
    setClosed(true);
    const { settings, setSetting } = useForkSettingsStore.getState();
    if (settings && !settings.dismissed_banners.includes(feature.dismiss_id)) {
      void setSetting("dismissed_banners", [
        ...settings.dismissed_banners,
        feature.dismiss_id,
      ]);
    }
    if (feature.entry_id !== null) {
      void commands
        .forkNotificationsMarkRead([feature.entry_id])
        .then(() => useInbox.getState().refresh())
        .catch(() => {
          // The entry stays unread; nothing else depends on it.
        });
    }
    then?.();
  };

  return (
    <Dialog
      open
      title={t(feature.title_key)}
      description={t("hub.featurePopup.label")}
      closeLabel={t("hub.featurePopup.dismiss")}
      onOpenChange={(open) => {
        if (!open) close();
      }}
      footer={
        <div className="flex justify-end gap-2">
          <Button
            data-testid="wb-feature-popup-dismiss"
            onClick={() => close()}
          >
            {t("hub.featurePopup.dismiss")}
          </Button>
          <Button
            variant="primary"
            data-testid="wb-feature-popup-open"
            // After the dialog has returned focus, so a panel it opens keeps it.
            onClick={() =>
              close(() => window.setTimeout(() => runHubAction(feature.action)))
            }
          >
            {t("hub.featurePopup.open")}
          </Button>
        </div>
      }
    >
      <p
        data-testid="wb-feature-popup"
        className="text-[14px] leading-5 text-(--wb-ink-2)"
      >
        {t(feature.body_key, { appName: t("rust.appName") })}
      </p>
    </Dialog>
  );
}
