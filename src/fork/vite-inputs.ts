// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Extra Vite entry points for fork windows (R5 §4.2: one import and one
 * spread in `vite.config.ts`). Each key is the page's output name, each value
 * its HTML file. S-03 adds the Flow Bar; S-24 and S-30 add the Scratchpad
 * and the spotlight.
 */
import { resolve } from "path";
import { editionInputs } from "./edition";

export function forkInputs(root: string): Record<string, string> {
  return {
    // S-03: the `fork-pill` web view (src-tauri/src/fork/pill/panel.rs).
    "fork-pill": resolve(root, "src/fork/pill/index.html"),
    // S-24: the singleton floating note window.
    "fork-scratchpad": resolve(root, "src/fork/scratchpad/index.html"),
    // S-30: the bar spotlight (click dismisses it since alpha.3;
    // src-tauri/src/fork/pill/spotlight.rs).
    "fork-spotlight": resolve(root, "src/fork/spotlight/index.html"),
    // Only in another edition's build (docs/fork/EDITIONS.md); none normally.
    ...editionInputs(root),
  };
}
