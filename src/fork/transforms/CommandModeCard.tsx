// SPDX-License-Identifier: GPL-3.0-or-later
import { MessageSquareText } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { FOCUS_RING } from "../ui";

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
      className="rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-5"
      data-testid="wb-command-mode-card"
      data-enabled={checked ? "true" : "false"}
    >
      <label className="flex cursor-pointer items-start gap-3">
        <input
          type="checkbox"
          role="switch"
          checked={checked}
          disabled={disabled}
          onChange={(event) => onChange(event.currentTarget.checked)}
          className={`mt-1 size-4 accent-(--wb-accent-fill) ${FOCUS_RING}`}
          aria-describedby="wb-command-mode-help"
          data-testid="wb-command-mode-enabled"
        />
        <MessageSquareText
          size={18}
          aria-hidden="true"
          className="mt-0.5 shrink-0 text-(--wb-accent)"
        />
        <span className="min-w-0 flex-1">
          <span className="flex flex-wrap items-center gap-2">
            <span id="wb-command-mode-title" className="font-semibold">
              {t("transforms.commandMode.title")}
            </span>
            <kbd className="rounded border border-(--wb-line) px-1.5 py-0.5 text-xs text-(--wb-ink-2)">
              {shortcut
                ? chordLabel(shortcut)
                : t("settingsModal.hotkeys.notSet")}
            </kbd>
            <span className="text-xs text-(--wb-ink-2)">
              {checked
                ? t("transforms.commandMode.on")
                : t("transforms.commandMode.off")}
            </span>
          </span>
          <span
            id="wb-command-mode-help"
            className="mt-1 block text-sm leading-5 text-(--wb-ink-2)"
          >
            {t("transforms.commandMode.help")}
          </span>
        </span>
      </label>
    </section>
  );
}
