// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { commands as handyCommands } from "@/bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { Button } from "../../ui";

interface HandyKeysEvent {
  modifiers: string[];
  key: string | null;
  is_key_down: boolean;
  hotkey_string: string;
}

interface CaptureProps {
  actionId: string;
  onDone: (chord: string | null) => void;
}

export type CaptureError = "secure" | "failed";

interface KeyRecorderOptions {
  actionId: string;
  /** The keys held right now ("" when none), for live key caps. */
  onLive?: (chord: string) => void;
  /** A finished press: the keys that were down when it ended. */
  onChord: (chord: string) => void;
  /** Esc (as a page key) ends the recorder. */
  onEscape: () => void;
}

/**
 * Handy's key recorder (`startHandyKeysRecording`) while mounted: each
 * finished press, modifier-only presses included, reaches `onChord`.
 */
export function useKeyRecorder({
  actionId,
  onLive,
  onChord,
  onEscape,
}: KeyRecorderOptions): { ready: boolean; error: CaptureError | null } {
  const [error, setError] = useState<CaptureError | null>(null);
  const [ready, setReady] = useState(false);
  const callbacks = useRef({ onLive, onChord, onEscape });
  callbacks.current = { onLive, onChord, onEscape };

  useEffect(() => {
    let disposed = false;
    let keyed = "";
    let modifierOnly = "";
    let unlisten: (() => void) | undefined;
    const finish = (chord: string) => {
      keyed = "";
      modifierOnly = "";
      callbacks.current.onLive?.("");
      callbacks.current.onChord(chord);
    };
    const pending = listen<HandyKeysEvent>("handy-keys-event", (event) => {
      const { hotkey_string, is_key_down, key, modifiers } = event.payload;
      if (is_key_down && hotkey_string) {
        if (key) keyed = hotkey_string;
        else if (!keyed) modifierOnly = hotkey_string;
        callbacks.current.onLive?.(hotkey_string);
      } else if (!is_key_down && key) {
        const chord = keyed || hotkey_string;
        if (chord) finish(chord);
      } else if (!is_key_down && !key && !modifiers.length && !keyed) {
        if (modifierOnly) finish(modifierOnly);
      }
    });
    void pending.then((off) => {
      if (disposed) off();
      else unlisten = off;
    });

    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      callbacks.current.onEscape();
    };
    window.addEventListener("keydown", onKey, true);

    void (async () => {
      try {
        const secure = await handyCommands.getSecureInputStatus();
        if (secure.enabled) {
          if (!disposed) setError("secure");
          return;
        }
        const result = await handyCommands.startHandyKeysRecording(
          `fork:${actionId}`,
        );
        if (result.status === "error") {
          if (!disposed) {
            setError(
              String(result.error).includes("secure-input-active")
                ? "secure"
                : "failed",
            );
          }
          return;
        }
        if (!disposed) setReady(true);
      } catch (cause) {
        if (!disposed) {
          setError(
            String(cause).includes("secure-input-active") ? "secure" : "failed",
          );
        }
      }
    })();

    return () => {
      disposed = true;
      window.removeEventListener("keydown", onKey, true);
      unlisten?.();
      void handyCommands.stopHandyKeysRecording().catch(() => undefined);
    };
  }, [actionId]);

  return { ready, error };
}

export function Capture({ actionId, onDone }: CaptureProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const done = useRef(false);
  const finish = (chord: string | null) => {
    if (done.current) return;
    done.current = true;
    onDone(chord);
  };
  const { ready, error } = useKeyRecorder({
    actionId,
    onChord: finish,
    onEscape: () => finish(null),
  });
  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  return (
    <div
      className="flex flex-col items-start gap-2"
      data-testid="wb-hotkey-capture"
      data-ready={ready || undefined}
    >
      <span role="status" className="sr-only">
        {ready
          ? t("settingsModal.hotkeys.capture.listeningHelp")
          : t("settingsModal.hotkeys.capture.starting")}
      </span>
      {error && (
        <p
          role="alert"
          className="text-[13px] leading-[18px] text-(--wb-danger)"
          data-testid="wb-hotkey-capture-error"
        >
          {t(`settingsModal.hotkeys.capture.${error}`)}
        </p>
      )}
      <Button
        ref={cancelRef}
        variant="ghost"
        size="sm"
        data-testid="wb-hotkey-capture-cancel"
        onClick={() => finish(null)}
      >
        {error
          ? t("settingsModal.hotkeys.capture.close")
          : t("settingsModal.hotkeys.capture.cancel")}
      </Button>
    </div>
  );
}
