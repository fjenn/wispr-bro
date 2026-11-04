// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One-of-many choices (Design Phase 0b; REFERENCE-SPEC §4 "Option card").
 *
 * - `RadioGroup`: the named `radiogroup` that owns the value. Children are
 *   `Radio` rows or `OptionCard`s; they read the group's name and value from
 *   context. Native radio inputs underneath, so the browser gives the
 *   standard keyboard model: Tab enters on the chosen item, arrow keys move
 *   and select (mirrored in RTL), Space selects.
 * - `Radio`: an 18 pt circle (ink dot when chosen) with a label and an
 *   optional one-line description. For short lists in dialogs and settings.
 * - `OptionCard`: the large choice card of onboarding questions and lessons:
 *   title and one line, an optional leading glyph or a `media` tile on top
 *   (image tiles). Rest = white with a 3:1 edge; selected = lilac fill and
 *   the 1.5 pt purple border (as `Card selected`), plus a filled tick.
 */
import { createContext, useContext, useId, type ReactNode } from "react";
import { Check } from "./icons";

interface GroupContext {
  name: string;
  value: string | undefined;
  onValueChange: (value: string) => void;
  disabled: boolean;
}

const RadioContext = createContext<GroupContext | null>(null);

function useGroup(component: string): GroupContext {
  const ctx = useContext(RadioContext);
  if (!ctx) throw new Error(`${component} must be inside a RadioGroup`);
  return ctx;
}

export interface RadioGroupProps {
  value: string | undefined;
  onValueChange: (value: string) => void;
  /** Accessible name ("Microphone"); or `labelledBy`. */
  label?: string;
  labelledBy?: string;
  describedBy?: string;
  /** Form name; generated when omitted. */
  name?: string;
  disabled?: boolean;
  /** `list` stacks rows/cards; `grid` lays cards out 2 per row (`columns`). */
  layout?: "list" | "grid";
  columns?: 2 | 3 | 4;
  className?: string;
  testId?: string;
  children: ReactNode;
}

const GRID_COLS = {
  2: "grid-cols-2",
  3: "grid-cols-3",
  4: "grid-cols-4",
} as const;

export function RadioGroup({
  value,
  onValueChange,
  label,
  labelledBy,
  describedBy,
  name,
  disabled = false,
  layout = "list",
  columns = 2,
  className = "",
  testId,
  children,
}: RadioGroupProps) {
  const fallback = useId();
  const layoutClass =
    layout === "grid"
      ? `grid gap-4 ${GRID_COLS[columns]}`
      : "flex flex-col gap-3";
  return (
    <RadioContext.Provider
      value={{ name: name ?? fallback, value, onValueChange, disabled }}
    >
      <div
        role="radiogroup"
        aria-label={label}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        aria-disabled={disabled || undefined}
        data-testid={testId}
        className={`${layoutClass} ${className}`}
      >
        {children}
      </div>
    </RadioContext.Provider>
  );
}

interface ItemProps {
  value: string;
  disabled?: boolean;
  testId?: string;
  className?: string;
}

/** The hidden-but-focusable native input shared by both item kinds. */
function NativeRadio({
  ctx,
  value,
  disabled,
  testId,
  describedBy,
  className,
}: {
  ctx: GroupContext;
  value: string;
  disabled: boolean;
  testId?: string;
  describedBy?: string;
  className: string;
}) {
  return (
    <input
      type="radio"
      name={ctx.name}
      value={value}
      checked={ctx.value === value}
      disabled={disabled}
      data-testid={testId}
      aria-describedby={describedBy}
      onChange={() => ctx.onValueChange(value)}
      className={className}
    />
  );
}

export interface RadioProps extends ItemProps {
  label: ReactNode;
  description?: ReactNode;
}

export function Radio({
  value,
  label,
  description,
  disabled = false,
  testId,
  className = "",
}: RadioProps) {
  const ctx = useGroup("Radio");
  const descId = useId();
  const off = disabled || ctx.disabled;
  return (
    <label
      className={`group/radio flex items-start gap-3 text-(--wb-ink) ${off ? "cursor-default opacity-50" : "cursor-pointer"} ${className}`}
    >
      <span className="relative mt-px inline-grid size-[18px] shrink-0 place-items-center">
        <NativeRadio
          ctx={ctx}
          value={value}
          disabled={off}
          testId={testId}
          describedBy={description ? descId : undefined}
          className="peer absolute inset-0 m-0 cursor-[inherit] appearance-none rounded-full border border-(--wb-line-strong) bg-(--wb-surface) transition-colors duration-(--wb-dur-fast) checked:border-(--wb-ink) group-hover/radio:border-(--wb-ink-3) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--wb-accent)"
        />
        <span className="pointer-events-none relative size-2 scale-50 rounded-full bg-(--wb-ink) opacity-0 transition-[opacity,transform] duration-(--wb-dur-fast) ease-(--wb-ease-out) peer-checked:scale-100 peer-checked:opacity-100" />
      </span>
      <span className="flex min-w-0 flex-col gap-0.5">
        <span className="text-[15px] leading-5">{label}</span>
        {description && (
          <span
            id={descId}
            className="text-[13px] leading-[18px] text-(--wb-ink-2)"
          >
            {description}
          </span>
        )}
      </span>
    </label>
  );
}

