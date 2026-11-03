// SPDX-License-Identifier: GPL-3.0-or-later
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { AutostartToggle } from "@/components/settings/AutostartToggle";
import { OutputDeviceSelector } from "@/components/settings/OutputDeviceSelector";
import { ShowTrayIcon } from "@/components/settings/ShowTrayIcon";
import { StartHidden } from "@/components/settings/StartHidden";
import { useSettings } from "@/hooks/useSettings";
import type { NotificationSettings, ScratchpadOpenBehavior } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { SettingGroup, Switch } from "../ui";
import { HandyRow, SelectRow, ToggleRow } from "./Controls";
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
    <div className="flex flex-col gap-8" data-testid="wb-settings-system">
      <SettingGroup label={t("settingsModal.system.app")}>
        <HandyRow>
          <AutostartToggle descriptionMode="inline" grouped />
        </HandyRow>
        <ToggleRow
          label={t("settingsModal.system.alwaysBar")}
          hint={t("settings.bar.alwaysShowHelp")}
          checked={settings.bar_always_visible}
          disabled={!!pending.bar_always_visible}
          onChange={(checked) => void setSetting("bar_always_visible", checked)}
          testId="wb-system-always-bar"
        />
        <ToggleRow
          label={t("settingsModal.system.showDock")}
          hint={t("settingsModal.system.showDockHint")}
          checked={settings.show_in_dock}
          disabled={!!pending.show_in_dock}
          onChange={(checked) => void setSetting("show_in_dock", checked)}
          testId="wb-system-show-dock"
        />
        <HandyRow>
          <ShowTrayIcon descriptionMode="inline" grouped />
        </HandyRow>
        <HandyRow>
          <StartHidden descriptionMode="inline" grouped />
        </HandyRow>
      </SettingGroup>
      <div data-testid="wb-system-sounds">
        <SettingGroup label={t("settingsModal.system.sounds")}>
          <SoundSettings />
          <HandyRow>
            <OutputDeviceSelector
              descriptionMode="inline"
              grouped
              disabled={!audioFeedbackEnabled}
            />
          </HandyRow>
        </SettingGroup>
      </div>
      <div data-testid="wb-system-notifications">
        <SettingGroup label={t("settingsModal.system.notifications")}>
          <ul className="divide-y divide-(--wb-line) rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint)">
            {NOTIFICATIONS.map(({ key, label }) => (
              <NotificationLine
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
          </ul>
        </SettingGroup>
      </div>
      <div data-testid="wb-system-scratchpad">
        <SettingGroup label={t("settingsModal.system.scratchpad")}>
          <SelectRow
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
          <ToggleRow
            label={t("scratchpad.addToBar")}
            hint={t("scratchpad.addToBarHelp")}
            checked={settings.scratchpad_on_bar}
            disabled={!!pending.scratchpad_on_bar}
            onChange={(checked) =>
              void setSetting("scratchpad_on_bar", checked)
            }
            testId="wb-system-scratchpad-on-bar"
          />
        </SettingGroup>
      </div>
    </div>
  );
}

/** One kind of notification: a compact line in the grouped card. */
function NotificationLine({
  label,
  checked,
  disabled,
  onChange,
  testId,
}: {
  label: string;
  checked: boolean;
  disabled: boolean;
  onChange: (checked: boolean) => void;
  testId: string;
}) {
  const id = useId();
  return (
    <li className="flex min-h-12 items-center justify-between gap-4 px-5 py-2">
      <label
        htmlFor={id}
        className="min-w-0 flex-1 cursor-pointer text-[15px] leading-5"
      >
        {label}
      </label>
      <Switch
        id={id}
        checked={checked}
        disabled={disabled}
        onChange={onChange}
        testId={testId}
      />
    </li>
  );
}
