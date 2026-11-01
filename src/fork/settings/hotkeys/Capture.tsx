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

export function Capture({ actionId, onDone }: CaptureProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [error, setError] = useState<"secure" | "failed" | null>(null);
  const [ready, setReady] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const done = useRef(false);
  const doneRef = useRef(onDone);
  doneRef.current = onDone;

  useEffect(() => {
    cancelRef.current?.focus();
    let disposed = false;
    let keyed = "";
    let modifierOnly = "";
    let unlisten: (() => void) | undefined;
    const finish = (chord: string | null) => {
      if (done.current) return;
      done.current = true;
      doneRef.current(chord);
    };
    const pending = listen<HandyKeysEvent>("handy-keys-event", (event) => {
      if (done.current) return;
      const { hotkey_string, is_key_down, key, modifiers } = event.payload;
      if (is_key_down && hotkey_string) {
        if (key) keyed = hotkey_string;
        else modifierOnly = hotkey_string;
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
      finish(null);
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

  return (
    <div
      className="flex flex-col items-start gap-2"
      data-testid="wb-hotkey-capture"
    >
      <span role="status" className="sr-only">
        {t("settingsModal.hotkeys.capture.listeningHelp")}
      </span>
      {error ? (
        <p
          role="alert"
          className="max-w-md text-xs text-red-700 dark:text-red-300"
          data-testid="wb-hotkey-capture-error"
        >
          {t(`settingsModal.hotkeys.capture.${error}`)}
        </p>
      ) : (
        <span className="text-xs text-(--wb-ink-2)">
          {ready
            ? t("settingsModal.hotkeys.capture.listening")
            : t("settingsModal.hotkeys.capture.starting")}
        </span>
      )}
      <Button
        ref={cancelRef}
        data-testid="wb-hotkey-capture-cancel"
        onClick={() => doneRef.current(null)}
      >
        {error
          ? t("settingsModal.hotkeys.capture.close")
          : t("settingsModal.hotkeys.capture.cancel")}
      </Button>
    </div>
  );
}
