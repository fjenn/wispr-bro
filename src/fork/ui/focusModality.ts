// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Focus rings only after keyboard navigation (OT2-E, Mac wave 6: a deep
 * link into Settings or a tab drew the ring on the modal's close button or
 * the tab panel). WebKit matches `:focus-visible` on focus that script
 * moves (a dialog's autofocus, a tab panel focused by a deep link) even
 * when the user only clicked; macOS draws no ring there until Tab is
 * pressed. So `<html data-wb-modality>` records how the user last moved:
 * "keyboard" after a navigation key (Tab, arrows, Home/End, Page Up/Down,
 * without ⌘/⌃), "pointer" after a pointer press, absent at launch. The
 * stylesheet (`tokens.css`, "Focus modality") hides rings on controls
 * unless the modality is "keyboard"; text fields keep theirs (a caret
 * field shows focus however it got it).
 */
const NAV_KEYS = new Set([
  "Tab",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown",
]);

export type FocusModality = "keyboard" | "pointer";

export function setFocusModality(
  modality: FocusModality,
  root: HTMLElement = document.documentElement,
): void {
  if (root.dataset.wbModality !== modality) root.dataset.wbModality = modality;
}

/** True when this key press is keyboard navigation (a ring should show). */
export function isNavigationKey(e: {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
}): boolean {
  return NAV_KEYS.has(e.key) && !e.metaKey && !e.ctrlKey;
}

let installed = false;

/** Installs the listeners once per window. */
export function installFocusModality(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  window.addEventListener(
    "keydown",
    (e) => {
      if (isNavigationKey(e)) setFocusModality("keyboard");
    },
    true,
  );
  window.addEventListener(
    "pointerdown",
    () => setFocusModality("pointer"),
    true,
  );
}
