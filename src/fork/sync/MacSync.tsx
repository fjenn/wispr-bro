// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Data and Privacy › Sync with your iPhone (the Mac's side of
 * `fork::sync`): Export writes this Mac's `.wisprbro` file to AirDrop to
 * the phone; Import merges the phone's, after the same review the phone
 * shows.
 */
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import { FORK_NAMESPACE } from "../i18n";
import { Button, SettingGroup, SettingRow, toast } from "../ui";
import { Download, Upload } from "../ui/icons";
import { deviceLabel, SyncReviewDialog } from "./SyncReviewDialog";
import { formatWhen, useSync } from "./useSync";

export function MacSync() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const sync = useSync();
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
    </div>
  );
}
