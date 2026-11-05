// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * In-window coach marks (S-30) on driver.js 1.10.2 (MIT, adopted; R4): a
 * dimmed page with one element lit and a short popover. The setup
 * checklist uses it to point at the button that does its step ("Add a
 * word"); S-31's lessons reuse it.
 *
 * driver.js writes the title, text and button labels as HTML, so only our
 * own `t()` strings ever go in, never user text (R4 parts-bin rule).
 * Esc and the close button end it; the lit element stays usable.
 */
import { driver, type Driver } from "driver.js";
import "driver.js/dist/driver.css";
import "./coach.css";

let current: Driver | null = null;

export interface CoachTexts {
  title: string;
  body: string;
  done: string;
  close: string;
}

/** Ends the coach mark on screen, if any. */
export function endCoach(): void {
  current?.destroy();
  current = null;
}

/**
 * Lights `selector` once it is on the page (it may still be rendering after
 * a navigation; gives up after `waitMs`). Resolves true when shown.
 */
export async function showCoach(
  selector: string,
  texts: CoachTexts,
  waitMs = 2000,
): Promise<boolean> {
  endCoach();
  const started = performance.now();
  let element = document.querySelector(selector);
  while (!element && performance.now() - started < waitMs) {
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    element = document.querySelector(selector);
  }
  if (!element) return false;
  const reduced = window.matchMedia?.(
    "(prefers-reduced-motion: reduce)",
  ).matches;
  const tour = driver({
    animate: !reduced,
    smoothScroll: !reduced,
    allowClose: true,
    allowKeyboardControl: true,
    overlayOpacity: 0.35,
    stagePadding: 6,
    stageRadius: 10,
    popoverClass: "wb-coach",
    showButtons: ["next", "close"],
    doneBtnText: texts.done,
    nextBtnText: texts.done,
    closeBtnLabel: texts.close,
    onDestroyed: () => {
      if (current === tour) current = null;
    },
  });
  current = tour;
  tour.highlight({
    element,
    popover: {
      title: texts.title,
      description: texts.body,
      side: "bottom",
      align: "end",
      onNextClick: () => tour.destroy(),
    },
  });
  return true;
}
