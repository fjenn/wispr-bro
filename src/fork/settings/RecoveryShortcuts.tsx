// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Recovery shortcuts (S-06): Paste last dictation (⌃⌘V), Copy last
 * dictation (⌃⌘C), Press Enter (unset; a key or a mouse button) and Undo a
 * cancel (unset; the bar's Undo does the same). Each row shows its current
 * shortcut with the layout-aware label from `fork_hotkey_status`, and can
 * record a new one, clear it, or go back to the default.
 *
 * Recording uses Handy's own recorder (`start_handy_keys_recording`, which
 * reports keys and mouse buttons as `handy-keys-event`): the shortcut is the
 * chord held when a key or button is released. Escape alone cancels. A
 * binding the engine cannot run (Return for Press Enter, a chord another
 * action uses) is saved but reported, as `fork_hotkey_status` lists it.
 *
 * Mounted on the fork debug panel until S-14's Settings modal takes it
 * (Settings › Shortcuts).
 */
import { useCallback, useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { commands as handyCommands } from "@/bindings";
import { commands, type HotkeyBinding, type HotkeyStatus } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "./useForkSettings";

type ActionId = "paste_last" | "copy_last" | "press_enter" | "undo_cancel";

const ACTIONS: { id: ActionId; key: string; defaultChord: string | null }[] = [
  { id: "paste_last", key: "pasteLast", defaultChord: "ctrl+command+v" },
  { id: "copy_last", key: "copyLast", defaultChord: "ctrl+command+c" },
  { id: "press_enter", key: "pressEnter", defaultChord: null },
  { id: "undo_cancel", key: "undoCancel", defaultChord: null },
];

interface HandyKeysEvent {
  modifiers: string[];
  key: string | null;
  is_key_down: boolean;
  hotkey_string: string;
}

export function RecoveryShortcuts() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  const [status, setStatus] = useState<HotkeyStatus | null>(null);
  const [recording, setRecording] = useState<ActionId | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await commands.forkHotkeyStatus();
      if (result.status === "ok") setStatus(result.data);
    } catch (e) {
      console.warn("fork: hotkey status failed", e);
    }
  }, []);

  // Labels and issues follow every settings change.
  useEffect(() => {
    void refresh();
  }, [refresh, settings?.hotkeys]);

  const save = useCallback(
    async (action: ActionId, bindings: HotkeyBinding[]) => {
      setError(null);
      const result = await setSetting("hotkey", { action, bindings });
      if (result.status === "error")
        setError(t("settings.recovery.saveFailed"));
    },
    [setSetting, t],
  );

  if (!settings) return null;

  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="wb-recovery">
      <p className="text-xs text-ink-3">{t("settings.recovery.description")}</p>
      {ACTIONS.map((a) => (
        <ShortcutRow
          key={a.id}
          action={a}
          bindings={settings.hotkeys[a.id] ?? []}
          status={status}
          recording={recording === a.id}
          busy={recording !== null && recording !== a.id}
          onRecord={() => {
            setError(null);
            setRecording(a.id);
          }}
          onRecorded={(chord) => {
            setRecording(null);
            if (chord) void save(a.id, [{ chord, activation: "press" }]);
          }}
          onRecordFailed={() => {
            setRecording(null);
            setError(t("settings.recovery.recordFailed"));
          }}
          onClear={() => void save(a.id, [])}
          onDefault={() =>
            a.defaultChord &&
            void save(a.id, [{ chord: a.defaultChord, activation: "press" }])
          }
        />
      ))}
      {error && (
        <p role="alert" className="text-xs text-red-600 dark:text-red-400">
          {error}
        </p>
      )}
    </div>
  );
}

