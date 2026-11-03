// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One preset of a style group as a selectable card (S-19a, R2 D-25): its
 * name, what it does, and the group's example as this preset inserts it.
 * The cards of a group are one radio group (arrow keys move, Space picks);
 * the example text is English (`lang="en"`), as styles are.
 */
import { useTranslation } from "react-i18next";
import type { StylePreset } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";

export interface PresetCardProps {
  preset: StylePreset;
  /** The radio group's `name`. */
  name: string;
  checked: boolean;
  disabled?: boolean;
  /** The group's example as this preset inserts it. */
  example: string;
  onSelect: () => void;
}

export function PresetCard({
  preset,
  name,
  checked,
  disabled,
  example,
  onSelect,
}: PresetCardProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const id = `${name}-${preset}`;
  return (
    <label
      htmlFor={id}
      data-testid={`wb-style-preset-${preset}`}
      data-selected={checked}
      className={`flex min-w-0 cursor-pointer flex-col gap-2 rounded-(--wb-radius-md) border bg-(--wb-surface) p-4 transition-colors has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-(--wb-accent) ${
        checked
          ? "border-(--wb-accent) shadow-(--wb-elev-1)"
          : "border-(--wb-line) hover:bg-(--wb-fill-hover)"
      }`}
    >
      <span className="flex items-center gap-2">
        <input
          id={id}
          type="radio"
          name={name}
          value={preset}
          checked={checked}
          disabled={disabled}
          onChange={onSelect}
          aria-describedby={`${id}-desc ${id}-example`}
          className="size-4 accent-(--wb-accent)"
        />
        <span className="text-[14px] font-semibold text-(--wb-ink)">
          {t(`style.presets.${preset}.label`)}
        </span>
      </span>
      <span
        id={`${id}-desc`}
        className="text-[12.5px] leading-snug text-(--wb-ink-2)"
      >
        {t(`style.presets.${preset}.description`)}
      </span>
      <span className="sr-only">{t("style.after")}</span>
      <span
        id={`${id}-example`}
        lang="en"
        dir="ltr"
        data-testid={`wb-style-example-${preset}`}
        className="rounded-[10px] bg-(--wb-panel) px-3 py-2 text-start text-[13px] leading-snug text-(--wb-ink)"
      >
        {example}
      </span>
    </label>
  );
}
