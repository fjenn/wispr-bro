// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Sync with your Mac. No account and no server: a file goes
 * from one device to the other (AirDrop), and the merge happens here.
 *
 *   Sync with your Mac
 *   ┌ From your Mac ────────────────────────────────┐
 *   │ 1 On your Mac: Settings › Data and Privacy …  │
 *   │ 2 AirDrop the file to this iPhone …           │
 *   │ 3 Check what will change, then Merge          │
 *   │ [ Choose a file ]                             │
 *   └────────────────────────────────────────────────┘
 *   ┌ To your Mac ──────────────────────────────────┐
 *   │ Send to Mac                                 ›  │
 *   │ Add CSV files                               ◉  │
 *   └────────────────────────────────────────────────┘
 *   ┌ Synced ───────────────────────────────────────┐
 *   │ Finn's MacBook Pro      17 Nov 2026, 14:02     │
 *   └────────────────────────────────────────────────┘
 *
 * A file opened in Wispr Bro from AirDrop or Files lands here by itself
 * (`useSyncInbox` in the app shell) with its review open.
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, events } from "../bindings";
import { formatCount } from "../home/StatsRail";
import { navigate } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import { deviceLabel, SyncReviewDialog } from "../sync/SyncReviewDialog";
import { formatWhen, useSync } from "../sync/useSync";
import { Button, toast } from "../ui";
import { FileText, Laptop, Share, Smartphone } from "../ui/icons";
import { IosGroup, IosSwitchRow, IosTitle } from "./parts";

const STEPS = [1, 2, 3] as const;

/**
 * Opens the Sync page when another app hands Wispr Bro a file. The page
 * then reviews it (`useSync` reads the waiting file on mount).
 */
export function useSyncInbox(): void {
  useEffect(() => {
    let alive = true;
    const go = () => alive && navigate("settings", "sync");
    void commands
      .forkSyncStatus()
      .then((result) => {
        if (result.status === "ok" && result.data?.pending_file) go();
      })
      .catch(() => undefined);
    const unlisten = events.forkSyncFileOpened.listen(go);
    return () => {
      alive = false;
      void unlisten.then((f) => f()).catch(() => undefined);
    };
  }, []);
}

export function IosSync() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const sync = useSync();
  const { status } = sync;
  const [withCsv, setWithCsv] = useState(false);
  const reading = sync.state.step === "reading";

  const send = async () => {
    const sent = await sync.send(null, withCsv);
    if (!sent) toast.error(t("sync.sendFailed"));
  };

  return (
    <div className="flex flex-col gap-6" data-testid="wb-ios-sync">
      <IosTitle>{t("ios.settings.pages.sync")}</IosTitle>
      <p className="text-[15px] leading-[21px] text-(--wb-ink-2)">
        {t("ios.sync.intro")}
      </p>

      <IosGroup
        label={t("ios.sync.fromMac.title")}
        footer={t("ios.sync.fromMac.footer")}
        testId="wb-ios-sync-from"
      >
        <ol className="flex flex-col gap-3.5 px-4 pt-4 pb-1">
          {STEPS.map((step) => (
            <li key={step} className="flex items-start gap-3">
              <span
                aria-hidden="true"
                className="grid size-6 shrink-0 place-items-center rounded-full bg-(--wb-accent-soft) text-[13px] leading-none font-semibold text-(--wb-accent) tabular-nums"
              >
                {step}
              </span>
              <span className="min-w-0 flex-1 text-[15px] leading-[21px] text-(--wb-ink)">
                {t(`ios.sync.fromMac.step${step}`)}
              </span>
            </li>
          ))}
        </ol>
        <div className="p-4">
          <Button
            variant="primary"
            size="lg"
            block
            disabled={reading}
            aria-busy={reading}
            onClick={() => void sync.pick()}
            data-testid="wb-ios-sync-import"
          >
            {t(
              reading ? "ios.sync.fromMac.reading" : "ios.sync.fromMac.choose",
            )}
          </Button>
        </div>
      </IosGroup>

      <IosGroup
        label={t("ios.sync.toMac.title")}
        footer={t("ios.sync.toMac.footer")}
        testId="wb-ios-sync-to"
      >
        <button
          type="button"
          className="wb-ios-row wb-ios-row-press disabled:opacity-50"
          disabled={sync.sending}
          onClick={() => void send()}
          data-testid="wb-ios-sync-send"
        >
          <span
            aria-hidden="true"
            className="grid size-7 shrink-0 place-items-center text-(--wb-ink-2)"
          >
            <Share size={20} />
          </span>
          <span className="min-w-0 flex-1 truncate text-start">
            {t("ios.sync.toMac.send")}
          </span>
          {status?.last_export_at != null && (
            <span className="max-w-[55%] shrink-0 truncate text-(--wb-ink-3) tabular-nums">
              {formatWhen(status.last_export_at, i18n.language)}
            </span>
          )}
        </button>
        <IosSwitchRow
          icon={FileText}
          label={t("ios.sync.toMac.csv")}
          hint={t("ios.sync.toMac.csvHint")}
          checked={withCsv}
          onChange={setWithCsv}
          testId="wb-ios-sync-csv"
        />
      </IosGroup>

      <IosGroup
        label={t("ios.sync.devices.title")}
        testId="wb-ios-sync-devices"
      >
        {status && status.devices.length === 0 && (
          <div className="wb-ios-row text-(--wb-ink-3)">
            {t("ios.sync.devices.none")}
          </div>
        )}
        {status?.devices.map((device) => (
          <div
            key={device.device_id}
            className="wb-ios-row"
            data-testid="wb-ios-sync-device"
          >
            <span
              aria-hidden="true"
              className="grid size-7 shrink-0 place-items-center text-(--wb-ink-2)"
            >
              {device.platform === "ios" ? (
                <Smartphone size={20} />
              ) : (
                <Laptop size={20} />
              )}
            </span>
            <span className="flex min-w-0 flex-1 flex-col gap-0.5 py-2.5">
              <span className="truncate">{deviceLabel(t, device, "row")}</span>
              <span className="text-[13px] leading-[18px] text-(--wb-ink-3) tabular-nums">
                {[
                  device.last_import_at != null &&
                    t("ios.sync.devices.merged", {
                      when: formatWhen(device.last_import_at, i18n.language),
                    }),
                  device.words > 0 &&
                    t("ios.sync.devices.words", {
                      count: device.words,
                      words: formatCount(device.words, i18n.language),
                    }),
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            </span>
          </div>
        ))}
      </IosGroup>
      <SyncReviewDialog sync={sync} />
    </div>
  );
}

/** The Settings row's value: when this iPhone last merged a file. */
export function useLastSynced(): string {
  const { i18n } = useTranslation(FORK_NAMESPACE);
  const [at, setAt] = useState<number | null>(null);
  useEffect(() => {
    let alive = true;
    void commands
      .forkSyncStatus()
      .then((result) => {
        if (!alive || result.status !== "ok" || !result.data) return;
        const times = result.data.devices
          .map((d) => d.last_import_at)
          .filter((v): v is number => v != null);
        setAt(times.length ? Math.max(...times) : null);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);
  if (at == null) return "";
  try {
    return new Intl.DateTimeFormat(i18n.language, {
      dateStyle: "medium",
    }).format(new Date(at));
  } catch {
    return "";
  }
}
