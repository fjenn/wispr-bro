// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The setup checklist's item registry (S-30, B-44). Each feature registers
 * its own item; the card (`SetupChecklist.tsx`) shows whatever is
 * registered, in `order`, without knowing the items. S-30 registers four
 * (`checklistItems.ts`): Try it in another app, Add a word, Create a
 * snippet, Open at login. S-31 adds "Try a Transform" and S-32 "Try
 * whispering" by calling `registerChecklistItem` from their own modules.
 *
 * Ticks are persisted by id in fork settings
 * (`onboarding.checklist_done`, through `fork_checklist_mark`), so an item
 * stays ticked once done. An item ticks itself in one of two ways:
 * - `isDone`: a check of the app's state (a word exists), run when the card
 *   mounts, when the window gets focus and after each of the item's
 *   `refreshOn` events; when it answers true the id is persisted;
 * - the backend persists the id itself (`try_in_app`, ticked by the first
 *   take pasted into another app, `fork/onboarding/checklist.rs`).
 *
 * Chaining (R1 §11): when the item the user started from the card ticks,
 * the next open item is *shown*, never started: its `reveal` runs (open its
 * page, point at it with a coach mark) or, without one, the card points at
 * its row. `run` is for the user's own click only, because a `run` may have
 * a side effect (Open at login's turns autostart on; S-32 decision).
 */
import { useSyncExternalStore } from "react";
import type { LucideIcon } from "lucide-react";

/** What an item's `run` can use. */
export interface ChecklistRunContext {
  /** Opens a Hub page (the deep-link API, `nav.tsx`). */
  navigate: (page: string, section?: string) => void;
  /** Points at an element on the page with a coach mark (driver.js). */
  coach: (target: string, titleKey: string, bodyKey: string) => void;
  /** Reopens the first-use chooser. */
  openChooser: () => void;
  /** Marks the item done (for items whose action is the whole step). */
  markDone: () => Promise<void>;
}

export interface ChecklistItem {
  /** Stable id stored in `onboarding.checklist_done` (`a-z0-9_.-`). */
  id: string;
  /**
   * Position in the card (S-30 uses 10, 20, 30, 40; later items fit in:
   * S-31's Try a Transform 35, S-32's Try whispering 37).
   */
  order: number;
  icon: LucideIcon;
  /** `fork` namespace keys. */
  titleKey: string;
  actionKey: string;
  bodyKey?: string;
  /** A check of the app's state; true ticks the item for good. */
  isDone?: () => Promise<boolean>;
  /** Event names after which `isDone` runs again. */
  refreshOn?: string[];
  /** The item's step, from its button only (an explicit click). */
  run: (ctx: ChecklistRunContext) => void | Promise<void>;
  /**
   * Shows the item when chaining reaches it: navigate to its page, point at
   * it. Must never change a setting or start anything (no `markDone`).
   * Without one, chaining points at the item's row in the card.
   */
  reveal?: (ctx: ChecklistRevealContext) => void | Promise<void>;
}

/** What `reveal` can use: showing things only. */
export type ChecklistRevealContext = Pick<
  ChecklistRunContext,
  "navigate" | "coach"
>;

let items: readonly ChecklistItem[] = [];
const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of listeners) listener();
}

/**
 * Adds (or replaces, by id) an item. Returns a function that removes it.
 */
export function registerChecklistItem(item: ChecklistItem): () => void {
  items = [...items.filter((i) => i.id !== item.id), item].sort(
    (a, b) => a.order - b.order || a.id.localeCompare(b.id),
  );
  emit();
  return () => {
    const before = items.length;
    items = items.filter((i) => i !== item);
    if (items.length !== before) emit();
  };
}

export function checklistItems(): readonly ChecklistItem[] {
  return items;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The registered items, in order (re-renders when one is added). */
export function useChecklistItems(): readonly ChecklistItem[] {
  return useSyncExternalStore(subscribe, checklistItems, checklistItems);
}

/** The first item not done, in order. */
export function nextOpenItem(
  list: readonly ChecklistItem[],
  done: readonly string[],
): ChecklistItem | undefined {
  return list.find((item) => !done.includes(item.id));
}

/** Done and total, counting only registered items. */
export function checklistProgress(
  list: readonly ChecklistItem[],
  done: readonly string[],
): { done: number; total: number } {
  return {
    done: list.filter((item) => done.includes(item.id)).length,
    total: list.length,
  };
}
