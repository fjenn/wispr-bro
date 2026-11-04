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
  /** Position in the row; picks the sample's sender dot. */
  index?: number;
  onSelect: () => void;
}

/** The sample bubble's sender dot, one per card (decorative). */
const AVATARS = [
  "bg-(--wb-accent-soft)",
  "bg-(--wb-accent-2)",
  "bg-(--wb-accent)",
];

export function PresetCard({
  preset,
  name,
  index = 0,
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
      className={`relative flex min-h-[300px] min-w-0 cursor-pointer flex-col gap-6 rounded-(--wb-radius-sm) border bg-(--wb-surface) p-5 transition-[border-color,outline-color] duration-(--wb-dur-fast) has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-(--wb-accent) ${
        checked
          ? "border-(--wb-accent-2) outline-[1.5px] -outline-offset-2 outline-(--wb-accent-2) outline-solid"
          : "border-(--wb-line) hover:border-(--wb-line-2)"
      }`}
    >
      {/* A real radio, invisible over the whole card: the card is its
          label, and the browser's radio keys and RTL arrows keep working. */}
      <input
        id={id}
        type="radio"
        name={name}
        value={preset}
        checked={checked}
        disabled={disabled}
        onChange={onSelect}
        aria-describedby={`${id}-desc ${id}-example`}
        className="absolute inset-0 m-0 size-full cursor-pointer appearance-none rounded-[inherit] opacity-0 disabled:cursor-default"
      />
      <span className="flex flex-col text-[15px] leading-[22px] text-(--wb-ink)">
        <span className="font-medium">
          {t(`style.presets.${preset}.label`)}
        </span>
        <span id={`${id}-desc`} className="text-(--wb-ink-2)">
          {t(`style.presets.${preset}.description`)}
        </span>
      </span>
      <span className="mt-auto flex flex-col items-end gap-2">
        <span className="sr-only">{t("style.after")}</span>
        <span
          id={`${id}-example`}
          lang="en"
          dir="ltr"
          data-testid={`wb-style-example-${preset}`}
          className="w-full rounded-(--wb-radius-md) bg-(--wb-surface-tint) px-4 py-3 text-start text-[15px] leading-[22px] text-(--wb-ink)"
        >
          {example}
        </span>
        <span
          aria-hidden="true"
          className={`size-9 rounded-full ${AVATARS[index % AVATARS.length]}`}
        />
      </span>
    </label>
  );
}
