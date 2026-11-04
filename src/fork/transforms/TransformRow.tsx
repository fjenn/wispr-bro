// SPDX-License-Identifier: GPL-3.0-or-later
// One transform in the page's list (XF2): its name, its shortcut and, for
// custom transforms, Delete on hover or focus, like Scratchpad's notes.
import { useTranslation } from "react-i18next";
import { Trash2 } from "../ui/icons";
import type { TransformEntry } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { FOCUS_RING, IconButton, KbdCombo } from "../ui";

export function TransformRow({
  entry,
  hotkey,
  onOpen,
  onDelete,
}: {
  entry: TransformEntry;
  hotkey?: string;
  onOpen: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const custom = entry.builtin_key === null;
  return (
    <li
      className="group/row relative rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-surface-tint)"
      data-testid={`wb-transform-${entry.key}`}
    >
      <button
        type="button"
        onClick={onOpen}
        className={`flex min-h-14 w-full cursor-pointer items-center gap-4 rounded-(--wb-radius-md) px-5 py-3 text-start ${custom ? "pe-14" : ""} ${FOCUS_RING}`}
        data-testid={`wb-transform-open-${entry.key}`}
      >
        <span
          dir="auto"
          className="min-w-0 flex-1 truncate text-[16px] leading-[22px] font-semibold text-(--wb-ink)"
        >
          {entry.name}
        </span>
        {hotkey && <KbdCombo keys={chordLabel(hotkey).split(" ")} />}
      </button>
      {custom && (
        <IconButton
          variant="ghost"
          onClick={onDelete}
          aria-label={t("transforms.card.delete", { name: entry.name })}
          title={t("transforms.card.delete", { name: entry.name })}
          className="absolute end-3 top-1/2 -translate-y-1/2 opacity-0 transition-opacity duration-(--wb-dur-fast) group-focus-within/row:opacity-100 group-hover/row:opacity-100 focus-visible:opacity-100"
          data-testid={`wb-transform-delete-${entry.key}`}
        >
          <Trash2 size={16} aria-hidden="true" />
        </IconButton>
      )}
    </li>
  );
}
