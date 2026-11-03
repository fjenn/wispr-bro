// SPDX-License-Identifier: GPL-3.0-or-later
/** Native Dictation, Vocabulary, Help and tray actions (S-26a). */
import { useEffect } from "react";
import { events } from "../bindings";
import { isHubPageId, navigate } from "./nav";

export function useMenuActions(): void {
  useEffect(() => {
    const listening = events.forkMenuAction.listen((event) => {
      const { page, section } = event.payload;
      if (isHubPageId(page)) navigate(page, section);
    });
    return () => {
      void listening.then((stop) => stop());
    };
  }, []);
}
