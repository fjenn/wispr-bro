// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Apple's Dictation on "Press fn twice" (OT2, `shortcut/apple_dictation.rs`).
 * While fn is the trigger the app switches that shortcut off itself (the
 * owner's call); Settings › Shortcuts shows a switch to keep Apple's, and
 * when it still fires (kept, or macOS refused) a line with "Open Keyboard
 * settings". The state is re-read whenever the window becomes active again,
 * so a change made in System Settings shows at once.
 */
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { commands, type HotkeyStatus } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { Button, Switch } from "../../ui";
import { SettingRow } from "../../ui/SettingRow";
import { TriangleAlert } from "../../ui/icons";
import { useForkSettings } from "../useForkSettings";

/** Calls `refresh` when the window or tab becomes active again. */
export function useRecheckOnFocus(refresh: () => void) {
  useEffect(() => {
    const onFocus = () => refresh();
    const onVisible = () => {
      if (document.visibilityState === "visible") refresh();
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [refresh]);
}

export function openKeyboardSettings() {
  void commands.forkOnboardingOpenPane("keyboard");
}

/** "Apple's Dictation still starts on fn twice" with the one-click fix. */
export function AppleDictationWarning({
  preview = false,
}: {
  preview?: boolean;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div
      role="status"
      data-testid="wb-apple-dictation-warning"
      className="flex flex-wrap items-center gap-x-3 gap-y-2 text-[13px] leading-[18px] text-(--wb-ink)"
    >
      <TriangleAlert size={16} aria-hidden="true" className="shrink-0" />
      <span className="min-w-0 flex-1">
        {t("settingsModal.hotkeys.appleDictation.active")}
      </span>
      <Button
        size="sm"
        onClick={() => {
          if (!preview) openKeyboardSettings();
        }}
        data-testid="wb-apple-dictation-open"
      >
        {t("onboardingFlow.shortcut.diagnoses.globe.open")}
      </Button>
    </div>
  );
}

/** The Settings › Shortcuts row (only while fn is the trigger). */
export function AppleDictationRow({
  status,
  onChanged,
}: {
  status: HotkeyStatus | null;
  onChanged: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  if (!status || status.trigger !== "fn" || !settings) return null;
  const on = settings.apple_dictation_shortcut_off;
  const state = status.apple_dictation ?? "none";
  // Nothing to say when Apple's shortcut is not on fn twice and the user
  // never turned the switch off.
  if (state === "none" && on) return null;
  return (
    <SettingRow
      testId="wb-apple-dictation-row"
      title={t("settingsModal.hotkeys.appleDictation.title")}
      description={t("settingsModal.hotkeys.appleDictation.description")}
      control={(ids) => (
        <Switch
          checked={on}
          onChange={(next) => {
            void setSetting("apple_dictation_shortcut_off", next).then(
              onChanged,
            );
          }}
          ariaLabelledBy={ids.labelledBy}
          ariaDescribedBy={ids.describedBy}
          testId="wb-apple-dictation-switch"
        />
      )}
    >
      {state === "active" && <AppleDictationWarning />}
    </SettingRow>
  );
}
