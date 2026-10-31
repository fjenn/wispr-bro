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
import { Check, type LucideIcon } from "lucide-react";
import type { ReactElement } from "react";

export interface MenuItem {
  id: string;
  label: string;
  icon?: LucideIcon;
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
}

export function Menu({
  trigger,
  groups,
  side = "bottom",
  align = "start",
  label,
  testId,
}: MenuProps) {
  return (
    // Non-modal: the rest of the window stays readable (no aria-hidden on
    // it); Esc, a click outside or Tab closes the menu.
    <RadixMenu.Root modal={false}>
      <RadixMenu.Trigger asChild>{trigger}</RadixMenu.Trigger>
      <RadixMenu.Portal>
        <RadixMenu.Content
          side={side}
          align={align}
          sideOffset={6}
          collisionPadding={8}
          aria-label={label}
          data-testid={testId}
          className="z-50 min-w-52 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-1.5 text-(--wb-ink) shadow-(--wb-shadow) motion-safe:animate-drop"
        >
          {groups.map((group, index) => (
            <RadixMenu.Group key={group.label ?? index}>
              {index > 0 && (
                <RadixMenu.Separator className="my-1 h-px bg-(--wb-line)" />
              )}
              {group.label && (
                <RadixMenu.Label className="px-2.5 pt-1.5 pb-1 text-[11.5px] font-semibold text-(--wb-ink-3)">
                  {group.label}
                </RadixMenu.Label>
              )}
              {group.items.map((item) => {
                const Icon = item.icon;
                const itemClass =
                  "flex cursor-pointer items-center gap-2.5 rounded-[8px] px-2.5 py-1.5 text-[13px] outline-none select-none data-[disabled]:cursor-default data-[disabled]:opacity-50 data-[highlighted]:bg-(--wb-hover)";
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
                        className="grid size-[15px] place-items-center text-(--wb-accent)"
                      >
                        <RadixMenu.ItemIndicator>
                          <Check size={15} />
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
                    {Icon && (
                      <Icon
                        size={15}
                        aria-hidden="true"
                        className="text-(--wb-ink-2)"
                      />
                    )}
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
