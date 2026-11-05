// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Select (Design Phase 0) on Radix Select (MIT, R4 "UI primitives"): a
 * `combobox` button and a `listbox` popup with arrow keys, Home/End,
 * typeahead, Esc to close, focus returned to the trigger, and the selected
 * option ticked. Replaces native `<select>`s, which look like web forms in
 * the app's WebKit view. Styled like a field (or a ghost button with
 * `variant="ghost"`); the list is a popover on `--wb-elev-1`.
 *
 * ```tsx
 * <Select value={v} onValueChange={setV} label={t("…")}
 *   options={[{ value: "a", label: t("…"), description: t("…") }]} />
 * ```
 */
import * as RadixSelect from "@radix-ui/react-select";
import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { FOCUS_RING } from "./Button";
import { Check, ChevronDown } from "./icons";
import { usePinnedThemeProps } from "./ThemeScope";

export interface SelectOption {
  value: string;
  label: string;
  /** A second, quieter line in the list (not in the trigger). */
  description?: string;
  disabled?: boolean;
  /** A glyph before the label. */
  icon?: ReactNode;
}

export interface SelectGroup {
  label?: string;
  options: SelectOption[];
}

export interface SelectProps {
  value: string | undefined;
  onValueChange: (value: string) => void;
  /** Flat options, or `groups` for labelled sections. */
  options?: SelectOption[];
  groups?: SelectGroup[];
  /** Accessible name (when no visible <label> points at `id`). */
  label?: string;
  /** Id of a visible label. */
  labelledBy?: string;
  /** Id of a visible description (a setting row's hint). */
  describedBy?: string;
  id?: string;
  placeholder?: string;
  disabled?: boolean;
  size?: "sm" | "md" | "lg";
  variant?: "field" | "ghost";
  testId?: string;
  className?: string;
  /** Popup width follows the trigger (default) or its content. */
  fitContent?: boolean;
  /**
   * Where focus goes when the list closes (Radix returns it to the
   * trigger). Call `event.preventDefault()` and focus another element.
   */
  onCloseAutoFocus?: (event: Event) => void;
}

const HEIGHTS = {
  sm: "h-8 px-2.5 text-[13px]",
  md: "h-9 px-3 text-[14px]",
  lg: "h-10 px-3 text-[15px]",
};

export function Select({
  value,
  onValueChange,
  options,
  groups,
  label,
  labelledBy,
  describedBy,
  id,
  placeholder,
  disabled,
  size = "md",
  variant = "field",
  testId,
  className = "",
  fitContent = false,
  onCloseAutoFocus,
}: SelectProps) {
  const { i18n } = useTranslation();
  const pinned = usePinnedThemeProps();
  const sections: SelectGroup[] = groups ?? [{ options: options ?? [] }];
  // Inside a native modal <dialog> (Settings) everything outside the dialog
  // is inert, so the list portals into the dialog instead of <body>.
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [container, setContainer] = useState<HTMLElement | null>(null);
  useLayoutEffect(() => {
    setContainer(triggerRef.current?.closest("dialog") ?? null);
  }, []);
  const trigger =
    variant === "field"
      ? "border border-(--wb-line-strong) bg-(--wb-surface) hover:border-(--wb-ink-3)"
      : "bg-transparent hover:bg-(--wb-fill-hover)";
  return (
    <RadixSelect.Root
      dir={getLanguageDirection(i18n.language)}
      value={value}
      onValueChange={onValueChange}
      disabled={disabled}
    >
      <RadixSelect.Trigger
        ref={triggerRef}
        id={id}
        aria-label={label}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        data-testid={testId}
        className={`inline-flex min-w-0 cursor-pointer items-center justify-between gap-2 rounded-(--wb-radius-sm) text-(--wb-ink) transition-colors duration-(--wb-dur-fast) disabled:cursor-default disabled:opacity-60 data-[placeholder]:text-(--wb-ink-3) ${HEIGHTS[size]} ${trigger} ${FOCUS_RING} ${className}`}
      >
        <span className="min-w-0 truncate">
          <RadixSelect.Value placeholder={placeholder} />
        </span>
        <RadixSelect.Icon className="shrink-0 text-(--wb-ink-3)">
          <ChevronDown size={16} />
        </RadixSelect.Icon>
      </RadixSelect.Trigger>
      <RadixSelect.Portal container={container ?? undefined}>
        <RadixSelect.Content
          {...pinned}
          position="popper"
          onCloseAutoFocus={onCloseAutoFocus}
          sideOffset={6}
          collisionPadding={8}
          className={`wb-pop z-50 max-h-(--radix-select-content-available-height) overflow-hidden rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) text-(--wb-ink) shadow-(--wb-elev-1) ${fitContent ? "min-w-(--radix-select-trigger-width)" : "w-(--radix-select-trigger-width) min-w-44"}`}
        >
          <RadixSelect.Viewport className="p-1">
            {sections.map((section, index) => (
              <RadixSelect.Group key={section.label ?? index}>
                {index > 0 && (
                  <RadixSelect.Separator className="my-1 h-px bg-(--wb-line)" />
                )}
                {section.label && (
                  <RadixSelect.Label className="type-micro px-2.5 pt-2 pb-1 text-(--wb-ink-3)">
                    {section.label}
                  </RadixSelect.Label>
                )}
                {section.options.map((option) => (
                  <RadixSelect.Item
                    key={option.value}
                    value={option.value}
                    disabled={option.disabled}
                    data-testid={
                      testId ? `${testId}-option-${option.value}` : undefined
                    }
                    className="relative flex cursor-pointer items-start gap-2.5 rounded-(--wb-radius-sm) py-2 ps-2.5 pe-8 text-[14px] outline-none select-none data-[disabled]:cursor-default data-[disabled]:opacity-50 data-[highlighted]:bg-(--wb-fill-hover)"
                  >
                    {option.icon && (
                      <span className="mt-0.5 shrink-0 text-(--wb-ink-2)">
                        {option.icon}
                      </span>
                    )}
                    <span className="flex min-w-0 flex-col">
                      <RadixSelect.ItemText>
                        {option.label}
                      </RadixSelect.ItemText>
                      {option.description && (
                        <span className="text-[13px] leading-[18px] text-(--wb-ink-2)">
                          {option.description}
                        </span>
                      )}
                    </span>
                    <RadixSelect.ItemIndicator className="absolute end-2.5 top-2.5 text-(--wb-ink)">
                      <Check size={16} />
                    </RadixSelect.ItemIndicator>
                  </RadixSelect.Item>
                ))}
              </RadixSelect.Group>
            ))}
          </RadixSelect.Viewport>
        </RadixSelect.Content>
      </RadixSelect.Portal>
    </RadixSelect.Root>
  );
}
