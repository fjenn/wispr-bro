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
 * - Open at login: Handy's `autostart_enabled`; "Turn on" sets it.
 */
import { AppWindow, BookA, Power, TextQuote } from "lucide-react";
import { commands as handyCommands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import { commands } from "../bindings";
import { registerChecklistItem } from "./checklistRegistry";

export const TRY_IN_APP = "try_in_app";
export const ADD_WORD = "add_word";
export const CREATE_SNIPPET = "create_snippet";
export const LAUNCH_AT_LOGIN = "launch_at_login";

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
    run: ({ navigate, coach }) => {
      navigate("dictionary");
      coach(
        '[data-testid="wb-dictionary-add"]',
        "checklist.items.add_word.coachTitle",
        "checklist.items.add_word.coachBody",
      );
    },
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
    run: ({ navigate, coach }) => {
      navigate("snippets");
      coach(
        '[data-testid="wb-snippets-add"]',
        "checklist.items.create_snippet.coachTitle",
        "checklist.items.create_snippet.coachBody",
      );
    },
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
