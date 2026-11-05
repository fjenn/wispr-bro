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
import { Button, FOCUS_RING, Select } from "../ui";
import { Play } from "../ui/icons";
import { ToggleRow } from "./Controls";

type Theme = "marimba" | "pop" | "custom";

export function SoundSettings() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, isUpdating, updateSetting } = useSettings();
  const playTestSound = useSettingsStore((s) => s.playTestSound);
  const customSounds = useSettingsStore((s) => s.customSounds);
  const checkCustomSounds = useSettingsStore((s) => s.checkCustomSounds);
  const themeId = useId();
  const volumeId = useId();

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
    <div className="flex flex-col gap-3" data-testid="wb-sound-settings">
      <ToggleRow
        label={t("settings.sound.enabled")}
        hint={t("settings.sound.enabledHelp")}
        checked={!!sounds}
        disabled={isUpdating("audio_feedback")}
        onChange={(checked) => void updateSetting("audio_feedback", checked)}
        testId="wb-sound-enabled"
      >
        <div
          className={`flex flex-wrap items-center gap-x-5 gap-y-3 border-t border-(--wb-line) pt-3 text-[14px] ${sounds ? "" : "opacity-60"}`}
        >
          <div className="flex items-center gap-2">
            <span id={themeId} className="text-(--wb-ink-2)">
              {t("settings.sound.theme")}
            </span>
            <Select
              value={theme}
              size="sm"
              labelledBy={themeId}
              disabled={!sounds || isUpdating("sound_theme")}
              options={themes.map((th) => ({
                value: th,
                label: t(`settings.sound.themes.${th}`),
              }))}
              onValueChange={(value) =>
                void updateSetting("sound_theme", value as Theme)
              }
              testId="wb-sound-theme"
              className="w-32"
            />
          </div>
          <label htmlFor={volumeId} className="flex items-center gap-2">
            <span className="text-(--wb-ink-2)">
              {t("settings.sound.volume")}
            </span>
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
              className={`w-32 cursor-pointer accent-(--wb-ink) disabled:cursor-default ${FOCUS_RING}`}
            />
          </label>
          <Button
            variant="outline"
            size="sm"
            data-testid="wb-sound-preview"
            disabled={!sounds}
            onClick={() => void preview()}
            className="ms-auto"
          >
            <Play size={14} />
            {t("settings.sound.preview")}
          </Button>
        </div>
      </ToggleRow>
      <ToggleRow
        label={t("settings.sound.mute")}
        hint={t("settings.sound.muteHelp")}
        checked={!!settings.mute_while_recording}
        disabled={isUpdating("mute_while_recording")}
        onChange={(checked) =>
          void updateSetting("mute_while_recording", checked)
        }
        testId="wb-sound-mute"
      />
    </div>
  );
}
