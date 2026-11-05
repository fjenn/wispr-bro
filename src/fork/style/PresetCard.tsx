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
  /** Position in the row; picks the sample's sender disc. */
  index?: number;
  /** The letter on the sender disc (the user's initial). */
  initial?: string;
  onSelect: () => void;
}

/** The sample bubble's sender disc, one per card (decorative). */
const AVATARS = [
  "bg-(--wb-avatar-1)",
  "bg-(--wb-avatar-2)",
  "bg-(--wb-avatar-3)",
];

export function PresetCard({
  preset,
  name,
  index = 0,
  checked,
  disabled,
  example,
  initial,
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
      initial={initial}
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
  /** A short line under the description (serif cards only), read with it. */
  note?: string;
  /** English sample text, shown in a tinted box at the bottom; its line
   *  breaks show (an email's greeting and sign-off). */
  example: string;
  /** Read before the example by screen readers. */
  exampleLabel: string;
  /** A chat bubble with this sender disc; without it, a serif title and
   *  an italic sample (Style › Cleanup). */
  avatar?: string;
  /** The letter on the sender disc. */
  initial?: string;
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
  note,
  example,
  exampleLabel,
  avatar,
  initial,
}: ChoiceCardProps) {
  const id = `${name}-${value}`;
  const chat = avatar !== undefined;
  return (
    <label
      htmlFor={id}
      data-testid={testId}
      data-selected={checked}
      className={`relative flex min-w-0 cursor-pointer flex-col rounded-(--wb-radius-md) border bg-(--wb-surface) transition-[border-color,box-shadow] duration-(--wb-dur-fast) has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-(--wb-accent) ${
        chat ? "min-h-[350px] p-[18px]" : "min-h-[360px] p-[13px] pt-4"
      } ${
        checked
          ? chat
            ? "border-(--wb-accent-2) shadow-[inset_0_0_0_1px_var(--wb-accent-2)]"
            : "border-(--wb-ink) shadow-[inset_0_0_0_0.5px_var(--wb-ink)]"
          : "border-(--wb-line) hover:border-(--wb-line-2)"
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
      {chat ? (
        <span className="flex flex-col gap-2 text-[16px] leading-[24px] text-(--wb-ink)">
          <span>{title}</span>
          <span id={`${id}-desc`}>{description}</span>
        </span>
      ) : (
        <span className="flex flex-col gap-3 text-(--wb-ink)">
          <span className="type-serif-title text-[32px]! leading-[38px]! tracking-[-0.02em]">
            {title}
          </span>
          <span
            id={`${id}-desc`}
            className="text-[16px] leading-[22px] text-(--wb-ink-3)"
          >
            {description}
            {note && (
              <span className="mt-2 block text-[13px] leading-[18px]">
                {note}
              </span>
            )}
          </span>
        </span>
      )}
      <span className={`flex flex-col items-end ${chat ? "mt-14" : "mt-auto"}`}>
        <span className="sr-only">{exampleLabel}</span>
        <span
          id={`${id}-example`}
          lang="en"
          dir="ltr"
          data-testid={exampleTestId}
          className={`w-full rounded-(--wb-radius-md) bg-(--wb-accent-soft) text-start whitespace-pre-line text-[16px] leading-[24px] text-(--wb-ink) ${
            chat
              ? "rounded-ee-none px-[14px] py-[18px]"
              : "px-4 py-3 leading-[20px]! italic [font-synthesis:style]"
          }`}
        >
          {example}
        </span>
        {chat && (
          <span
            aria-hidden="true"
            className={`mt-3 grid size-[52px] place-items-center rounded-full text-[22px] font-medium text-(--wb-on-ink) ${avatar}`}
          >
            {initial}
          </span>
        )}
      </span>
    </label>
  );
}
