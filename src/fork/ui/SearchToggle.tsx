// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A list's search as a magnifier button that opens into a field
 * (flowmatch): the tools at the end of a page's tab row stay quiet until
 * the user wants them. The field stays open while it holds a query; Esc
 * on an empty field (or a blur with nothing typed) folds it back and
 * returns focus to the button. `SortMenu` is the sort order beside it.
 */
import { useEffect, useRef, useState } from "react";
import { IconButton } from "./Button";
import { ArrowUpDown, Search } from "./icons";
import { Menu } from "./Menu";
import { SearchField } from "./TextField";

export interface SearchToggleProps {
  value: string;
  onChange: (value: string) => void;
  /** Accessible name (and the placeholder unless `placeholder` is set). */
  label: string;
  placeholder?: string;
  /** Test id of the field; the button gets `${testId}-open`. */
  testId?: string;
}

export function SearchToggle({
  value,
  onChange,
  label,
  placeholder,
  testId,
}: SearchToggleProps) {
  const [open, setOpen] = useState(value !== "");
  const field = useRef<HTMLInputElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const wasOpen = useRef(open);
  // Only Esc gives focus back to the button; a blur leaves it where it went.
  const refocus = useRef(false);

  useEffect(() => {
    if (open && !wasOpen.current) field.current?.focus();
    if (!open && wasOpen.current && refocus.current) button.current?.focus();
    refocus.current = false;
    wasOpen.current = open;
  }, [open]);

  if (!open && value === "") {
    return (
      <IconButton
        ref={button}
        aria-label={label}
        title={label}
        onClick={() => setOpen(true)}
        data-testid={testId ? `${testId}-open` : undefined}
      >
        <Search size={18} aria-hidden="true" />
      </IconButton>
    );
  }
  return (
    <SearchField
      ref={field}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Escape" && value === "") {
          e.preventDefault();
          refocus.current = true;
          setOpen(false);
        }
      }}
      onBlur={() => {
        if (value === "") setOpen(false);
      }}
      aria-label={label}
      placeholder={placeholder ?? label}
      className="w-56"
      data-testid={testId}
    />
  );
}

export interface SortMenuProps<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  /** Accessible name of the button and the menu ("Sort"). */
  label: string;
  /** Test id of the button; its items are `wb-menu-item-${testId}-${value}`. */
  testId: string;
}

/** A list's sort order as an icon button with a checked menu (flowmatch). */
export function SortMenu<T extends string>({
  value,
  options,
  onChange,
  label,
  testId,
}: SortMenuProps<T>) {
  const current = options.find((o) => o.value === value)?.label;
  const name = current ? `${label}: ${current}` : label;
  return (
    <Menu
      label={label}
      align="end"
      testId={`${testId}-menu`}
      groups={[
        {
          items: options.map((o) => ({
            id: `${testId}-${o.value}`,
            label: o.label,
            checked: o.value === value,
            closeOnSelect: true,
            onSelect: () => onChange(o.value),
          })),
        },
      ]}
      trigger={
        <IconButton aria-label={name} title={name} data-testid={testId}>
          <ArrowUpDown size={18} aria-hidden="true" />
        </IconButton>
      }
    />
  );
}
