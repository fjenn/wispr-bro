// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One-time Hub note after the app switched Apple Dictation's "Press fn
 * twice" off (OT2, `shortcut/apple_dictation.rs`): what was changed, why,
 * and the way back (Settings › Shortcuts has the switch). Dismissal is kept
 * in `dismissed_banners`, so it never returns. Lowest priority in the Hub's
 * one notice slot (`HubNotices.tsx`).
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
import type { HubNotice } from "./HubNotices";

export const APPLE_DICTATION_NOTE_ID = "hub.apple-dictation-off";

export function useAppleDictationNotice(): HubNotice | null {
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
  return {
    id: "apple-dictation",
    testId: "wb-apple-dictation-note",
    role: "status",
    icon: <Info size={18} aria-hidden className="shrink-0" />,
    message: t("hub.appleDictationNote.body"),
    actions: (
      <>
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
      </>
    ),
  };
}
