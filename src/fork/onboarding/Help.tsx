// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The header's Help chip and its dialog (S-29a; restyled in Design W1 on
 * the shared `Dialog`: focus trap, Esc, scrim, motion).
 */
import { CircleHelp } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Button, Dialog } from "../ui";

export function HelpChip({
  open,
  onOpen,
  onClose,
}: {
  open: boolean;
  onOpen: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        onClick={onOpen}
        leading={<CircleHelp size={16} aria-hidden="true" />}
      >
        {t("onboardingFlow.help.chip")}
      </Button>
      <Dialog
        open={open}
        onOpenChange={(next) => {
          if (!next) onClose();
        }}
        title={t("onboardingFlow.help.title")}
        closeLabel={t("onboardingFlow.help.close")}
        className="max-w-[440px]!"
        footer={
          <Button variant="primary" onClick={onClose}>
            {t("onboardingFlow.help.done")}
          </Button>
        }
      >
        <div className="flex flex-col gap-3 text-[15px] leading-[22px] text-(--wb-ink-2)">
          <p className="m-0">{t("onboardingFlow.help.body")}</p>
          <p className="m-0">{t("onboardingFlow.help.download")}</p>
        </div>
      </Dialog>
    </>
  );
}
