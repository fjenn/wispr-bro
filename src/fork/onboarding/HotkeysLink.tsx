// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Keyboard } from "lucide-react";
import { useSettings } from "@/hooks/useSettings";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { FORK_NAMESPACE } from "../i18n";
import { HotkeysPanel } from "../settings/hotkeys/HotkeysPanel";
import { Button, FOCUS_RING } from "../ui";

export function OnboardingHotkeysLink() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings } = useSettings();
  const [open, setOpen] = useState(false);
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (open && dialog && !dialog.open) dialog.showModal();
  }, [open]);

  if (settings?.onboarding_completed !== false) return null;
  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className={`fixed bottom-5 end-5 z-40 flex cursor-pointer items-center gap-2 rounded-full border border-(--wb-line-strong) bg-(--wb-card) px-3 py-2 text-sm text-(--wb-ink) shadow-(--wb-shadow) ${FOCUS_RING}`}
        data-testid="wb-onboarding-shortcuts"
      >
        <Keyboard size={16} aria-hidden="true" />
        {t("settingsModal.hotkeys.open")}
      </button>
      {open && (
        <dialog
          ref={dialogRef}
          aria-labelledby="wb-onboarding-hotkeys-title"
          dir={getLanguageDirection(i18n.language)}
          className="m-auto h-[min(680px,calc(100vh-32px))] w-[min(760px,calc(100vw-32px))] max-w-none overflow-hidden rounded-2xl border border-(--wb-line) bg-(--wb-canvas) p-0 text-(--wb-ink) shadow-(--wb-shadow) backdrop:bg-black/45"
          data-testid="wb-onboarding-hotkeys-modal"
          onCancel={(event) => {
            event.preventDefault();
            setOpen(false);
          }}
        >
          <div className="flex h-full flex-col">
            <header className="flex items-center justify-between border-b border-(--wb-line) px-6 py-5">
              <h2
                id="wb-onboarding-hotkeys-title"
                className="font-(--wb-font-display) text-2xl"
              >
                {t("settingsModal.sections.hotkeys")}
              </h2>
              <Button onClick={() => setOpen(false)}>
                {t("settingsModal.close")}
              </Button>
            </header>
            <div className="min-h-0 flex-1 overflow-y-auto p-6">
              <HotkeysPanel onDone={() => setOpen(false)} />
            </div>
          </div>
        </dialog>
      )}
    </>
  );
}
