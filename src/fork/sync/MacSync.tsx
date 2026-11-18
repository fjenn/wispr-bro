// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Data and Privacy › Sync (the Mac's side of `fork::sync`).
 *
 * - "Sync with your iPhone": the switch for automatic sync (the Mac becomes
 *   findable on the local network and accepts its paired iPhones), the
 *   paired devices with their state, Unpair. The pairing code itself shows
 *   on `MacPairingGate`, wherever the user is.
 * - "Sync with a file": Export writes this Mac's `.wisprbro` file to
 *   AirDrop to the phone; Import merges the phone's, after the same review
 *   the phone shows.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import { FORK_NAMESPACE } from "../i18n";
import { Button, SettingGroup, SettingRow, toast } from "../ui";
import { Download, Smartphone, Upload } from "../ui/icons";
import { ToggleRow } from "../settings/Controls";
import { UnpairDialog } from "./PairingDialog";
import { deviceLabel, SyncReviewDialog } from "./SyncReviewDialog";
import { deviceLine, notesLine, useAutoSync } from "./useAutoSync";
import { formatWhen, useSync } from "./useSync";

export function MacSync() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const sync = useSync();
  const auto = useAutoSync();
  const [unpairing, setUnpairing] = useState<string | null>(null);
  const unpairTarget = auto.status?.devices.find(
    (d) => d.device_id === unpairing,
  );
  const { status } = sync;
  const busy = sync.sending || sync.state.step === "reading";
  const last = status?.devices.find((d) => d.last_import_at != null);

  const exportFile = async () => {
    try {
      const path = await save({
        defaultPath: t("settingsModal.privacy.sync.fileName", {
          device: (status?.device_name ?? "Mac").replace(
            /[^\p{L}\p{N} _-]/gu,
            "",
          ),
        }),
        filters: [
          {
            name: t("settingsModal.privacy.sync.filter"),
            extensions: ["wisprbro"],
          },
        ],
      });
      if (!path) return;
      const sent = await sync.send(path, false);
      if (sent) toast.success(t("settingsModal.privacy.sync.exported"));
      else toast.error(t("sync.sendFailed"));
    } catch {
      toast.error(t("sync.sendFailed"));
    }
  };

  return (
    <div data-testid="wb-privacy-sync">
      <SettingGroup label={t("settingsModal.privacy.sync.title")}>
        {auto.status && (
          <ToggleRow
            label={t("settingsModal.privacy.sync.autoTitle")}
            hint={t("settingsModal.privacy.sync.autoHint")}
            checked={auto.status.enabled}
            onChange={auto.enable}
            testId="wb-privacy-sync-auto"
          >
            <div className="flex flex-col gap-2">
              {auto.status.network_denied && (
                <p
                  role="alert"
                  className="type-small text-(--wb-danger)"
                  data-testid="wb-privacy-sync-denied"
                >
                  {t("settingsModal.privacy.sync.denied")}
                </p>
              )}
              {auto.status.devices.map((device) => {
                const line = deviceLine(t, device, i18n.language, "mac");
                const notes = notesLine(
                  t,
                  device,
                  deviceLabel(t, device, "row"),
                );
                return (
                  <div
                    key={device.device_id}
                    data-testid="wb-privacy-sync-device"
                    data-state={line.state}
                    className="flex items-center gap-3 rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface) px-3 py-2"
                  >
                    <Smartphone
                      size={18}
                      aria-hidden="true"
                      className="shrink-0 text-(--wb-ink-2)"
                    />
                    <span className="flex min-w-0 flex-1 flex-col">
                      <span className="truncate text-[14px] leading-5 font-medium text-(--wb-ink)">
                        {deviceLabel(t, device, "row")}
                      </span>
                      <span className="flex items-center gap-1.5 text-[13px] leading-[18px] text-(--wb-ink-2)">
                        <span
                          aria-hidden="true"
                          className={`size-1.5 shrink-0 rounded-full ${
                            line.state === "in_sync"
                              ? "bg-(--wb-success)"
                              : "bg-(--wb-ink-disabled)"
                          }`}
                        />
                        {auto.status?.enabled
                          ? line.text
                          : t("sync.auto.lastSynced", {
                              when:
                                device.last_sync_at != null
                                  ? formatWhen(
                                      device.last_sync_at,
                                      i18n.language,
                                    )
                                  : "–",
                            })}
                      </span>
                      {notes && (
                        <span
                          className="text-[13px] leading-[18px] text-(--wb-ink-2)"
                          data-testid="wb-privacy-sync-notes-update"
                        >
                          {notes}
                        </span>
                      )}
                    </span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setUnpairing(device.device_id)}
                      data-testid="wb-privacy-sync-unpair"
                    >
                      {t("sync.auto.unpair")}
                    </Button>
                  </div>
                );
              })}
              {auto.status.enabled && auto.status.devices.length === 0 && (
                <p
                  className="type-small text-(--wb-ink-2)"
                  data-testid="wb-privacy-sync-howto"
                >
                  {t("settingsModal.privacy.sync.howTo")}
                </p>
              )}
              <p className="type-small text-(--wb-ink-3)">
                {t("settingsModal.privacy.sync.direct")}
              </p>
            </div>
          </ToggleRow>
        )}
        <SettingRow
          title={t("settingsModal.privacy.sync.rowTitle")}
          description={t("settingsModal.privacy.sync.hint")}
          control={
            <>
              <Button
                variant="outline"
                size="sm"
                onClick={() => void sync.pick()}
                disabled={busy}
                data-testid="wb-privacy-sync-import"
              >
                <Download size={14} />
                {t("settingsModal.privacy.sync.import")}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => void exportFile()}
                disabled={busy}
                data-testid="wb-privacy-sync-export"
              >
                <Upload size={14} />
                {t("settingsModal.privacy.sync.export")}
              </Button>
            </>
          }
        >
          <p
            className="type-small text-(--wb-ink-2)"
            data-testid="wb-privacy-sync-last"
          >
            {last?.last_import_at != null
              ? t("settingsModal.privacy.sync.last", {
                  device: deviceLabel(t, last, "row"),
                  when: formatWhen(last.last_import_at, i18n.language),
                })
              : t("settingsModal.privacy.sync.never")}
          </p>
        </SettingRow>
      </SettingGroup>
      <SyncReviewDialog sync={sync} />
      <UnpairDialog
        device={unpairTarget ? deviceLabel(t, unpairTarget, "row") : null}
        onClose={() => setUnpairing(null)}
        onConfirm={() => {
          if (unpairing) auto.unpair(unpairing);
          setUnpairing(null);
        }}
      />
    </div>
  );
}
