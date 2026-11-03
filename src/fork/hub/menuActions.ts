// SPDX-License-Identifier: GPL-3.0-or-later
/** Native Dictation, Vocabulary, Help and tray actions (S-26a). */
import { useEffect } from "react";
import { events } from "../bindings";
import { isHubPageId, navigate } from "./nav";

let listening: Promise<void> | null = null;

/**
 * Listens once per page load. S-38: started before the Hub mounts (with the
 * other listeners `reportHubReady` waits for), so an action that rebuilt a
 * parked Hub reaches it; `navigate` only sets the nav store.
 */
export function listenForMenuActions(): Promise<void> {
  listening ??= events.forkMenuAction
    .listen((event) => {
      const { page, section } = event.payload;
      if (isHubPageId(page)) navigate(page, section);
    })
    .then(() => undefined)
    .catch((error) => {
      listening = null;
      console.warn("hub: listening for menu actions failed", error);
    });
  return listening;
}

export function useMenuActions(): void {
  useEffect(() => {
    void listenForMenuActions();
  }, []);
}
