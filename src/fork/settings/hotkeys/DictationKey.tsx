// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The dictation key in one step (owner test: changing fn showed every
 * shortcut variation at once). One row, "Dictation key [fn] Change"; Change
 * turns the row into a capture: "Press the key or keys you want", the keys
 * drawn live as caps while held, then Cancel or Save. Saving moves hold to
 * talk, the hands-free double-tap and key+Space together, as the engine runs
 * them on one trigger. Problems (not a usable key, already taken, Secure
 * Input) are one inline line. Every other shortcut lives under "More
 * shortcuts".
 */
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../../i18n";
import { Button, SettingRow } from "../../ui";
import { useForkSettings } from "../useForkSettings";
import { useKeyRecorder } from "./Capture";
import { ChordKeys } from "./ChordKeys";
import {
  chordLabel,
  dictationKeyBindings,
  dictationKeyConflict,
  triggerChord,
} from "./model";

export function DictationKeyRow({
  startEditing = false,
  onClose,
  actionName,
  className,
}: {
  className?: string;
  /** Open straight in capture (onboarding's dialog). */
  startEditing?: boolean;
  /** After Save or Cancel (onboarding closes its dialog). */
  onClose?: (saved: boolean) => void;
  /** The shown name of an action id, for a conflict. */
  actionName?: (id: string) => string;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings } = useForkSettings();
  const [editing, setEditing] = useState(startEditing);
  const current = settings?.hotkeys.dictate?.find(
    (binding) => binding.activation === "hold",
  )?.chord;

  return (
    <SettingRow
      title={t("settingsModal.hotkeys.key.title")}
      description={t("settingsModal.hotkeys.key.hint")}
      testId="wb-dictation-key"
      className={className}
      control={
        editing ? undefined : (
          <>
            <span
              className="flex items-center text-[14px] text-(--wb-ink-2)"
              data-testid="wb-dictation-key-current"
            >
              {current ? (
                <>
                  <span aria-hidden="true">
                    <ChordKeys chord={current} />
                  </span>
                  <span className="sr-only">
                    {t("settingsModal.hotkeys.current", {
                      shortcut: chordLabel(current),
                    })}
                  </span>
                </>
              ) : (
                t("settingsModal.hotkeys.notSet")
              )}
            </span>
            <Button
              variant="outline"
              size="sm"
              disabled={!settings}
              onClick={() => setEditing(true)}
              data-testid="wb-dictation-key-change"
            >
              {t("settingsModal.general.change")}
            </Button>
          </>
        )
      }
    >
      {editing && settings && (
        <KeyCapture
          current={current}
          actionName={actionName}
          onClose={(saved) => {
            setEditing(false);
            onClose?.(saved);
          }}
        />
      )}
    </SettingRow>
  );
}

export function KeyCapture({
  current,
  actionName,
  onClose,
}: {
  current: string | undefined;
  actionName?: (id: string) => string;
  onClose: (saved: boolean) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  const [live, setLive] = useState("");
  const [picked, setPicked] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    cancelRef.current?.focus();
  }, []);
  const { ready, error } = useKeyRecorder({
    actionId: "dictate",
    onLive: setLive,
    onChord: (chord) => {
      if (saving) return;
      const key = triggerChord(chord);
      if (!key) {
        setPicked(null);
        setProblem(t("settingsModal.hotkeys.key.invalid"));
        return;
      }
      const other = settings && dictationKeyConflict(settings.hotkeys, key);
      if (other) {
        setPicked(null);
        setProblem(
          t("settingsModal.hotkeys.conflict", {
            action: actionName
              ? actionName(other)
              : t(`settingsModal.hotkeys.actions.${other}`, {
                  defaultValue: other,
                }),
          }),
        );
        return;
      }
      setProblem(null);
      setPicked(key);
    },
    onEscape: () => onClose(false),
  });

  const save = async () => {
    if (!picked || !settings) return;
    if (picked === current) {
      onClose(false);
      return;
    }
    setSaving(true);
    const next = dictationKeyBindings(settings.hotkeys, picked);
    for (const action of ["dictate", "hands_free"] as const) {
      const result = await setSetting("hotkey", {
        action,
        bindings: next[action],
      });
      if (result.status === "error") {
        setSaving(false);
        setProblem(t("settingsModal.hotkeys.saveFailed"));
        return;
      }
    }
    setSaving(false);
    onClose(true);
  };

  const shown = live || picked || current || "";
  const message = error ? t(`settingsModal.hotkeys.capture.${error}`) : problem;
  return (
    <div
      className="flex flex-col gap-3"
      data-testid="wb-dictation-key-capture"
      data-ready={ready || undefined}
    >
      <p role="status" className="text-[14px] leading-5 text-(--wb-ink)">
        {t("settingsModal.hotkeys.key.prompt")}
        {picked && (
          <span className="sr-only">
            {" "}
            {t("settingsModal.hotkeys.current", {
              shortcut: chordLabel(picked),
            })}
          </span>
        )}
      </p>
      <div
        className="flex h-14 items-center justify-center rounded-(--wb-radius-sm) border border-(--wb-accent) bg-(--wb-surface) outline-1 -outline-offset-1 outline-(--wb-accent) outline-solid"
        aria-hidden="true"
        data-testid="wb-dictation-key-field"
        data-live={live || undefined}
      >
        {shown && (
          <ChordKeys
            chord={shown}
            pressed={Boolean(live)}
            capClassName="h-8 min-w-8 px-2 text-[15px]"
          />
        )}
      </div>
      {message && (
        <p
          role="alert"
          className="text-[13px] leading-[18px] text-(--wb-danger)"
          data-testid="wb-dictation-key-problem"
        >
          {message}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button
          ref={cancelRef}
          variant="secondary"
          size="sm"
          onClick={() => onClose(false)}
          data-testid="wb-dictation-key-cancel"
        >
          {t("settingsModal.hotkeys.capture.cancel")}
        </Button>
        <Button
          variant="primary"
          size="sm"
          disabled={!picked || saving}
          onClick={() => void save()}
          data-testid="wb-dictation-key-save"
        >
          {t("settingsModal.hotkeys.key.save")}
        </Button>
      </div>
    </div>
  );
}
