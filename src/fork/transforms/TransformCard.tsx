// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import { Plus, Trash2, WandSparkles } from "../ui/icons";
import type { TransformEntry } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { FOCUS_RING } from "../ui";

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
      className="relative flex min-h-36 flex-col rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-4"
      data-testid={`wb-transform-${entry.key}`}
    >
      <button
        type="button"
        onClick={onOpen}
        className={`flex flex-1 cursor-pointer flex-col items-start gap-2 rounded-md text-start ${FOCUS_RING}`}
        data-testid={`wb-transform-open-${entry.key}`}
      >
        <span className="flex w-full items-center gap-2">
          <WandSparkles
            size={18}
            aria-hidden="true"
            className="text-(--wb-accent)"
          />
          <span className="font-semibold">{entry.name}</span>
          {hotkey && (
            <kbd className="ms-auto rounded border border-(--wb-line) px-1.5 py-0.5 text-xs text-(--wb-ink-2)">
              {chordLabel(hotkey)}
            </kbd>
          )}
        </span>
        <span className="text-sm leading-5 text-(--wb-ink-2)">
          {description}
        </span>
      </button>
      {entry.builtin_key === null && (
        <button
          type="button"
          onClick={onDelete}
          aria-label={t("transforms.card.delete", { name: entry.name })}
          title={t("transforms.card.delete", { name: entry.name })}
          className={`absolute bottom-3 end-3 grid size-7 cursor-pointer place-items-center rounded-md text-(--wb-ink-2) hover:bg-(--wb-fill-hover) ${FOCUS_RING}`}
          data-testid={`wb-transform-delete-${entry.key}`}
        >
          <Trash2 size={14} aria-hidden="true" />
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
      className={`flex min-h-36 cursor-pointer flex-col items-center justify-center gap-2 rounded-(--wb-radius-md) border border-dashed border-(--wb-line-strong) bg-transparent p-4 text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
      data-testid="wb-transform-create-card"
    >
      <Plus size={20} aria-hidden="true" />
      <span className="font-semibold">{t("transforms.card.createOwn")}</span>
      <span className="text-sm">{t("transforms.card.createOwnHint")}</span>
    </button>
  );
}
