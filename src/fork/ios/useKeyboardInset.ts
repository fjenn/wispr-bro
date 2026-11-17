// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Keyboard avoidance. iOS does not resize the page when the keyboard
 * opens; it only shrinks the visual viewport. This mirrors the covered
 * height into `--wb-ios-kb` on `<html>` (sheets and the page's bottom
 * padding use it) and marks `<html data-wb-kb>` while a keyboard is up, so
 * the tab bar gets out of the way.
 */
import { useEffect } from "react";

export function useKeyboardInset(): void {
  useEffect(() => {
    const viewport = window.visualViewport;
    if (!viewport) return;
    const root = document.documentElement;
    const apply = () => {
      const covered = Math.max(
        0,
        Math.round(window.innerHeight - viewport.height - viewport.offsetTop),
      );
      root.style.setProperty("--wb-ios-kb", `${covered}px`);
      if (covered > 80) root.dataset.wbKb = "";
      else delete root.dataset.wbKb;
    };
    apply();
    viewport.addEventListener("resize", apply);
    viewport.addEventListener("scroll", apply);
    return () => {
      viewport.removeEventListener("resize", apply);
      viewport.removeEventListener("scroll", apply);
      root.style.removeProperty("--wb-ios-kb");
      delete root.dataset.wbKb;
    };
  }, []);
}
