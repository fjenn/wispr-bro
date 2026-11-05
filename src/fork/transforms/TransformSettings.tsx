// SPDX-License-Identifier: GPL-3.0-or-later
// The Transforms page's secondary controls (XF2; flowmatch): Command Mode
// and the ⌥-key characters, behind the settings button beside "My
// Transforms". Auto-apply and Reset are on the page itself.
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { useForkSettings } from "../settings/useForkSettings";
import {
  Button,
  Dialog,
  Kbd,
  KbdCombo,
  SettingGroup,
  SettingRow,
  Switch,
} from "../ui";

export function TransformSettings({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
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
    >
      <SettingGroup>
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
