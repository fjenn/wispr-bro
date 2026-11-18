// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Sync with your Mac. No account and no server.
 *
 *   Sync with your Mac
 *   ┌ Pair your Mac (until paired) ─────────────────┐
 *   │ 1 On your Mac: turn on Sync with your iPhone  │
 *   │ 2 Find my Mac  3 Type the code                │
 *   │ [ Find my Mac ]  → Macs nearby → the code     │
 *   └────────────────────────────────────────────────┘
 *   ┌ Your Mac (once paired) ───────────────────────┐
 *   │ Finns MacBook Pro   ● In sync · 2 minutes ago │
 *   │ Sync now                                      │
 *   │ Unpair                                        │
 *   └────────────────────────────────────────────────┘
 *   ┌ Sync with a file ⌄ ───────────────────────────┐  the manual route:
 *   │ From your Mac · To your Mac · Synced          │  AirDrop a file
 *   └────────────────────────────────────────────────┘
 *
 * Paired, the two sync by themselves while this app runs (open, or in a
 * dictation session) and the Mac's app runs on the same local network
 * (`src-tauri/src/fork/sync/auto.rs`). A file opened in Wispr Bro from
 * AirDrop or Files lands here by itself (`useSyncInbox` in the app shell)
 * with its review open.
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, events } from "../bindings";
import { formatCount } from "../home/StatsRail";
import { navigate } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import { PairingDialog, UnpairDialog } from "../sync/PairingDialog";
import { deviceLabel, SyncReviewDialog } from "../sync/SyncReviewDialog";
import {
  deviceLine,
  notesLine,
  useAutoSync,
  type AutoSync,
} from "../sync/useAutoSync";
import { formatWhen, useSync, type Sync } from "../sync/useSync";
import { Button, toast } from "../ui";
import {
  ChevronDown,
  FileText,
  Laptop,
  Loader2,
  RefreshCw,
  Share,
  Smartphone,
  Unlink,
} from "../ui/icons";
import { iosCommands } from "./commands";
import { IosGroup, IosNavRow, IosSwitchRow, IosTitle } from "./parts";

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
  const { t } = useTranslation(FORK_NAMESPACE);
  const sync = useSync();
  const auto = useAutoSync();
  const paired = (auto.status?.devices.length ?? 0) > 0;
  const [fileOpen, setFileOpen] = useState(false);

  return (
    <div className="flex flex-col gap-6" data-testid="wb-ios-sync">
      <IosTitle>{t("ios.settings.pages.sync")}</IosTitle>
      <p className="text-[15px] leading-[21px] text-(--wb-ink-2)">
        {t("ios.sync.intro")}
      </p>

      {auto.status &&
        (paired ? <PairedMac auto={auto} /> : <PairMac auto={auto} />)}

      <section className="flex flex-col gap-6">
        <div className="wb-ios-group">
          <button
            type="button"
            aria-expanded={fileOpen}
            onClick={() => setFileOpen((open) => !open)}
            data-testid="wb-ios-sync-file"
            className="wb-ios-row wb-ios-row-press"
          >
            <span
              aria-hidden="true"
              className="grid size-7 shrink-0 place-items-center text-(--wb-ink-2)"
            >
              <FileText size={20} />
            </span>
            <span className="flex min-w-0 flex-1 flex-col gap-0.5 py-2.5 text-start">
              <span>{t("ios.sync.file.title")}</span>
              <span className="text-[13px] leading-[18px] text-(--wb-ink-3)">
                {t("ios.sync.file.hint")}
              </span>
            </span>
            <ChevronDown
              size={18}
              aria-hidden="true"
              className={`-me-1 shrink-0 text-(--wb-ink-disabled) transition-transform ${fileOpen ? "rotate-180" : ""}`}
            />
          </button>
        </div>
        {fileOpen && <FileSync sync={sync} />}
      </section>
      <SyncReviewDialog sync={sync} />
      <PairingDialog auto={auto} />
    </div>
  );
}

/** Not paired yet: three steps, Find my Mac, the Macs found. */
function PairMac({ auto }: { auto: AutoSync }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const status = auto.status;
  const searching = !!status?.searching;
  const found = status?.found ?? [];
  return (
    <>
      <IosGroup
        label={t("ios.sync.pair.title")}
        footer={t("ios.sync.pair.footer")}
        testId="wb-ios-sync-pair"
      >
        <Steps prefix="ios.sync.pair" />
        <div className="p-4">
          <Button
            variant={searching ? "secondary" : "primary"}
            size="lg"
            block
            onClick={() => auto.search(!searching)}
            data-testid="wb-ios-sync-find"
          >
            {t(searching ? "ios.sync.pair.stop" : "ios.sync.pair.find")}
          </Button>
        </div>
      </IosGroup>
      {status?.network_denied && <Denied />}
      {searching && !status?.network_denied && (
        <IosGroup label={t("ios.sync.pair.nearby")} testId="wb-ios-sync-found">
          {found.map((mac) => (
            <IosNavRow
              key={mac.service}
              icon={Laptop}
              label={mac.name}
              onClick={() => auto.pair(mac.service)}
              testId="wb-ios-sync-found-mac"
            />
          ))}
          <div
            role="status"
            className="wb-ios-row gap-3 text-(--wb-ink-3)"
            data-testid="wb-ios-sync-looking"
          >
            <span
              aria-hidden="true"
              className="grid size-7 shrink-0 place-items-center"
            >
              <Loader2 size={18} className="animate-spin" />
            </span>
            {t("ios.sync.pair.looking")}
          </div>
        </IosGroup>
      )}
    </>
  );
}

