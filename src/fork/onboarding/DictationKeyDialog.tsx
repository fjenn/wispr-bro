// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Onboarding's "Edit shortcut" (S-29a): changing the dictation key in one
 * step, the same capture as Settings' "Dictation key" row (press the keys,
 * then Save). Every other shortcut stays in Settings › Shortcuts.
 */
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { KeyCapture } from "../settings/hotkeys/DictationKey";
import { useForkSettings } from "../settings/useForkSettings";
import { Dialog } from "../ui";

export function DictationKeyDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings } = useForkSettings();
  const current = settings?.hotkeys.dictate?.find(
    (binding) => binding.activation === "hold",
  )?.chord;
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      title={t("settingsModal.hotkeys.key.title")}
      description={t("settingsModal.hotkeys.key.hint")}
      closeLabel={t("settingsModal.close")}
      contentFades={false}
      testId="wb-onboarding-key-dialog"
    >
      {settings && <KeyCapture current={current} onClose={onClose} />}
    </Dialog>
  );
}
