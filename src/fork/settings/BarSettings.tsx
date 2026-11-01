// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Flow Bar settings (S-03): "Always show the bar", position, "Hide the bar
 * in screen recordings", a way back from "Hide for an hour", and forgetting
 * dragged positions. S-14 mounts the controls in Settings › Advanced;
 * System also exposes "Always show the bar" through the same fork setting.
 */
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { BarPosition } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "./useForkSettings";

const POSITIONS: BarPosition[] = ["bottom", "top"];

export function BarSettings() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const alwaysId = useId();
  const alwaysHelpId = useId();
  const positionId = useId();
  const protectId = useId();
  const protectHelpId = useId();

  if (!settings) return null;

  const hiddenUntil = settings.bar_hidden_until_ms;
  const hidden = hiddenUntil !== null && hiddenUntil > Date.now();
  const positions = Object.keys(settings.bar_x_by_display).length;

  return (
    <div className="flex flex-col gap-2 text-sm" data-testid="wb-bar-settings">
      <label htmlFor={alwaysId} className="flex items-center gap-2">
        <input
          id={alwaysId}
          type="checkbox"
          data-testid="wb-bar-always"
          aria-describedby={alwaysHelpId}
          checked={settings.bar_always_visible}
          disabled={!!pending.bar_always_visible}
          onChange={(e) =>
            void setSetting("bar_always_visible", e.currentTarget.checked)
          }
        />
        {t("settings.bar.alwaysShow")}
      </label>
      <p id={alwaysHelpId} className="text-xs text-ink-3 -mt-1 ms-6">
        {t("settings.bar.alwaysShowHelp")}
      </p>
      <label htmlFor={positionId} className="flex items-center gap-2">
        {t("settings.bar.position")}
        <select
          id={positionId}
          data-testid="wb-bar-position"
          value={settings.bar_position}
          disabled={!!pending.bar_position}
          onChange={(e) =>
            void setSetting(
              "bar_position",
              e.currentTarget.value as BarPosition,
            )
          }
          className="rounded border border-line-strong bg-wash px-1 py-0.5"
        >
          {POSITIONS.map((p) => (
            <option key={p} value={p}>
              {t(`settings.bar.positions.${p}`)}
            </option>
          ))}
        </select>
      </label>
      <label htmlFor={protectId} className="flex items-center gap-2">
        <input
          id={protectId}
          type="checkbox"
          data-testid="wb-bar-protect"
          aria-describedby={protectHelpId}
          checked={settings.bar_hide_in_recordings}
          disabled={!!pending.bar_hide_in_recordings}
          onChange={(e) =>
            void setSetting("bar_hide_in_recordings", e.currentTarget.checked)
          }
        />
        {t("settings.bar.hideInRecordings")}
      </label>
      <p id={protectHelpId} className="text-xs text-ink-3 -mt-1 ms-6">
        {t("settings.bar.hideInRecordingsHelp")}
      </p>
      {hidden && (
        <div className="flex items-center gap-2" data-testid="wb-bar-hidden">
          <span className="text-xs text-ink-3">
            {t("settings.bar.hiddenUntil", {
              time: new Date(hiddenUntil).toLocaleTimeString(i18n.language, {
                hour: "numeric",
                minute: "2-digit",
              }),
            })}
          </span>
          <button
            type="button"
            className="rounded border border-line-strong px-2 py-0.5 text-xs"
            data-testid="wb-bar-show-now"
            onClick={() => void setSetting("bar_hidden_until_ms", null)}
          >
            {t("settings.bar.showNow")}
          </button>
        </div>
      )}
      {positions > 0 && (
        <button
          type="button"
          className="self-start rounded border border-line-strong px-2 py-0.5 text-xs"
          data-testid="wb-bar-reset-positions"
          onClick={() => void setSetting("bar_x_by_display", {})}
        >
          {t("settings.bar.resetPositions")}
        </button>
      )}
    </div>
  );
}
