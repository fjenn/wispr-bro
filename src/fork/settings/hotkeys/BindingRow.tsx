// SPDX-License-Identifier: GPL-3.0-or-later
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type {
  HotkeyActivation,
  HotkeyBinding,
  HotkeyStatus,
} from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { Button, FOCUS_RING } from "../../ui";
import { Capture } from "./Capture";
import { chordLabel } from "./model";

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
  const activationId = useId();
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
      className="flex flex-col gap-2 rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface) p-3"
      data-testid={`wb-hotkey-${actionId}-${index}`}
    >
      <div className="flex flex-wrap items-center gap-2">
        <kbd
          className="min-w-20 rounded-md border border-(--wb-line-strong) bg-(--wb-panel) px-2 py-1 text-center font-sans text-xs"
          aria-label={
            binding.chord
              ? t("settingsModal.hotkeys.current", {
                  shortcut: label?.spoken ?? binding.chord,
                })
              : t("settingsModal.hotkeys.notSet")
          }
          data-testid={`wb-hotkey-${actionId}-${index}-chord`}
        >
          {capturing ? t("settingsModal.hotkeys.capture.listening") : shown}
        </kbd>
        <label htmlFor={activationId} className="sr-only">
          {t("settingsModal.hotkeys.activationLabel")}
        </label>
        <select
          id={activationId}
          className={`rounded-md border border-(--wb-line-strong) bg-(--wb-surface) px-2 py-1 text-xs ${FOCUS_RING}`}
          value={binding.activation}
          disabled={capturing || captureBusy || activations.length === 1}
          onChange={(event) =>
            onActivation(event.currentTarget.value as HotkeyActivation)
          }
          data-testid={`wb-hotkey-${actionId}-${index}-activation`}
        >
          {activations.map((activation) => (
            <option key={activation} value={activation}>
              {t(`settingsModal.hotkeys.activations.${activation}`)}
            </option>
          ))}
        </select>
        {!capturing && (
          <>
            <Button
              className="px-2.5 py-1"
              disabled={captureBusy}
              onClick={onCapture}
              data-testid={`wb-hotkey-${actionId}-${index}-edit`}
            >
              {t("settingsModal.hotkeys.edit")}
            </Button>
            {onDelete && (
              <Button
                className="px-2.5 py-1"
                disabled={captureBusy}
                onClick={onDelete}
                data-testid={`wb-hotkey-${actionId}-${index}-delete`}
              >
                {t("settingsModal.hotkeys.delete")}
              </Button>
            )}
          </>
        )}
      </div>
      {capturing && <Capture actionId={actionId} onDone={onCaptured} />}
      {issues.map((issue, issueIndex) => (
        <p
          key={`${issue.kind}-${issueIndex}`}
          role="alert"
          className="text-xs text-red-700 dark:text-red-300"
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
