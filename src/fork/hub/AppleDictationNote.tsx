// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One-time Hub note after the app switched Apple Dictation's "Press fn
 * twice" off (OT2, `shortcut/apple_dictation.rs`): what was changed, why,
 * and the way back (Settings › Shortcuts has the switch). Dismissal is kept
 * in `dismissed_banners`, so it never returns.
 */
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type HotkeyStatus } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { Info } from "../ui/icons";
import { useForkSettings } from "../settings/useForkSettings";
import { useRecheckOnFocus } from "../settings/hotkeys/AppleDictation";
import { navigate } from "./nav";

export const APPLE_DICTATION_NOTE_ID = "hub.apple-dictation-off";

export function AppleDictationNote() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  const [status, setStatus] = useState<HotkeyStatus | null>(null);

  const refresh = useCallback(() => {
    commands
      .forkHotkeyStatus()
      .then((result) => {
        if (result.status === "ok") setStatus(result.data);
      })
      .catch(() => undefined);
  }, []);
  useEffect(refresh, [refresh]);
  useRecheckOnFocus(refresh);

  if (
    !settings ||
    status?.apple_dictation !== "turned_off" ||
    settings.dismissed_banners.includes(APPLE_DICTATION_NOTE_ID)
  ) {
    return null;
  }
  const dismiss = () =>
    setSetting("dismissed_banners", [
      ...settings.dismissed_banners,
      APPLE_DICTATION_NOTE_ID,
    ]);
  return (
    <div
      role="status"
      data-testid="wb-apple-dictation-note"
      className="flex flex-wrap items-center gap-3 rounded-(--wb-radius-md) border border-(--wb-line-strong) bg-(--wb-surface) px-4 py-3 text-(--wb-ink)"
    >
      <Info size={18} aria-hidden className="shrink-0" />
      <p className="m-0 min-w-0 flex-1 text-[14px] leading-5">
        {t("hub.appleDictationNote.body")}
      </p>
      <div className="flex gap-2">
        <Button
          variant="secondary"
          onClick={() => {
            void dismiss();
            navigate("settings", "hotkeys");
          }}
          data-testid="wb-apple-dictation-note-settings"
        >
          {t("settingsModal.sections.hotkeys")}
        </Button>
        <Button
          variant="primary"
          onClick={() => void dismiss()}
          data-testid="wb-apple-dictation-note-dismiss"
        >
          {t("hub.featurePopup.dismiss")}
        </Button>
      </div>
    </div>
  );
}
