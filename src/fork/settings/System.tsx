// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import { AutostartToggle } from "@/components/settings/AutostartToggle";
import { OutputDeviceSelector } from "@/components/settings/OutputDeviceSelector";
import { ShowTrayIcon } from "@/components/settings/ShowTrayIcon";
import { StartHidden } from "@/components/settings/StartHidden";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { useSettings } from "@/hooks/useSettings";
import type { NotificationSettings, ScratchpadOpenBehavior } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { ForkSelect, ForkToggle, Panel } from "./Controls";
import { SoundSettings } from "./SoundSettings";
import { useForkSettings } from "./useForkSettings";

const NOTIFICATIONS: {
  key: keyof NotificationSettings;
  label: string;
}[] = [
  { key: "system", label: "system" },
  { key: "model_ready", label: "modelReady" },
  { key: "update_ready", label: "updateReady" },
  { key: "queued_takes_ready", label: "queuedReady" },
  { key: "errors", label: "errors" },
  { key: "suggestions", label: "suggestions" },
  { key: "announcements", label: "announcements" },
  { key: "milestones", label: "milestones" },
];

export function System() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const { audioFeedbackEnabled } = useSettings();
  if (!settings) return null;

  const notifications = settings.notifications;
  return (
    <div className="flex flex-col gap-6" data-testid="wb-settings-system">
      <SettingsGroup title={t("settingsModal.system.app")}>
        <div data-handy-content>
          <AutostartToggle descriptionMode="tooltip" grouped />
        </div>
        <div className="px-4">
          <ForkToggle
            label={t("settingsModal.system.alwaysBar")}
            hint={t("settings.bar.alwaysShowHelp")}
            checked={settings.bar_always_visible}
            disabled={!!pending.bar_always_visible}
            onChange={(checked) =>
              void setSetting("bar_always_visible", checked)
            }
            testId="wb-system-always-bar"
          />
          <ForkToggle
            label={t("settingsModal.system.showDock")}
            hint={t("settingsModal.system.showDockHint")}
            checked={settings.show_in_dock}
            disabled={!!pending.show_in_dock}
            onChange={(checked) => void setSetting("show_in_dock", checked)}
            testId="wb-system-show-dock"
          />
        </div>
        <div data-handy-content>
          <ShowTrayIcon descriptionMode="tooltip" grouped />
          <StartHidden descriptionMode="tooltip" grouped />
        </div>
      </SettingsGroup>
      <Panel title={t("settingsModal.system.sounds")} testId="wb-system-sounds">
        <SoundSettings />
        <div className="mt-3" data-handy-content>
          <OutputDeviceSelector
            descriptionMode="tooltip"
            disabled={!audioFeedbackEnabled}
          />
        </div>
      </Panel>
      <Panel
        title={t("settingsModal.system.notifications")}
        testId="wb-system-notifications"
      >
        <div className="grid gap-x-5 sm:grid-cols-2">
          {NOTIFICATIONS.map(({ key, label }) => (
            <ForkToggle
              key={key}
              label={t(`settingsModal.system.notificationKinds.${label}`)}
              checked={notifications[key]}
              disabled={!!pending.notifications}
              onChange={(checked) =>
                void setSetting("notifications", {
                  ...notifications,
                  [key]: checked,
                })
              }
              testId={`wb-notification-${key}`}
            />
          ))}
        </div>
      </Panel>
      <Panel
        title={t("settingsModal.system.scratchpad")}
        testId="wb-system-scratchpad"
      >
        <ForkSelect
          label={t("settingsModal.system.scratchpadLabel")}
          hint={t("settingsModal.system.scratchpadHint")}
          value={settings.scratchpad_open_behavior}
          disabled={!!pending.scratchpad_open_behavior}
          options={["resume_last", "new_note"].map((value) => ({
            value,
            label: t(`settingsModal.system.scratchpadOptions.${value}`),
          }))}
          onChange={(value) =>
            void setSetting(
              "scratchpad_open_behavior",
              value as ScratchpadOpenBehavior,
            )
          }
          testId="wb-scratchpad-open-behavior"
        />
        <ForkToggle
          label={t("scratchpad.addToBar")}
          hint={t("scratchpad.addToBarHelp")}
          checked={settings.scratchpad_on_bar}
          disabled={!!pending.scratchpad_on_bar}
          onChange={(checked) => void setSetting("scratchpad_on_bar", checked)}
          testId="wb-system-scratchpad-on-bar"
        />
      </Panel>
    </div>
  );
}
