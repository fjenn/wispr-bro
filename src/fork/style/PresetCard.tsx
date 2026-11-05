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
  return (
    <ChoiceCard
      value={preset}
      name={name}
      checked={checked}
      disabled={disabled}
      onSelect={onSelect}
      testId={`wb-style-preset-${preset}`}
      exampleTestId={`wb-style-example-${preset}`}
      title={t(`style.presets.${preset}.label`)}
      description={t(`style.presets.${preset}.description`)}
      example={example}
      exampleLabel={t("style.after")}
      avatar={AVATARS[index % AVATARS.length]}
    />
  );
}

export interface ChoiceCardProps {
  value: string;
  /** The radio group's `name`. */
  name: string;
  checked: boolean;
  disabled?: boolean;
  onSelect: () => void;
  testId: string;
  exampleTestId: string;
  title: string;
  description: string;
  /** English sample text, shown in a tinted box at the bottom. */
  example: string;
  /** Read before the example by screen readers. */
  exampleLabel: string;
  /** A chat bubble with this sender dot; without it, a serif title and an
   *  italic sample (Style › Cleanup). */
  avatar?: string;
}

/**
 * One choice of a Style tab as a card (flowmatch): the cards of a tab are
 * one radio group; the selected card has an accent outline.
 */
export function ChoiceCard({
  value,
  name,
  checked,
  disabled,
  onSelect,
  testId,
  exampleTestId,
  title,
  description,
  example,
  exampleLabel,
  avatar,
}: ChoiceCardProps) {
  const id = `${name}-${value}`;
  const chat = avatar !== undefined;
  return (
    <label
      htmlFor={id}
      data-testid={testId}
      data-selected={checked}
      className={`relative flex min-h-[300px] min-w-0 cursor-pointer flex-col gap-6 rounded-(--wb-radius-md) border bg-(--wb-surface) p-5 transition-[border-color,outline-color,background-color] duration-(--wb-dur-fast) has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-(--wb-accent) ${
        checked
          ? chat
            ? "border-(--wb-accent-2) outline-[1.5px] -outline-offset-2 outline-(--wb-accent-2) outline-solid"
            : "border-(--wb-ink) outline-[1px] -outline-offset-2 outline-(--wb-ink) outline-solid"
          : "border-(--wb-line) hover:border-(--wb-line-2) hover:bg-(--wb-surface-tint)/40"
      }`}
    >
      {/* A real radio, invisible over the whole card: the card is its
          label, and the browser's radio keys and RTL arrows keep working. */}
      <input
        id={id}
        type="radio"
        name={name}
        value={value}
        checked={checked}
        disabled={disabled}
        onChange={onSelect}
        aria-describedby={`${id}-desc ${id}-example`}
        className="absolute inset-0 m-0 size-full cursor-pointer appearance-none rounded-[inherit] opacity-0 disabled:cursor-default"
      />
      <span className="flex flex-col gap-2 text-[15px] leading-[22px] text-(--wb-ink)">
        <span
          className={
            chat
              ? "font-medium"
              : "type-serif-title text-[34px]! leading-[40px]!"
          }
        >
          {title}
        </span>
        <span id={`${id}-desc`} className="text-(--wb-ink-2)">
          {description}
        </span>
      </span>
      <span className="mt-auto flex flex-col items-end gap-2">
        <span className="sr-only">{exampleLabel}</span>
        <span
          id={`${id}-example`}
          lang="en"
          dir="ltr"
          data-testid={exampleTestId}
          className={`w-full rounded-(--wb-radius-md) bg-(--wb-accent-soft) px-4 py-3 text-start text-[15px] leading-[22px] text-(--wb-ink) ${chat ? "" : "italic"}`}
        >
          {example}
        </span>
        {chat && (
          <span
            aria-hidden="true"
            className={`size-9 rounded-full ${avatar}`}
          />
        )}
      </span>
    </label>
  );
}