/** Local network access was refused: say so, with the way to allow it. */
function Denied() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div
      role="alert"
      data-testid="wb-ios-sync-denied"
      className="rounded-(--wb-radius-lg) bg-(--wb-accent-soft) p-4"
    >
      <p className="text-[15px] leading-[21px] text-(--wb-ink)">
        {t("sync.auto.denied")}
      </p>
      <button
        type="button"
        className="wb-ios-link -ms-2 mt-1"
        onClick={() => void iosCommands.openSettings().catch(() => undefined)}
      >
        {t("sync.auto.openSettings")}
      </button>
    </div>
  );
}

/** Paired: the Mac, how it stands, Sync now, Unpair. */
function PairedMac({ auto }: { auto: AutoSync }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const [unpairing, setUnpairing] = useState<string | null>(null);
  const status = auto.status;
  if (!status) return null;
  const target = status.devices.find((d) => d.device_id === unpairing);
  return (
    <>
      {status.devices.map((device) => {
        const line = deviceLine(t, device, i18n.language, "phone");
        const notes = notesLine(t, device, deviceLabel(t, device, "row"));
        return (
          <IosGroup
            key={device.device_id}
            label={t("ios.sync.mac.title")}
            footer={t("ios.sync.mac.footer")}
            testId="wb-ios-sync-mac"
          >
            <div className="wb-ios-row" data-state={line.state}>
              <span
                aria-hidden="true"
                className="grid size-7 shrink-0 place-items-center text-(--wb-ink-2)"
              >
                <Laptop size={20} />
              </span>
              <span className="flex min-w-0 flex-1 flex-col gap-0.5 py-2.5">
                <span className="truncate font-medium">
                  {deviceLabel(t, device, "row")}
                </span>
                <span
                  role="status"
                  data-testid="wb-ios-sync-state"
                  className="flex items-start gap-2 text-[13px] leading-[18px] text-(--wb-ink-2)"
                >
                  <span
                    aria-hidden="true"
                    className={`mt-[5px] size-2 shrink-0 rounded-full ${
                      line.state === "in_sync"
                        ? "bg-(--wb-success)"
                        : line.state === "connecting"
                          ? "bg-(--wb-accent)"
                          : "bg-(--wb-ink-disabled)"
                    }`}
                  />
                  {line.text}
                </span>
                {notes && (
                  <span
                    data-testid="wb-ios-sync-notes-update"
                    className="text-[13px] leading-[18px] text-(--wb-ink-2)"
                  >
                    {notes}
                  </span>
                )}
              </span>
            </div>
            <button
              type="button"
              className="wb-ios-row wb-ios-row-press"
              onClick={auto.syncNow}
              data-testid="wb-ios-sync-now"
            >
              <span
                aria-hidden="true"
                className="grid size-7 shrink-0 place-items-center text-(--wb-ink-2)"
              >
                <RefreshCw size={20} />
              </span>
              <span className="flex-1 text-start">
                {t("sync.auto.syncNow")}
              </span>
            </button>
            <button
              type="button"
              className="wb-ios-row wb-ios-row-press text-(--wb-danger)"
              onClick={() => setUnpairing(device.device_id)}
              data-testid="wb-ios-sync-unpair"
            >
              <span
                aria-hidden="true"
                className="grid size-7 shrink-0 place-items-center"
              >
                <Unlink size={20} />
              </span>
              <span className="flex-1 text-start">{t("sync.auto.unpair")}</span>
            </button>
          </IosGroup>
        );
      })}
      {status.network_denied && <Denied />}
      <UnpairDialog
        device={target ? deviceLabel(t, target, "row") : null}
        onClose={() => setUnpairing(null)}
        onConfirm={() => {
          if (unpairing) auto.unpair(unpairing);
          setUnpairing(null);
        }}
      />
    </>
  );
}

function Steps({ prefix }: { prefix: string }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
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
            {t(`${prefix}.step${step}`)}
          </span>
        </li>
      ))}
    </ol>
  );
}

/** The file route: from the Mac, to the Mac, what was merged. */
function FileSync({ sync }: { sync: Sync }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { status } = sync;
  const [withCsv, setWithCsv] = useState(false);
  const reading = sync.state.step === "reading";

  const send = async () => {
    const sent = await sync.send(null, withCsv);
    if (!sent) toast.error(t("sync.sendFailed"));
  };

  return (
    <>
      <IosGroup
        label={t("ios.sync.fromMac.title")}
        footer={t("ios.sync.fromMac.footer")}
        testId="wb-ios-sync-from"
      >
        <Steps prefix="ios.sync.fromMac" />
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
    </>
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
