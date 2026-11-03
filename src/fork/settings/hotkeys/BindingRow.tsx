// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import type {
  HotkeyActivation,
  HotkeyBinding,
  HotkeyStatus,
} from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { IconButton, Select } from "../../ui";
import { Pencil, Trash2 } from "../../ui/icons";
import { Capture } from "./Capture";
import { ChordKeys } from "./ChordKeys";
import { chordLabel } from "./model";

/**
 * One shortcut of an action (S-14; Design W3 after o-0178): a white key
 * field with the keys as caps, an activation choice when the action has
 * more than one, and inline edit and delete glyphs. Capture opens below.
 */

interface BindingRowProps {
  actionId: string;
  binding: HotkeyBinding;
  index: number;
  capturing: boolean;
  captureBusy: boolean;
  status: HotkeyStatus | null;
  activations: HotkeyActivation[];
  onCapture: () => void;
  onCaptured: (chord: string | null) => void;
  onActivation: (activation: HotkeyActivation) => void;
  onDelete?: () => void;
}

export function BindingRow({
  actionId,
  binding,
  index,
  capturing,
  captureBusy,
  status,
  activations,
  onCapture,
  onCaptured,
  onActivation,
  onDelete,
}: BindingRowProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const label = status?.bindings.find(
    (candidate) =>
      candidate.action === actionId && candidate.chord === binding.chord,
  )?.label;
  const issues = (status?.issues ?? []).filter(
    (issue) => issue.action === actionId && issue.chord === binding.chord,
  );
  const shown = binding.chord
    ? (label?.text ?? chordLabel(binding.chord))
    : t("settingsModal.hotkeys.notSet");

  return (
    <div
      className="flex flex-col gap-2"
      data-testid={`wb-hotkey-${actionId}-${index}`}
    >
      <div
        className={`flex h-10 items-center gap-1 rounded-(--wb-radius-sm) border bg-(--wb-surface) ps-1.5 pe-1 transition-colors duration-(--wb-dur-fast) ${capturing ? "border-(--wb-accent) outline-1 -outline-offset-1 outline-(--wb-accent) outline-solid" : "border-(--wb-line)"}`}
      >
        {activations.length > 1 && (
          <Select
            value={binding.activation}
            variant="ghost"
            size="sm"
            fitContent
            label={t("settingsModal.hotkeys.activationLabel")}
            disabled={capturing || captureBusy}
            options={activations.map((activation) => ({
              value: activation,
              label: t(`settingsModal.hotkeys.activations.${activation}`),
            }))}
            onValueChange={(value) => onActivation(value as HotkeyActivation)}
            testId={`wb-hotkey-${actionId}-${index}-activation`}
            className="shrink-0 text-(--wb-ink-2)"
          />
        )}
        <kbd
          className={`flex min-w-0 flex-1 items-center px-1.5 font-(family-name:--wb-font-ui) text-[14px] ${binding.chord && !capturing ? "" : "text-(--wb-ink-2)"}`}
          aria-label={
            binding.chord
              ? t("settingsModal.hotkeys.current", {
                  shortcut: label?.spoken ?? binding.chord,
                })
              : t("settingsModal.hotkeys.notSet")
          }
          data-testid={`wb-hotkey-${actionId}-${index}-chord`}
        >
          {capturing ? (
            t("settingsModal.hotkeys.capture.listening")
          ) : binding.chord ? (
            <ChordKeys text={shown} />
          ) : (
            shown
          )}
        </kbd>
        {!capturing && (
          <>
            <IconButton
              size="sm"
              aria-label={t("settingsModal.hotkeys.edit")}
              title={t("settingsModal.hotkeys.edit")}
              disabled={captureBusy}
              onClick={onCapture}
              data-testid={`wb-hotkey-${actionId}-${index}-edit`}
            >
              <Pencil size={16} />
            </IconButton>
            {onDelete && (
              <IconButton
                size="sm"
                aria-label={t("settingsModal.hotkeys.delete")}
                title={t("settingsModal.hotkeys.delete")}
                disabled={captureBusy}
                onClick={onDelete}
                data-testid={`wb-hotkey-${actionId}-${index}-delete`}
              >
                <Trash2 size={16} />
              </IconButton>
            )}
          </>
        )}
      </div>
      {capturing && <Capture actionId={actionId} onDone={onCaptured} />}
      {issues.map((issue, issueIndex) => (
        <p
          key={`${issue.kind}-${issueIndex}`}
          role="alert"
          className="text-[13px] leading-[18px] text-(--wb-danger)"
          data-testid={`wb-hotkey-${actionId}-${index}-issue`}
        >
          {issue.kind === "conflict"
            ? t("settingsModal.hotkeys.conflict", {
                action: t(`settingsModal.hotkeys.actions.${issue.other}`, {
                  defaultValue: issue.other,
                }),
              })
            : issue.kind === "invalid"
              ? t("settingsModal.hotkeys.invalid")
              : t("settingsModal.hotkeys.unsupported")}
        </p>
      ))}
    </div>
  );
}
