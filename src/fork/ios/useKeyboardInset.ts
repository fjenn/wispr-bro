// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Keyboard avoidance. iOS does not resize the page when the keyboard
 * opens; it only shrinks the visual viewport. This puts the covered
 * height (from the app itself, else from the visual viewport) into `--wb-ios-kb` on `<html>` (sheets and the page's bottom
 * padding use it) and marks `<html data-wb-kb>` while a keyboard is up, so
 * the tab bar gets out of the way. The page itself never scrolls (its
 * own scroll areas do).
 */
import { useEffect } from "react";

export function useKeyboardInset(): void {
  useEffect(() => {
    const viewport = window.visualViewport;
    if (!viewport) return;
    const root = document.documentElement;
    let lastCovered = 0;
    const revealFocused = () => {
      const field = document.activeElement;
      if (
        !(field instanceof HTMLElement) ||
        !field.matches("input, textarea, [contenteditable='true']")
      ) {
        return;
      }
      field.scrollIntoView({ block: "center", behavior: "smooth" });
      window.scrollTo(0, 0);
    };
    // On the phone the app tells the page the keyboard's height before the
    // keyboard moves (`scene.m`); from then on that is the truth. In a
    // browser (the dev override) the visual viewport is all there is.
    let native: number | null = null;
    const apply = () => {
      const covered =
        native ??
        Math.max(
          0,
          Math.round(window.innerHeight - viewport.height - viewport.offsetTop),
        );
      // iOS also scrolls the whole page up to bring the focused field
      // into view, which pushes the top bar under the status bar. The
      // page is laid out for the space above the keyboard (the inset
      // above), so it stays where it is.
      if (viewport.offsetTop > 0 || window.scrollY > 0) window.scrollTo(0, 0);
      root.style.setProperty("--wb-ios-kb", `${covered}px`);
      const wasUp = "wbKb" in root.dataset;
      if (covered > 80) root.dataset.wbKb = "";
      else delete root.dataset.wbKb;
      // The keyboard came up, or changed height (another keyboard): bring
      // the field being typed in back into view, inside its own scroll
      // area, once the layout has its new bottom padding.
      if (covered > 80 && (!wasUp || covered !== lastCovered)) {
        requestAnimationFrame(revealFocused);
      }
      lastCovered = covered;
    };
    apply();
    const target = window as Window & {
      __wbKeyboardInset?: (points: number) => void;
    };
    target.__wbKeyboardInset = (points) => {
      native = Math.max(0, Math.round(points));
      apply();
    };
    viewport.addEventListener("resize", apply);
    viewport.addEventListener("scroll", apply);
    return () => {
      viewport.removeEventListener("resize", apply);
      viewport.removeEventListener("scroll", apply);
      delete target.__wbKeyboardInset;
      root.style.removeProperty("--wb-ios-kb");
      delete root.dataset.wbKb;
    };
  }, []);
}
