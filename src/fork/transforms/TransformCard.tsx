// SPDX-License-Identifier: GPL-3.0-or-later
// One transform in "My Transforms" (flowmatch, after Wispr Flow's grid): its
// shortcut as key caps, its name and one grey line; the card opens the
// editor. Custom transforms show Delete on hover or focus. `CreateCard` is
// the grid's last card, "Create your own".
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import { Plus, Trash2 } from "../ui/icons";
import type { TransformEntry } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { FOCUS_RING, IconButton, KbdCombo, useMotionPresets } from "../ui";

const CARD =
  "group/card relative min-h-[148px] rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) transition-[border-color,background-color,box-shadow] duration-(--wb-dur-fast) hover:border-(--wb-line-2) hover:shadow-(--wb-elev-1)";
const OPEN = `flex size-full min-h-[inherit] cursor-pointer flex-col items-start gap-3 rounded-(--wb-radius-md) p-5 text-start ${FOCUS_RING}`;

/** The grey line under a transform's name. */
function line(entry: TransformEntry, t: (key: string) => string): string {
  if (entry.builtin_key) return t(`transforms.builtin.${entry.builtin_key}`);
  return (entry.description || entry.prompt).split("\n")[0] ?? "";
}

export function TransformCard({
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
  const m = useMotionPresets();
  const custom = entry.builtin_key === null;
  const sub = line(entry, t);
  return (
    <li className={CARD} data-testid={`wb-transform-${entry.key}`}>
      <motion.button
        type="button"
        {...m.press}
        onClick={onOpen}
        className={`${OPEN} ${custom ? "pe-12" : ""}`}
        data-testid={`wb-transform-open-${entry.key}`}
      >
        <span className="flex h-7 items-center">
          {hotkey && <KbdCombo keys={chordLabel(hotkey).split(" ")} />}
        </span>
        <span
          dir="auto"
          className="type-card-title mt-auto w-full truncate text-(--wb-ink)"
        >
          {entry.name}
        </span>
        <span
          dir="auto"
          className="line-clamp-1 w-full text-[15px] leading-[22px] text-(--wb-ink-2)"
        >
          {sub || " "}
        </span>
      </motion.button>
      {custom && (
        <IconButton
          variant="ghost"
          onClick={onDelete}
          aria-label={t("transforms.card.delete", { name: entry.name })}
          title={t("transforms.card.delete", { name: entry.name })}
          className="absolute end-3 top-3 opacity-0 transition-opacity duration-(--wb-dur-fast) group-focus-within/card:opacity-100 group-hover/card:opacity-100 focus-visible:opacity-100"
          data-testid={`wb-transform-delete-${entry.key}`}
        >
          <Trash2 size={16} aria-hidden="true" />
        </IconButton>
      )}
    </li>
  );
}

export function CreateCard({ onCreate }: { onCreate: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const m = useMotionPresets();
  return (
    <li className={CARD}>
      <motion.button
        type="button"
        {...m.press}
        onClick={onCreate}
        className={OPEN}
        data-testid="wb-transforms-create-card"
      >
        <span
          aria-hidden="true"
          className="grid size-7 place-items-center rounded-full bg-(--wb-surface-tint) text-(--wb-ink-2) transition-colors duration-(--wb-dur-fast) group-hover/card:bg-(--wb-ink) group-hover/card:text-(--wb-surface)"
        >
          <Plus size={16} />
        </span>
        <span className="type-card-title mt-auto text-(--wb-ink)">
          {t("transforms.card.createOwn")}
        </span>
        <span className="text-[15px] leading-[22px] text-(--wb-ink-2)">
          {t("transforms.card.createOwnHint")}
        </span>
      </motion.button>
    </li>
  );
}
