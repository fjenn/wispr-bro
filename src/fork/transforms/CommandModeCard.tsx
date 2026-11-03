// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { Kbd, KbdCombo, Switch } from "../ui";

export function CommandModeCard({
  checked,
  disabled,
  onChange,
  shortcut,
}: {
  checked: boolean;
  disabled: boolean;
  onChange: (checked: boolean) => void;
  /** The Command Mode binding's chord (`fn+ctrl` by default), if one is set. */
  shortcut?: string;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <section
      aria-labelledby="wb-command-mode-title"
      className="flex items-start gap-4 rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-5 py-4"
      data-testid="wb-command-mode-card"
      data-enabled={checked ? "true" : "false"}
    >
      <span className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="flex flex-wrap items-center gap-2">
          <span
            id="wb-command-mode-title"
            className="text-[15px] leading-5 font-medium text-(--wb-ink)"
          >
            {t("transforms.commandMode.title")}
          </span>
          {shortcut ? (
            <KbdCombo keys={chordLabel(shortcut).split(" ")} />
          ) : (
            <Kbd>{t("settingsModal.hotkeys.notSet")}</Kbd>
          )}
          <span className="sr-only">
            {checked
              ? t("transforms.commandMode.on")
              : t("transforms.commandMode.off")}
          </span>
        </span>
        <span
          id="wb-command-mode-help"
          className="text-[13px] leading-[18px] text-(--wb-ink-2)"
        >
          {t("transforms.commandMode.help")}
        </span>
      </span>
      <Switch
        checked={checked}
        disabled={disabled}
        onChange={onChange}
        ariaLabelledBy="wb-command-mode-title"
        ariaDescribedBy="wb-command-mode-help"
        testId="wb-command-mode-enabled"
        className="mt-0.5"
      />
    </section>
  );
}
