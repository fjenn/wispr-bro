// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Flow Bar settings (S-03): "Always show the bar", position, "Hide the bar
 * in screen recordings", a way back from "Hide for an hour", and forgetting
 * dragged positions. S-14 mounts the controls in Settings › Advanced;
 * System also exposes "Always show the bar" through the same fork setting.
 * Design W3: row cards with switches and a Radix select.
 */
import { useTranslation } from "react-i18next";
import type { BarPosition } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, Select, SettingRow } from "../ui";
import { ToggleRow } from "./Controls";
import { useForkSettings } from "./useForkSettings";

const POSITIONS: BarPosition[] = ["bottom", "top"];

export function BarSettings() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();

  if (!settings) return null;

  const hiddenUntil = settings.bar_hidden_until_ms;
  const hidden = hiddenUntil !== null && hiddenUntil > Date.now();
  const positions = Object.keys(settings.bar_x_by_display).length;

  return (
    <div className="flex flex-col gap-3" data-testid="wb-bar-settings">
      <ToggleRow
        label={t("settings.bar.alwaysShow")}
        hint={t("settings.bar.alwaysShowHelp")}
        checked={settings.bar_always_visible}
        disabled={!!pending.bar_always_visible}
        onChange={(checked) => void setSetting("bar_always_visible", checked)}
        testId="wb-bar-always"
      >
        {hidden && (
          <div
            className="flex flex-wrap items-center justify-between gap-2 border-t border-(--wb-line) pt-3"
            data-testid="wb-bar-hidden"
          >
            <span className="text-[14px] text-(--wb-ink-2)">
              {t("settings.bar.hiddenUntil", {
                time: new Date(hiddenUntil).toLocaleTimeString(i18n.language, {
                  hour: "numeric",
                  minute: "2-digit",
                }),
              })}
            </span>
            <Button
              variant="outline"
              size="sm"
              data-testid="wb-bar-show-now"
              onClick={() => void setSetting("bar_hidden_until_ms", null)}
            >
              {t("settings.bar.showNow")}
            </Button>
          </div>
        )}
      </ToggleRow>
      <SettingRow
        title={t("settings.bar.position")}
        testId="wb-bar-position-row"
        control={(ids) => (
          <>
            {positions > 0 && (
              <Button
                variant="ghost"
                size="sm"
                data-testid="wb-bar-reset-positions"
                onClick={() => void setSetting("bar_x_by_display", {})}
              >
                {t("settings.bar.resetPositions")}
              </Button>
            )}
            <Select
              value={settings.bar_position}
              disabled={!!pending.bar_position}
              labelledBy={ids.labelledBy}
              options={POSITIONS.map((p) => ({
                value: p,
                label: t(`settings.bar.positions.${p}`),
              }))}
              onValueChange={(value) =>
                void setSetting("bar_position", value as BarPosition)
              }
              testId="wb-bar-position"
              className="w-52"
            />
          </>
        )}
      />
      <ToggleRow
        label={t("settings.bar.hideInRecordings")}
        hint={t("settings.bar.hideInRecordingsHelp")}
        checked={settings.bar_hide_in_recordings}
        disabled={!!pending.bar_hide_in_recordings}
        onChange={(checked) =>
          void setSetting("bar_hide_in_recordings", checked)
        }
        testId="wb-bar-protect"
      />
    </div>
  );
}
