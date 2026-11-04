// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The hotkeys panel (S-14) in a modal dialog, for onboarding's shortcut
 * test ("Edit shortcut", S-29a). Escape and Done close it. On the shared
 * `Dialog` since Design W1.
 */
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { HotkeysPanel } from "../settings/hotkeys/HotkeysPanel";
import { Dialog } from "../ui";

export function HotkeysDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      title={t("settingsModal.sections.hotkeys")}
      closeLabel={t("settingsModal.close")}
      className="h-[min(680px,calc(100vh-32px))] max-w-[760px]!"
      testId="wb-onboarding-hotkeys-modal"
    >
      <HotkeysPanel onDone={onClose} />
    </Dialog>
  );
}
