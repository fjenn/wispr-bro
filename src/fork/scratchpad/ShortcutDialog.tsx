// SPDX-License-Identifier: GPL-3.0-or-later
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Capture } from "../settings/hotkeys/Capture";
import { useForkSettings } from "../settings/useForkSettings";
import { Button, Dialog, KbdCombo } from "../ui";

export const SCRATCHPAD_ACTION = "scratchpad.open";
export const PROPOSED_SHORTCUT = "ctrl+option+s";

export function ShortcutDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { setSetting } = useForkSettings();
  // The safe first focus: nothing is bound until the user picks.
  const closeRef = useRef<HTMLButtonElement>(null);
  const [capturing, setCapturing] = useState(false);
  const [error, setError] = useState(false);

  const save = async (chord: string) => {
    const result = await setSetting("hotkey", {
      action: SCRATCHPAD_ACTION,
      bindings: [{ chord, activation: "press" }],
    });
    if (result.status === "ok") onClose();
    else setError(true);
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={t("scratchpad.shortcut.title")}
      description={t("scratchpad.shortcut.help")}
      closeLabel={t("scratchpad.shortcut.close")}
      closeRef={closeRef}
      closeTestId="wb-scratchpad-shortcut-close"
      initialFocusRef={closeRef as React.RefObject<HTMLElement>}
      contentFades={false}
      testId="wb-scratchpad-shortcut-dialog"
      footer={
        <div className="flex w-full items-center justify-between gap-2">
          {capturing ? (
            <span />
          ) : (
            <Button
              onClick={() => setCapturing(true)}
              data-testid="wb-scratchpad-capture"
            >
              {t("scratchpad.shortcut.chooseAnother")}
            </Button>
          )}
          <Button
            variant="primary"
            onClick={() => void save(PROPOSED_SHORTCUT)}
            data-testid="wb-scratchpad-use-proposed"
          >
            {t("scratchpad.shortcut.use")}
          </Button>
        </div>
      }
    >
      <div className="flex flex-col gap-3">
        <div className="flex items-center justify-between gap-4 rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-5 py-4">
          <div className="min-w-0">
            <p className="text-[15px] font-semibold text-(--wb-ink)">
              {t("scratchpad.shortcut.proposed")}
            </p>
            <p
              className="mt-0.5 text-[13px] text-(--wb-ink-2)"
              data-testid="wb-scratchpad-proposed"
            >
              {t("scratchpad.shortcut.ctrlOptionS")}
            </p>
          </div>
          <span aria-hidden="true">
            <KbdCombo keys={["⌃", "⌥", "S"]} />
          </span>
        </div>
        {capturing && (
          <div className="rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-5 py-4">
            <Capture
              actionId={SCRATCHPAD_ACTION}
              onDone={(chord) => {
                setCapturing(false);
                if (chord) void save(chord);
              }}
            />
          </div>
        )}
        {error && (
          <p
            role="alert"
            className="rounded-(--wb-radius-sm) bg-(--wb-danger-soft) px-3 py-2 text-[14px] text-(--wb-ink)"
          >
            {t("scratchpad.shortcut.error")}
          </p>
        )}
      </div>
    </Dialog>
  );
}
