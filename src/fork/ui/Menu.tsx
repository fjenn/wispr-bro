// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Dropdown menu on Radix DropdownMenu (R4 "UI primitives", MIT): a real
 * `menu` with arrow-key navigation, typeahead, Esc to close and focus
 * returned to the trigger. Used for the Help menu (S-13), the notifications
 * menu with its toggles (S-18, `checked` items) and row menus later (S-15b).
 *
 * ```tsx
 * <Menu trigger={<button …/>} label={t("…")} side="right"
 *   groups={[{ label: t("…"), items: [{ id, label, icon, onSelect }] }]} />
 * ```
 */
import * as RadixMenu from "@radix-ui/react-dropdown-menu";
import { Check, type Glyph } from "./icons";
import { usePinnedThemeProps } from "./ThemeScope";
import type { ReactElement } from "react";

export interface MenuItem {
  id: string;
  label: string;
  icon?: Glyph;
  onSelect: () => void;
  disabled?: boolean;
  /** Makes the item a checkbox (`menuitemcheckbox`) in this state (S-18). */
  checked?: boolean;
}

export interface MenuGroup {
  /** Optional heading shown above the group. */
  label?: string;
  items: MenuItem[];
}

export interface MenuProps {
  /** The button that opens the menu (rendered through `asChild`). */
  trigger: ReactElement;
  groups: MenuGroup[];
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
  /** Accessible name of the menu. */
  label: string;
  testId?: string;
  /**
   * Called when the closed menu is about to return focus to its trigger;
   * `event.preventDefault()` keeps focus where an item put it (S-26b: an
   * item that opens a dialog).
   */
  onCloseAutoFocus?: (event: Event) => void;
  /** Controlled open state (OT2-E: Help › Support opens the Support menu). */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function Menu({
  trigger,
  groups,
  side = "bottom",
  align = "start",
  label,
  testId,
  onCloseAutoFocus,
  open,
  onOpenChange,
}: MenuProps) {
  const pinned = usePinnedThemeProps();
  return (
    // Non-modal: the rest of the window stays readable (no aria-hidden on
    // it); Esc, a click outside or Tab closes the menu.
    <RadixMenu.Root modal={false} open={open} onOpenChange={onOpenChange}>
      <RadixMenu.Trigger asChild>{trigger}</RadixMenu.Trigger>
      <RadixMenu.Portal>
        <RadixMenu.Content
          side={side}
          align={align}
          sideOffset={6}
          collisionPadding={8}
          aria-label={label}
          data-testid={testId}
          onCloseAutoFocus={onCloseAutoFocus}
          {...pinned}
          className="wb-pop z-50 min-w-52 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-1 text-(--wb-ink) shadow-(--wb-elev-1)"
        >
          {groups.map((group, index) => (
            <RadixMenu.Group key={group.label ?? index}>
              {index > 0 && (
                <RadixMenu.Separator className="my-1 h-px bg-(--wb-line)" />
              )}
              {group.label && (
                <RadixMenu.Label className="type-micro px-2.5 pt-2 pb-1 text-(--wb-ink-3)">
                  {group.label}
                </RadixMenu.Label>
              )}
              {group.items.map((item) => {
                const Icon = item.icon;
                const itemClass =
                  "flex min-h-8 cursor-pointer items-center gap-2.5 rounded-(--wb-radius-sm) px-2.5 py-1.5 text-[14px] outline-none select-none data-[disabled]:cursor-default data-[disabled]:opacity-50 data-[highlighted]:bg-(--wb-fill-hover)";
                if (item.checked !== undefined) {
                  return (
                    <RadixMenu.CheckboxItem
                      key={item.id}
                      checked={item.checked}
                      disabled={item.disabled}
                      // Toggles keep the menu open, so several can change.
                      onSelect={(event) => {
                        event.preventDefault();
                        item.onSelect();
                      }}
                      data-testid={`wb-menu-item-${item.id}`}
                      className={itemClass}
                    >
                      <span
                        aria-hidden="true"
                        className="grid size-4 place-items-center text-(--wb-ink)"
                      >
                        <RadixMenu.ItemIndicator>
                          <Check size={16} />
                        </RadixMenu.ItemIndicator>
                      </span>
                      {item.label}
                    </RadixMenu.CheckboxItem>
                  );
                }
                return (
                  <RadixMenu.Item
                    key={item.id}
                    disabled={item.disabled}
                    onSelect={item.onSelect}
                    data-testid={`wb-menu-item-${item.id}`}
                    className={itemClass}
                  >
                    {Icon && <Icon size={16} className="text-(--wb-ink-2)" />}
                    {item.label}
                  </RadixMenu.Item>
                );
              })}
            </RadixMenu.Group>
          ))}
        </RadixMenu.Content>
      </RadixMenu.Portal>
    </RadixMenu.Root>
  );
}
