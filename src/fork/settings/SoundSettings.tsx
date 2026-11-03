// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Sound (S-08): dictation sounds (on by default, DEC-14, with
 * Handy's Marimba and Pop themes), their volume and a preview, and "Mute all
 * audio while dictating" (off by default; CoreAudio, no AppleScript). These
 * are Handy's settings, written through Handy's settings store and commands;
 * the sounds are Handy's own files. S-14 mounts them in Settings › System.
 */
import { useEffect, useId } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import { useSettingsStore } from "@/stores/settingsStore";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

type Theme = "marimba" | "pop" | "custom";

export function SoundSettings() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, isUpdating, updateSetting } = useSettings();
  const playTestSound = useSettingsStore((s) => s.playTestSound);
  const customSounds = useSettingsStore((s) => s.customSounds);
  const checkCustomSounds = useSettingsStore((s) => s.checkCustomSounds);
  const soundsId = useId();
  const soundsHelpId = useId();
  const themeId = useId();
  const volumeId = useId();
  const muteId = useId();
  const muteHelpId = useId();

  useEffect(() => {
    void checkCustomSounds();
  }, [checkCustomSounds]);

  if (!settings) return null;

  const sounds = settings.audio_feedback;
  const theme = (settings.sound_theme ?? "marimba") as Theme;
  const volume = Math.round((settings.audio_feedback_volume ?? 1) * 100);
  const themes: Theme[] = ["marimba", "pop"];
  if (customSounds.start && customSounds.stop) themes.push("custom");

  const preview = async () => {
    await playTestSound("start");
    await playTestSound("stop");
  };

  return (
    <div
      className="flex flex-col gap-2 text-sm"
      data-testid="wb-sound-settings"
    >
      <label htmlFor={soundsId} className="flex items-center gap-2">
        <input
          id={soundsId}
          type="checkbox"
          data-testid="wb-sound-enabled"
          aria-describedby={soundsHelpId}
          checked={sounds}
          disabled={isUpdating("audio_feedback")}
          onChange={(e) =>
            void updateSetting("audio_feedback", e.currentTarget.checked)
          }
        />
        {t("settings.sound.enabled")}
      </label>
      <p id={soundsHelpId} className="-mt-1 ms-6 text-xs text-(--wb-ink-3)">
        {t("settings.sound.enabledHelp")}
      </p>
      <div className="ms-6 flex flex-wrap items-center gap-3">
        <label htmlFor={themeId} className="flex items-center gap-2">
          {t("settings.sound.theme")}
          <select
            id={themeId}
            data-testid="wb-sound-theme"
            value={theme}
            disabled={!sounds || isUpdating("sound_theme")}
            onChange={(e) =>
              void updateSetting("sound_theme", e.currentTarget.value as Theme)
            }
            className="rounded border border-(--wb-line-strong) bg-(--wb-surface) px-1 py-0.5"
          >
            {themes.map((th) => (
              <option key={th} value={th}>
                {t(`settings.sound.themes.${th}`)}
              </option>
            ))}
          </select>
        </label>
        <label htmlFor={volumeId} className="flex items-center gap-2">
          {t("settings.sound.volume")}
          <input
            id={volumeId}
            type="range"
            min={0}
            max={100}
            step={5}
            data-testid="wb-sound-volume"
            value={volume}
            disabled={!sounds}
            aria-valuetext={t("settings.sound.volumeValue", {
              percent: volume,
            })}
            onChange={(e) =>
              void updateSetting(
                "audio_feedback_volume",
                Number(e.currentTarget.value) / 100,
              )
            }
          />
        </label>
        <Button
          data-testid="wb-sound-preview"
          disabled={!sounds}
          onClick={() => void preview()}
        >
          {t("settings.sound.preview")}
        </Button>
      </div>
      <label htmlFor={muteId} className="mt-2 flex items-center gap-2">
        <input
          id={muteId}
          type="checkbox"
          data-testid="wb-sound-mute"
          aria-describedby={muteHelpId}
          checked={settings.mute_while_recording}
          disabled={isUpdating("mute_while_recording")}
          onChange={(e) =>
            void updateSetting("mute_while_recording", e.currentTarget.checked)
          }
        />
        {t("settings.sound.mute")}
      </label>
      <p id={muteHelpId} className="-mt-1 ms-6 text-xs text-(--wb-ink-3)">
        {t("settings.sound.muteHelp")}
      </p>
    </div>
  );
}
