// SPDX-License-Identifier: GPL-3.0-or-later
// The Transforms page's secondary controls (XF2): auto-apply, Command Mode,
// the ⌥-key characters and Reset, behind the page's settings button.
import { useTranslation } from "react-i18next";
import { RotateCcw } from "../ui/icons";
import type { TransformEntry } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { useForkSettings } from "../settings/useForkSettings";
import {
  Button,
  Dialog,
  Kbd,
  KbdCombo,
  Select,
  SettingGroup,
  SettingRow,
  Switch,
} from "../ui";

export function TransformSettings({
  open,
  onOpenChange,
  entries,
  onReset,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  entries: TransformEntry[];
  onReset: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const enabled = settings?.transforms_enabled ?? false;
  const commandOn = settings?.command_mode_enabled ?? false;
  const commandChord = settings
    ? settings.hotkeys["command_mode"]?.[0]?.chord
    : "fn+ctrl";
  const safeShortcuts = async () => {
    for (const [action, chord] of [
      ["transform.polish", "ctrl+option+1"],
      ["transform.prompt_engineer", "ctrl+option+2"],
      ["transform.view_changes", "ctrl+option+o"],
    ] as const) {
      const result = await setSetting("hotkey", {
        action,
        bindings: [{ chord, activation: "press" }],
      });
      if (result.status === "error") break;
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("transforms.settings.title")}
      closeLabel={t("debug.close")}
      testId="wb-transforms-settings-dialog"
      footer={
        <Button
          variant="ghost"
          onClick={onReset}
          data-testid="wb-transforms-reset"
        >
          <RotateCcw size={16} aria-hidden="true" />
          {t("transforms.reset")}
        </Button>
      }
    >
      <SettingGroup>
        <SettingRow
          title={t("transforms.auto.title")}
          testId="wb-transforms-auto-row"
          control={(ids) => (
            <>
              <Select
                value={settings?.auto_apply_transform ?? undefined}
                placeholder={t("transforms.auto.none")}
                onValueChange={(value) =>
                  void setSetting("auto_apply_transform", value || null)
                }
                disabled={
                  !enabled ||
                  !settings?.auto_apply_transform ||
                  !!pending.auto_apply_transform
                }
                label={t("transforms.auto.choice")}
                testId="wb-transforms-auto-choice"
                className="w-44"
                options={entries.map((entry) => ({
                  value: entry.key,
                  label: entry.name,
                }))}
              />
              <Switch
                checked={settings?.auto_apply_transform != null}
                disabled={!enabled || !!pending.auto_apply_transform}
                onChange={(checked) =>
                  void setSetting(
                    "auto_apply_transform",
                    checked ? (entries[0]?.key ?? null) : null,
                  )
                }
                ariaLabelledBy={ids.labelledBy}
                testId="wb-transforms-auto"
              />
            </>
          )}
        />
        <SettingRow
          title={
            <span className="flex flex-wrap items-center gap-2">
              {t("transforms.commandMode.title")}
              {commandChord ? (
                <KbdCombo keys={chordLabel(commandChord).split(" ")} />
              ) : (
                <Kbd>{t("settingsModal.hotkeys.notSet")}</Kbd>
              )}
            </span>
          }
          description={t("transforms.commandMode.help")}
          testId="wb-command-mode-card"
          control={(ids) => (
            <Switch
              checked={commandOn}
              disabled={!settings || !!pending.command_mode_enabled}
              onChange={(checked) =>
                void setSetting("command_mode_enabled", checked)
              }
              ariaLabelledBy={ids.labelledBy}
              ariaDescribedBy={ids.describedBy}
              testId="wb-command-mode-enabled"
            />
          )}
        />
        <SettingRow
          title={t("transforms.shortcuts.warning")}
          control={
            <Button
              size="sm"
              variant="outline"
              disabled={!settings || !!pending.hotkey}
              onClick={() => void safeShortcuts()}
              data-testid="wb-transforms-safe-shortcuts"
            >
              {t("transforms.shortcuts.safe")}
            </Button>
          }
        />
      </SettingGroup>
    </Dialog>
  );
}