export interface OptionCardProps extends ItemProps {
  title: ReactNode;
  description?: ReactNode;
  /** A glyph at the start of the card (from `./icons`). */
  icon?: ReactNode;
  /** An image tile above the title (quiz grids); fills the card's width. */
  media?: ReactNode;
}

/** The option card's frame: rest, selected, disabled (shared with `CheckCard`). */
function cardClass(selected: boolean, media: boolean, off: boolean): string {
  return `relative flex rounded-(--wb-radius-md) border p-[22px] text-(--wb-ink) transition-[background-color,border-color,box-shadow] duration-(--wb-dur-fast) has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-(--wb-accent) ${
    media ? "flex-col gap-3" : "items-start gap-4"
  } ${
    selected
      ? "border-(--wb-accent-2) bg-(--wb-accent-soft) shadow-[inset_0_0_0_0.5px_var(--wb-accent-2)]"
      : "border-(--wb-line-strong) bg-(--wb-surface) hover:bg-(--wb-surface-tint)"
  } ${off ? "cursor-default opacity-50" : "cursor-pointer"}`;
}

/** Media tile, glyph, title, line and the tick of an option/check card. */
function CardBody({
  title,
  description,
  descId,
  icon,
  media,
  selected,
  square = false,
}: {
  title: ReactNode;
  description?: ReactNode;
  descId: string;
  icon?: ReactNode;
  media?: ReactNode;
  selected: boolean;
  /** A square tick (several may be chosen) instead of a round one. */
  square?: boolean;
}) {
  return (
    <>
      {media && (
        <span
          aria-hidden="true"
          className="grid aspect-[4/3] w-full place-items-center overflow-hidden rounded-(--wb-radius-sm) bg-(--wb-surface-tint)"
        >
          {media}
        </span>
      )}
      {icon && !media && (
        <span aria-hidden="true" className="mt-0.5 shrink-0 text-(--wb-ink-2)">
          {icon}
        </span>
      )}
      <span className="flex min-w-0 flex-1 flex-col gap-1 pe-6">
        <span className="type-card-title">{title}</span>
        {description && (
          <span id={descId} className="text-[15px] leading-5 text-(--wb-ink-2)">
            {description}
          </span>
        )}
      </span>
      <span
        aria-hidden="true"
        className={`absolute end-4 top-4 grid size-5 place-items-center bg-(--wb-accent) text-(--wb-on-ink) transition-opacity duration-(--wb-dur-fast) ${square ? "rounded-(--wb-radius-xs)" : "rounded-full"} ${selected ? "opacity-100" : "opacity-0"}`}
      >
        <Check size={14} />
      </span>
    </>
  );
}

export function OptionCard({
  value,
  title,
  description,
  icon,
  media,
  disabled = false,
  testId,
  className = "",
}: OptionCardProps) {
  const ctx = useGroup("OptionCard");
  const descId = useId();
  const off = disabled || ctx.disabled;
  const selected = ctx.value === value;
  return (
    <label
      data-selected={selected || undefined}
      className={`${cardClass(selected, !!media, off)} ${className}`}
    >
      <NativeRadio
        ctx={ctx}
        value={value}
        disabled={off}
        testId={testId}
        describedBy={description ? descId : undefined}
        className="sr-only"
      />
      <CardBody
        title={title}
        description={description}
        descId={descId}
        icon={icon}
        media={media}
        selected={selected}
      />
    </label>
  );
}

export interface CheckCardProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  title: ReactNode;
  description?: ReactNode;
  icon?: ReactNode;
  media?: ReactNode;
  disabled?: boolean;
  /** On the native checkbox. */
  testId?: string;
  className?: string;
}

/**
 * `OptionCard`'s look for picking several (Design W5: "Which microphones do
 * you have?"): a native checkbox underneath (Space toggles), a square tick.
 * Put a set of them in a named `group` (e.g. a `fieldset` with a legend).
 */
export function CheckCard({
  checked,
  onCheckedChange,
  title,
  description,
  icon,
  media,
  disabled = false,
  testId,
  className = "",
}: CheckCardProps) {
  const descId = useId();
  return (
    <label
      data-selected={checked || undefined}
      className={`${cardClass(checked, !!media, disabled)} ${className}`}
    >
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        data-testid={testId}
        aria-describedby={description ? descId : undefined}
        onChange={(event) => onCheckedChange(event.target.checked)}
        className="sr-only"
      />
      <CardBody
        title={title}
        description={description}
        descId={descId}
        icon={icon}
        media={media}
        selected={checked}
        square
      />
    </label>
  );
}
