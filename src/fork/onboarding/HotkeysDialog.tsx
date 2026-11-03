// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The hotkeys panel (S-14) in a modal dialog, for onboarding's shortcut
 * test ("Edit shortcut", S-29a). Escape and Done close it.
 */
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { X } from "../ui/icons";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { FORK_NAMESPACE } from "../i18n";
import { HotkeysPanel } from "../settings/hotkeys/HotkeysPanel";
import { FOCUS_RING } from "../ui";

export function HotkeysDialog({ onClose }: { onClose: () => void }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (dialog && !dialog.open) dialog.showModal();
  }, []);

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="wb-onboarding-hotkeys-title"
      dir={getLanguageDirection(i18n.language)}
      className="m-auto h-[min(680px,calc(100vh-32px))] w-[min(760px,calc(100vw-32px))] max-w-none overflow-hidden rounded-2xl border border-(--wb-line) bg-(--wb-canvas) p-0 text-(--wb-ink) shadow-(--wb-shadow) backdrop:bg-black/45"
      data-testid="wb-onboarding-hotkeys-modal"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <div className="flex h-full flex-col">
        <header className="flex items-center justify-between border-b border-(--wb-line) px-6 py-5">
          <h2
            id="wb-onboarding-hotkeys-title"
            className="m-0 font-(--wb-font-display) text-2xl"
          >
            {t("settingsModal.sections.hotkeys")}
          </h2>
          <button
            type="button"
            aria-label={t("settingsModal.close")}
            onClick={onClose}
            className={`grid size-8 cursor-pointer place-items-center rounded-lg text-(--wb-ink-2) hover:bg-(--wb-hover) ${FOCUS_RING}`}
          >
            <X size={18} aria-hidden="true" />
          </button>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto p-6">
          <HotkeysPanel onDone={onClose} />
        </div>
      </div>
    </dialog>
  );
}
