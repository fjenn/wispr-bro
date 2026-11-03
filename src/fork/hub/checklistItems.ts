// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The four setup-checklist items S-30 registers (B-44, P-3). Later slices
 * register their own items the same way from their own modules (S-31 "Try
 * a Transform", S-32 "Try whispering"); nothing here changes for them.
 *
 * - Try it in another app: ticked by the backend (the first take pasted
 *   outside Wispr Bro, `fork/onboarding/checklist.rs`); "Show me" reopens
 *   the first-use chooser.
 * - Add a word / Create a snippet: done once the Dictionary or Snippets has
 *   one; the button opens the page and points at its Add button.
 * - Open at login: Handy's `autostart_enabled`; "Turn on" sets it, on the
 *   user's click only: it has no `reveal`, so chaining points at its row
 *   and never turns launch at login on by itself (S-32).
 */
import { AppWindow, BookA, Power, TextQuote } from "lucide-react";
import { commands as handyCommands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import { commands } from "../bindings";
import {
  registerChecklistItem,
  type ChecklistRevealContext,
} from "./checklistRegistry";

export const TRY_IN_APP = "try_in_app";
export const ADD_WORD = "add_word";
export const CREATE_SNIPPET = "create_snippet";
export const LAUNCH_AT_LOGIN = "launch_at_login";

/** Opens a page and points at its Add button (no setting changes). */
const showPage =
  (page: string, target: string, item: string) =>
  ({ navigate, coach }: ChecklistRevealContext) => {
    navigate(page);
    coach(
      target,
      `checklist.items.${item}.coachTitle`,
      `checklist.items.${item}.coachBody`,
    );
  };

const showDictionary = showPage(
  "dictionary",
  '[data-testid="wb-dictionary-add"]',
  ADD_WORD,
);
const showSnippets = showPage(
  "snippets",
  '[data-testid="wb-snippets-add"]',
  CREATE_SNIPPET,
);

let registered = false;

/** Registers the built-in items once (from the checklist card's module). */
export function registerBuiltInChecklistItems(): void {
  if (registered) return;
  registered = true;

  registerChecklistItem({
    id: TRY_IN_APP,
    order: 10,
    icon: AppWindow,
    titleKey: "checklist.items.try_in_app.title",
    actionKey: "checklist.items.try_in_app.action",
    run: ({ openChooser }) => openChooser(),
  });

  registerChecklistItem({
    id: ADD_WORD,
    order: 20,
    icon: BookA,
    titleKey: "checklist.items.add_word.title",
    actionKey: "checklist.items.add_word.action",
    isDone: async () => {
      const result = await commands.forkDictionaryList();
      return (
        result.status === "ok" &&
        result.data.counts.words + result.data.counts.rules > 0
      );
    },
    refreshOn: ["fork-dictionary-changed"],
    run: showDictionary,
    reveal: showDictionary,
  });

  registerChecklistItem({
    id: CREATE_SNIPPET,
    order: 30,
    icon: TextQuote,
    titleKey: "checklist.items.create_snippet.title",
    actionKey: "checklist.items.create_snippet.action",
    isDone: async () => {
      const result = await commands.forkSnippetsList();
      return result.status === "ok" && result.data.counts.snippets > 0;
    },
    refreshOn: ["fork-snippets-changed"],
    run: showSnippets,
    reveal: showSnippets,
  });

  registerChecklistItem({
    id: LAUNCH_AT_LOGIN,
    order: 40,
    icon: Power,
    titleKey: "checklist.items.launch_at_login.title",
    actionKey: "checklist.items.launch_at_login.action",
    bodyKey: "checklist.items.launch_at_login.body",
    isDone: async () => {
      const result = await handyCommands.getAppSettings();
      return result.status === "ok" && result.data.autostart_enabled === true;
    },
    refreshOn: ["fork-settings-changed"],
    // No `reveal`: turning autostart on is a side effect, a click only.
    run: async ({ markDone }) => {
      await useSettingsStore
        .getState()
        .updateSetting("autostart_enabled", true);
      if (useSettingsStore.getState().settings?.autostart_enabled) {
        await markDone();
      }
    },
  });
}
