// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { X } from "../ui/icons";
import { FORK_NAMESPACE } from "../i18n";
import { Capture } from "../settings/hotkeys/Capture";
import { useForkSettings } from "../settings/useForkSettings";
import { Button, FOCUS_RING } from "../ui";

export const SCRATCHPAD_ACTION = "scratchpad.open";
export const PROPOSED_SHORTCUT = "ctrl+option+s";

export function ShortcutDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { setSetting } = useForkSettings();
  const dialogRef = useRef<HTMLDialogElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const [capturing, setCapturing] = useState(false);
  const [error, setError] = useState(false);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (dialog && !dialog.open) dialog.showModal();
    closeRef.current?.focus();
  }, []);

  const save = async (chord: string) => {
    const result = await setSetting("hotkey", {
      action: SCRATCHPAD_ACTION,
      bindings: [{ chord, activation: "press" }],
    });
    if (result.status === "ok") onClose();
    else setError(true);
  };

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="wb-scratchpad-shortcut-title"
      className="m-auto w-[min(460px,calc(100vw-32px))] max-w-none rounded-2xl border border-(--wb-line) bg-(--wb-canvas) p-0 text-(--wb-ink) shadow-(--wb-shadow) backdrop:bg-black/45"
      data-testid="wb-scratchpad-shortcut-dialog"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <div className="flex items-start justify-between gap-4 border-b border-(--wb-line) px-5 py-4">
        <div>
          <h2
            id="wb-scratchpad-shortcut-title"
            className="font-(--wb-font-display) text-2xl"
          >
            {t("scratchpad.shortcut.title")}
          </h2>
          <p className="mt-1 text-sm text-(--wb-ink-2)">
            {t("scratchpad.shortcut.help")}
          </p>
        </div>
        <button
          ref={closeRef}
          type="button"
          onClick={onClose}
          aria-label={t("scratchpad.shortcut.close")}
          title={t("scratchpad.shortcut.close")}
          className={`grid size-8 shrink-0 cursor-pointer place-items-center rounded-full text-(--wb-ink-2) hover:bg-(--wb-hover) ${FOCUS_RING}`}
          data-testid="wb-scratchpad-shortcut-close"
        >
          <X size={18} aria-hidden="true" />
        </button>
      </div>
      <div className="flex flex-col gap-4 p-5">
        <div className="rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-4">
          <p className="text-sm font-medium">
            {t("scratchpad.shortcut.proposed")}
          </p>
          <p
            className="mt-1 font-mono text-lg"
            data-testid="wb-scratchpad-proposed"
          >
            {t("scratchpad.shortcut.ctrlOptionS")}
          </p>
          <Button
            variant="primary"
            className="mt-3"
            onClick={() => void save(PROPOSED_SHORTCUT)}
            data-testid="wb-scratchpad-use-proposed"
          >
            {t("scratchpad.shortcut.use")}
          </Button>
        </div>
        {capturing ? (
          <Capture
            actionId={SCRATCHPAD_ACTION}
            onDone={(chord) => {
              setCapturing(false);
              if (chord) void save(chord);
            }}
          />
        ) : (
          <Button
            onClick={() => setCapturing(true)}
            data-testid="wb-scratchpad-capture"
          >
            {t("scratchpad.shortcut.chooseAnother")}
          </Button>
        )}
        {error && (
          <p role="alert" className="text-sm text-red-700 dark:text-red-300">
            {t("scratchpad.shortcut.error")}
          </p>
        )}
      </div>
    </dialog>
  );
}
