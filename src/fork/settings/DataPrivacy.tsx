// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Copy,
  Download,
  FolderOpen,
  Save,
  ShieldCheck,
  Trash2,
} from "../ui/icons";
import { save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import {
  commands,
  type ForkDataLocationKind,
  type ForkDataLocations,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, Checkbox, FOCUS_RING, SettingGroup, SettingRow } from "../ui";
import { PanelCard, SelectRow, ToggleRow } from "./Controls";
import { useForkSettings } from "./useForkSettings";

const LOCATION_KINDS: ForkDataLocationKind[] = [
  "data",
  "models",
  "recordings",
  "logs",
];

/** Typed to confirm delete-all. Not translated: the backend checks the same
 * word (`fork::erase::CONFIRMATION`), and a fixed Latin word is typeable on
 * every keyboard layout. */
export const DELETE_CONFIRMATION_WORD = "DELETE";

function optionalDays(value: string): number | null {
  return value === "forever" ? null : Number(value);
}

export function DataPrivacy() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const [locations, setLocations] = useState<ForkDataLocations | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteModels, setDeleteModels] = useState(false);
  const [confirmation, setConfirmation] = useState("");
  const deleteDialog = useRef<HTMLDialogElement>(null);
  const confirmationInput = useRef<HTMLInputElement>(null);
  const confirmationWord = DELETE_CONFIRMATION_WORD;

  useEffect(() => {
    let live = true;
    void commands.forkDataLocations().then((result) => {
      if (!live) return;
      if (result.status === "ok") setLocations(result.data);
      else toast.error(t("settingsModal.privacy.locations.failed"));
    });
    return () => {
      live = false;
    };
  }, [t]);

  useEffect(() => {
    const dialog = deleteDialog.current;
    if (deleteOpen && dialog && !dialog.open) {
      dialog.showModal();
      // showModal() focuses the first control (the models box); the typed
      // confirmation is the field the user needs first.
      confirmationInput.current?.focus();
    }
    if (!deleteOpen && dialog?.open) dialog.close();
  }, [deleteOpen]);

  const showLocation = async (kind: ForkDataLocationKind) => {
    const result = await commands.forkShowDataLocation(kind);
    if (result.status === "error") {
      toast.error(t("settingsModal.privacy.locations.failed"));
    }
  };

  const exportData = async () => {
    setBusy("export");
    try {
      const date = new Date().toISOString().slice(0, 10);
      let path = await save({
        defaultPath: t("settingsModal.privacy.export.fileName", { date }),
        filters: [
          {
            name: t("settingsModal.privacy.export.filter"),
            extensions: ["zip"],
          },
        ],
      });
      if (!path) return;
      if (!/\.zip$/i.test(path)) path = `${path}.zip`;
      const result = await commands.forkExportData(path);
      if (result.status === "ok") {
        toast.success(t("settingsModal.privacy.export.done"));
      } else {
        toast.error(t("settingsModal.privacy.export.failed"));
      }
    } catch {
      toast.error(t("settingsModal.privacy.export.failed"));
    } finally {
      setBusy(null);
    }
  };

  const copyDiagnostics = async () => {
    setBusy("copy");
    try {
      const result = await commands.forkCopyDiagnostics();
      if (result.status === "ok") {
        toast.success(t("settingsModal.privacy.diagnostics.copied"));
      } else {
        toast.error(t("settingsModal.privacy.diagnostics.failed"));
      }
    } catch {
      toast.error(t("settingsModal.privacy.diagnostics.failed"));
    } finally {
      setBusy(null);
    }
  };

  const saveDiagnostics = async () => {
    setBusy("diagnostics");
    try {
      const date = new Date().toISOString().slice(0, 10);
      let path = await save({
        defaultPath: t("settingsModal.privacy.diagnostics.fileName", { date }),
        filters: [
          {
            name: t("settingsModal.privacy.export.filter"),
            extensions: ["zip"],
          },
        ],
      });
      if (!path) return;
      if (!/\.zip$/i.test(path)) path = `${path}.zip`;
      const result = await commands.forkSaveDiagnostics(path);
      if (result.status === "ok") {
        toast.success(t("settingsModal.privacy.diagnostics.saved"));
      } else {
        toast.error(t("settingsModal.privacy.diagnostics.failed"));
      }
    } catch {
      toast.error(t("settingsModal.privacy.diagnostics.failed"));
    } finally {
      setBusy(null);
    }
  };

  const erase = async () => {
    if (confirmation !== confirmationWord) return;
    setBusy("erase");
    try {
      // Webview storage (theme, sidebar, onboarding progress) is user data
      // too; the backend also clears WebKit's store before relaunching.
      try {
        localStorage.clear();
        sessionStorage.clear();
      } catch {
        // storage unavailable: nothing stored there
      }
      const result = await commands.forkEraseAllData(
        confirmation,
        deleteModels,
      );
      if (result.status === "error") {
        toast.error(t("settingsModal.privacy.delete.failed"));
        setBusy(null);
      }
    } catch {
      toast.error(t("settingsModal.privacy.delete.failed"));
      setBusy(null);
    }
  };

  if (!settings) return null;

  return (
    <div className="flex flex-col gap-8" data-testid="wb-settings-privacy">
      <div data-testid="wb-privacy-promise">
        <PanelCard>
          <div className="flex gap-4">
            <ShieldCheck
              size={20}
              aria-hidden="true"
              className="mt-0.5 shrink-0 text-(--wb-accent)"
            />
            <div className="flex flex-col gap-2 text-[14px] leading-5">
              <h3 className="type-card-title">
                {t("settingsModal.privacy.promise.local")}
              </h3>
              <p className="text-(--wb-ink-2)">
                {t("settingsModal.privacy.promise.noCloud")}
              </p>
              <p className="text-(--wb-ink-2)">
                {t("settingsModal.privacy.promise.networkIntro")}
              </p>
              <ul className="list-disc space-y-1 ps-5 text-(--wb-ink-2)">
                <li>{t("settingsModal.privacy.promise.models")}</li>
                <li>{t("settingsModal.privacy.promise.updates")}</li>
              </ul>
            </div>
          </div>
        </PanelCard>
      </div>

      <div data-testid="wb-privacy-locations">
        <SettingGroup label={t("settingsModal.privacy.locations.title")}>
          <p className="type-small -mt-1 text-(--wb-ink-2)">
            {t("settingsModal.privacy.locations.hint")}
          </p>
          {LOCATION_KINDS.map((kind) => (
            <SettingRow
              key={kind}
              title={t(`settingsModal.privacy.locations.${kind}`)}
              description={
                <span
                  className="font-(family-name:--wb-font-mono) text-[12px] break-all"
                  dir="ltr"
                  data-testid={`wb-privacy-path-${kind}`}
                >
                  {locations?.[kind] ??
                    t("settingsModal.privacy.locations.loading")}
                </span>
              }
              control={
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void showLocation(kind)}
                  data-testid={`wb-privacy-show-${kind}`}
                >
                  <FolderOpen size={14} />
                  {t("settingsModal.privacy.locations.show")}
                </Button>
              }
            />
          ))}
        </SettingGroup>
      </div>

      <div data-testid="wb-privacy-retention">
        <SettingGroup label={t("settingsModal.privacy.retention.title")}>
          <SelectRow
            label={t("settingsModal.privacy.retention.textLabel")}
            hint={t("settingsModal.privacy.retention.textHint")}
            value={
              settings.text_retention_days === null
                ? "forever"
                : String(settings.text_retention_days)
            }
            disabled={!!pending.text_retention_days}
            options={["forever", "365", "90", "30"].map((value) => ({
              value,
              label: t(`settingsModal.privacy.retention.options.${value}`),
            }))}
            onChange={(value) =>
              void setSetting("text_retention_days", optionalDays(value))
            }
            testId="wb-privacy-text-retention"
            width="w-44"
          />
          <SelectRow
            label={t("settingsModal.privacy.retention.audioLabel")}
            hint={t("settingsModal.privacy.retention.audioHint")}
            value={
              settings.audio_retention_days === null
                ? "forever"
                : String(settings.audio_retention_days)
            }
            disabled={!!pending.audio_retention_days}
            options={["0", "1", "7", "14", "30", "90", "forever"].map(
              (value) => ({
                value,
                label: t(`settingsModal.privacy.retention.options.${value}`),
              }),
            )}
            onChange={(value) =>
              void setSetting("audio_retention_days", optionalDays(value))
            }
            testId="wb-privacy-audio-retention"
            width="w-44"
          />
        </SettingGroup>
      </div>

      <div data-testid="wb-privacy-choices">
        <SettingGroup label={t("settingsModal.privacy.choices.title")}>
          <ToggleRow
            label={t("settingsModal.privacy.choices.context")}
            hint={t("settingsModal.privacy.choices.contextHint")}
            checked={settings.context_awareness}
            disabled={!!pending.context_awareness}
            onChange={(checked) =>
              void setSetting("context_awareness", checked)
            }
            testId="wb-privacy-context"
          />
          <ToggleRow
            label={t("settingsModal.privacy.choices.learn")}
            hint={t("settingsModal.privacy.choices.learnHint")}
            checked={settings.auto_learn_enabled}
            disabled={!!pending.auto_learn_enabled}
            onChange={(checked) =>
              void setSetting("auto_learn_enabled", checked)
            }
            testId="wb-privacy-learn"
          />
          <ToggleRow
            label={t("settingsModal.privacy.choices.updates")}
            hint={t("settingsModal.privacy.choices.updatesHint")}
            checked={settings.update_checks_enabled}
            disabled={!!pending.update_checks_enabled}
            onChange={(checked) =>
              void setSetting("update_checks_enabled", checked)
            }
            testId="wb-privacy-updates"
          />
        </SettingGroup>
      </div>

      <div data-testid="wb-privacy-export">
        <SettingGroup label={t("settingsModal.privacy.export.title")}>
          <SettingRow
            title={t("settingsModal.privacy.export.button")}
            description={t("settingsModal.privacy.export.hint")}
            control={
              <Button
                variant="outline"
                size="sm"
                onClick={() => void exportData()}
                disabled={busy !== null}
                aria-busy={busy === "export"}
                data-testid="wb-privacy-export-button"
              >
                <Download size={14} />
                {t("settingsModal.privacy.export.button")}
              </Button>
            }
          />
          <SettingRow
            title={t("settingsModal.privacy.delete.button")}
            description={t("settingsModal.privacy.delete.short")}
            control={
              <Button
                variant="destructive"
                size="sm"
                onClick={() => {
                  setConfirmation("");
                  setDeleteModels(false);
                  setDeleteOpen(true);
                }}
                disabled={busy !== null}
                data-testid="wb-privacy-delete-button"
              >
                <Trash2 size={14} />
                {t("settingsModal.privacy.delete.button")}
              </Button>
            }
          />
        </SettingGroup>
      </div>

      <div data-testid="wb-privacy-diagnostics">
        <SettingGroup label={t("settingsModal.privacy.diagnostics.title")}>
          <SettingRow
            title={t("settingsModal.privacy.diagnostics.rowTitle")}
            description={t("settingsModal.privacy.diagnostics.hint")}
            control={
              <>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void copyDiagnostics()}
                  disabled={busy !== null}
                  data-testid="wb-privacy-copy-diagnostics"
                >
                  <Copy size={14} />
                  {t("settingsModal.privacy.diagnostics.copy")}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void saveDiagnostics()}
                  disabled={busy !== null}
                  data-testid="wb-privacy-save-diagnostics"
                >
                  <Save size={14} />
                  {t("settingsModal.privacy.diagnostics.save")}
                </Button>
              </>
            }
          >
            <p className="type-small text-(--wb-ink-2)">
              {t("settingsModal.privacy.diagnostics.sanitized")}
            </p>
          </SettingRow>
        </SettingGroup>
      </div>

      <dialog
        ref={deleteDialog}
        aria-labelledby="wb-delete-title"
        className="wb-settings-dialog m-auto w-[min(480px,calc(100vw-32px))] max-w-none rounded-(--wb-radius-lg) border-0 bg-(--wb-surface) p-6 text-(--wb-ink) shadow-(--wb-elev-3)"
        data-testid="wb-delete-dialog"
        onCancel={(event) => {
          // React propagates cancel/close through the component tree: keep
          // Escape from also closing the Settings modal around this dialog.
          event.stopPropagation();
          event.preventDefault();
          if (busy !== "erase") setDeleteOpen(false);
        }}
        onClose={(event) => {
          event.stopPropagation();
          setDeleteOpen(false);
        }}
      >
        <h2 id="wb-delete-title" className="type-heading">
          {t("settingsModal.privacy.delete.title")}
        </h2>
        <p className="mt-2 text-[14px] leading-5 text-(--wb-ink-2)">
          {t("settingsModal.privacy.delete.warning")}
        </p>
        <p className="mt-2 text-[14px] leading-5 text-(--wb-ink-2)">
          {t("settingsModal.privacy.delete.relaunch")}
        </p>
        <label className="mt-5 flex cursor-pointer items-start gap-3 rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-4 py-3 text-[14px]">
          <Checkbox
            className="mt-0.5"
            checked={deleteModels}
            disabled={busy === "erase"}
            onChange={(event) => setDeleteModels(event.currentTarget.checked)}
            data-testid="wb-delete-models"
          />
          <span>
            <span className="block font-medium">
              {t("settingsModal.privacy.delete.models")}
            </span>
            <span className="mt-0.5 block text-[13px] text-(--wb-ink-2)">
              {t("settingsModal.privacy.delete.modelsHint")}
            </span>
          </span>
        </label>
        <label
          htmlFor="wb-delete-confirmation"
          className="mt-5 block text-[14px] font-medium"
        >
          {t("settingsModal.privacy.delete.confirmLabel", {
            word: confirmationWord,
          })}
        </label>
        <input
          id="wb-delete-confirmation"
          ref={confirmationInput}
          value={confirmation}
          disabled={busy === "erase"}
          onChange={(event) => setConfirmation(event.currentTarget.value)}
          autoComplete="off"
          spellCheck={false}
          className={`mt-2 h-10 w-full rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) px-3 text-[15px] ${FOCUS_RING}`}
          data-testid="wb-delete-confirmation"
        />
        <div className="mt-6 flex justify-end gap-2">
          <Button
            variant="secondary"
            onClick={() => setDeleteOpen(false)}
            disabled={busy === "erase"}
            data-testid="wb-delete-cancel"
          >
            {t("settingsModal.privacy.delete.cancel")}
          </Button>
          <Button
            onClick={() => void erase()}
            disabled={confirmation !== confirmationWord || busy === "erase"}
            aria-busy={busy === "erase"}
            variant="destructive"
            data-testid="wb-delete-confirm"
          >
            {t("settingsModal.privacy.delete.confirm")}
          </Button>
        </div>
      </dialog>
    </div>
  );
}
