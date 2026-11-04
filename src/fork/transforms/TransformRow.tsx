// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import { Plus, Trash2, WandSparkles } from "../ui/icons";
import type { TransformEntry } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { FOCUS_RING, KbdCombo } from "../ui";

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
  const description =
    entry.description ||
    t(`transforms.builtin.${entry.builtin_key}.description`, {
      defaultValue: "",
    });
  return (
    <article
      className="relative flex min-h-48 flex-col rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) transition-colors duration-(--wb-dur-fast) hover:border-(--wb-line-2)"
      data-testid={`wb-transform-${entry.key}`}
    >
      <button
        type="button"
        onClick={onOpen}
        className={`flex flex-1 cursor-pointer flex-col items-start gap-5 rounded-(--wb-radius-md) p-6 text-start ${FOCUS_RING}`}
        data-testid={`wb-transform-open-${entry.key}`}
      >
        {hotkey ? (
          <KbdCombo keys={chordLabel(hotkey).split(" ")} />
        ) : (
          <WandSparkles
            size={20}
            aria-hidden="true"
            className="text-(--wb-ink-3)"
          />
        )}
        <span className="flex flex-col gap-1.5">
          <span className="type-card-title text-(--wb-ink)">{entry.name}</span>
          <span className="text-[15px] leading-[22px] text-(--wb-ink-2)">
            {description}
          </span>
        </span>
      </button>
      {entry.builtin_key === null && (
        <button
          type="button"
          onClick={onDelete}
          aria-label={t("transforms.card.delete", { name: entry.name })}
          title={t("transforms.card.delete", { name: entry.name })}
          className={`absolute end-3 top-3 grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink-3) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
          data-testid={`wb-transform-delete-${entry.key}`}
        >
          <Trash2 size={16} aria-hidden="true" />
        </button>
      )}
    </article>
  );
}

export function CreateTransformCard({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex min-h-48 cursor-pointer flex-col items-start gap-5 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-6 text-start transition-colors duration-(--wb-dur-fast) hover:border-(--wb-line-2) ${FOCUS_RING}`}
      data-testid="wb-transform-create-card"
    >
      <span
        aria-hidden="true"
        className="grid size-6 place-items-center rounded-full border border-(--wb-line-2) text-(--wb-ink-2)"
      >
        <Plus size={14} />
      </span>
      <span className="flex flex-col gap-1.5">
        <span className="type-card-title text-(--wb-ink)">
          {t("transforms.card.createOwn")}
        </span>
        <span className="text-[15px] leading-[22px] text-(--wb-ink-2)">
          {t("transforms.card.createOwnHint")}
        </span>
      </span>
    </button>
  );
}