function ShortcutRow({
  action,
  bindings,
  status,
  recording,
  busy,
  onRecord,
  onRecorded,
  onRecordFailed,
  onClear,
  onDefault,
}: {
  action: (typeof ACTIONS)[number];
  bindings: HotkeyBinding[];
  status: HotkeyStatus | null;
  recording: boolean;
  busy: boolean;
  onRecord: () => void;
  onRecorded: (chord: string | null) => void;
  onRecordFailed: () => void;
  onClear: () => void;
  onDefault: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const titleId = useId();
  const helpId = useId();
  const chord = bindings[0]?.chord ?? null;
  const label = status?.bindings.find(
    (b) => b.action === action.id && b.chord === chord,
  )?.label;
  const unusable = status?.issues.some((i) => i.action === action.id) ?? false;
  const isDefault = chord === action.defaultChord;
  const shown = chord ? (label?.text ?? chord) : t("settings.recovery.notSet");

  return (
    <section
      aria-labelledby={titleId}
      aria-describedby={helpId}
      className="flex flex-col gap-1"
      data-testid={`wb-recovery-${action.id}`}
    >
      <div className="flex flex-wrap items-center gap-2">
        <h4 id={titleId} className="font-medium me-auto">
          {t(`settings.recovery.${action.key}.title`)}
        </h4>
        <kbd
          className="rounded border border-line-strong bg-wash px-1.5 py-0.5 font-sans text-xs"
          data-testid={`wb-recovery-${action.id}-chord`}
          data-set={chord ? "true" : "false"}
          aria-label={
            chord
              ? t("settings.recovery.current", {
                  shortcut: label?.spoken ?? chord,
                })
              : t("settings.recovery.notSet")
          }
        >
          {recording ? t("settings.recovery.listening") : shown}
        </kbd>
        {recording ? (
          <Recorder
            actionId={action.id}
            onDone={onRecorded}
            onFailed={onRecordFailed}
          />
        ) : (
          <button
            type="button"
            className="rounded border border-line-strong bg-wash px-2 py-0.5 hover:bg-logo-primary/10 disabled:opacity-50"
            data-testid={`wb-recovery-${action.id}-record`}
            disabled={busy}
            onClick={onRecord}
          >
            {t("settings.recovery.record")}
          </button>
        )}
        {chord && !recording && (
          <button
            type="button"
            className="rounded border border-line-strong bg-wash px-2 py-0.5 hover:bg-logo-primary/10 disabled:opacity-50"
            data-testid={`wb-recovery-${action.id}-clear`}
            disabled={busy}
            onClick={onClear}
          >
            {t("settings.recovery.clear")}
          </button>
        )}
        {action.defaultChord && !isDefault && !recording && (
          <button
            type="button"
            className="rounded border border-line-strong bg-wash px-2 py-0.5 hover:bg-logo-primary/10 disabled:opacity-50"
            data-testid={`wb-recovery-${action.id}-default`}
            disabled={busy}
            onClick={onDefault}
          >
            {t("settings.recovery.useDefault")}
          </button>
        )}
      </div>
      <p id={helpId} className="text-xs text-ink-3">
        {t(`settings.recovery.${action.key}.help`)}
        {action.id === "press_enter" && (
          <>
            {" "}
            <span data-testid="wb-recovery-mouse-note">
              {t("settings.recovery.pressEnter.mouse")}
            </span>
          </>
        )}
      </p>
      {unusable && chord && (
        <p
          role="alert"
          className="text-xs text-red-600 dark:text-red-400"
          data-testid={`wb-recovery-${action.id}-issue`}
        >
          {t("settings.recovery.unusable")}
        </p>
      )}
    </section>
  );
}

/** The live part of a row while it records: the Cancel button and Handy's
 * recorder with its key events. */
function Recorder({
  actionId,
  onDone,
  onFailed,
}: {
  actionId: ActionId;
  onDone: (chord: string | null) => void;
  onFailed: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const done = useRef(false);
  const doneRef = useRef(onDone);
  doneRef.current = onDone;
  const failedRef = useRef(onFailed);
  failedRef.current = onFailed;

  useEffect(() => {
    cancelRef.current?.focus();
    let keyed = "";
    let stopped = false;
    const finish = (chord: string | null) => {
      if (done.current) return;
      done.current = true;
      doneRef.current(chord);
    };
    const pending = listen<HandyKeysEvent>("handy-keys-event", (event) => {
      if (stopped) return;
      const { hotkey_string, is_key_down, key, modifiers } = event.payload;
      if (is_key_down && key && hotkey_string) {
        keyed = hotkey_string;
      } else if (!is_key_down && key) {
        stopped = true;
        // Escape alone cancels; anything else with a key is the shortcut.
        const chord = keyed || hotkey_string;
        finish(key === "escape" && !modifiers.length ? null : chord || null);
      }
    });
    // Escape in this window cancels too, and must not close the panel.
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      stopped = true;
      finish(null);
    };
    window.addEventListener("keydown", onKey, true);
    void handyCommands
      .startHandyKeysRecording(`fork:${actionId}`)
      .then((result) => {
        if (result.status === "error" && !done.current) {
          done.current = true;
          failedRef.current();
        }
      })
      .catch(() => {
        if (!done.current) {
          done.current = true;
          failedRef.current();
        }
      });
    return () => {
      stopped = true;
      window.removeEventListener("keydown", onKey, true);
      void pending.then((unlisten) => unlisten());
      void handyCommands.stopHandyKeysRecording().catch(() => undefined);
    };
  }, [actionId]);

  return (
    <>
      <span role="status" className="sr-only">
        {t("settings.recovery.listeningHelp")}
      </span>
      <button
        ref={cancelRef}
        type="button"
        className="rounded border border-line-strong bg-wash px-2 py-0.5 hover:bg-logo-primary/10"
        data-testid={`wb-recovery-${actionId}-cancel`}
        onClick={() => {
          if (done.current) return;
          done.current = true;
          doneRef.current(null);
        }}
      >
        {t("settings.recovery.cancel")}
      </button>
    </>
  );
}
