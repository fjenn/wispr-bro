// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A History row's ⋮ menu (S-15b, C-41): Undo AI edit / Redo AI edit,
 * Retry transcript (at the cleanup setting, or at another level), Save
 * audio as… and Delete transcript, on the shared Radix menu (arrow keys,
 * typeahead, Esc returns focus to ⋮).
 *
 * Save audio is disabled, with the reason as its label, when the
 * recording was removed by retention or never saved; Retry needs the
 * recording too.
 */
import {
  Download,
  MoreHorizontal,
  Redo2,
  RotateCcw,
  Trash2,
  Undo2,
} from "../ui/icons";
import { forwardRef, type ComponentPropsWithoutRef } from "react";
import { useTranslation } from "react-i18next";
import type { CleanupLevel, HistoryRow } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { FOCUS_RING, Menu, type MenuGroup } from "../ui";

export const LEVELS: CleanupLevel[] = ["none", "light", "medium"];

export interface RowMenuProps {
  row: HistoryRow;
  /** The cleanup level setting (Retry's default). */
  level: CleanupLevel;
  /** The audio retention setting in days (null: kept forever). */
  audioDays: number | null;
  /** A retry is queued or running. */
  busy: boolean;
  onToggleVariant: () => void;
  onRetry: (level: CleanupLevel | null) => void;
  onSaveAudio: () => void;
  onDelete: () => void;
}

export const ACTION_BUTTON = `grid size-7 cursor-pointer place-items-center rounded-[7px] text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent ${FOCUS_RING}`;

const MoreButton = forwardRef<
  HTMLButtonElement,
  ComponentPropsWithoutRef<"button"> & { label: string }
>(function MoreButton({ label, ...props }, ref) {
  return (
    <button
      ref={ref}
      type="button"
      aria-label={label}
      title={label}
      className={ACTION_BUTTON}
      {...props}
    >
      <MoreHorizontal size={16} aria-hidden="true" />
    </button>
  );
});

/** Why the row has no recording to play or save, or null. */
export function audioReason(
  row: HistoryRow,
  audioDays: number | null,
  t: (key: string, options?: Record<string, unknown>) => string,
): string | null {
  if (row.audio === "available") return null;
  if (row.audio === "expired") {
    return audioDays !== null
      ? t("history.audio.expired", { count: audioDays })
      : t("history.audio.removed");
  }
  return t("history.audio.missing");
}

export function RowMenu({
  row,
  level,
  audioDays,
  busy,
  onToggleVariant,
  onRetry,
  onSaveAudio,
  onDelete,
}: RowMenuProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const noAudio = audioReason(row, audioDays, t);
  const groups: MenuGroup[] = [];
  if (row.ai_edited) {
    groups.push({
      items: [
        row.variant === "final"
          ? {
              id: "undo-ai",
              label: t("history.menu.undoAi"),
              icon: Undo2,
              onSelect: onToggleVariant,
            }
          : {
              id: "redo-ai",
              label: t("history.menu.redoAi"),
              icon: Redo2,
              onSelect: onToggleVariant,
            },
      ],
    });
  }
  groups.push({
    items: [
      {
        id: "retry",
        label: t("history.menu.retry"),
        icon: RotateCcw,
        disabled: busy || noAudio !== null,
        onSelect: () => onRetry(null),
      },
      ...LEVELS.filter((l) => l !== level).map((l) => ({
        id: `retry-${l}`,
        label: t(`history.menu.retryAt.${l}`),
        disabled: busy || noAudio !== null,
        onSelect: () => onRetry(l),
      })),
    ],
  });
  groups.push({
    items: [
      {
        id: "save-audio",
        label: noAudio ?? t("history.menu.saveAudio"),
        icon: Download,
        disabled: noAudio !== null,
        onSelect: onSaveAudio,
      },
      {
        id: "delete",
        label: t("history.menu.delete"),
        icon: Trash2,
        onSelect: onDelete,
      },
    ],
  });
  return (
    <Menu
      label={t("history.menu.label")}
      testId="wb-history-menu"
      align="end"
      groups={groups}
      trigger={
        <MoreButton
          data-testid="wb-history-more"
          label={t("history.actions.more")}
        />
      }
    />
  );
}
