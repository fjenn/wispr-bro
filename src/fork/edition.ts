// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The one file of the normal app that knows the publik edition exists
 * (docs/fork/EDITIONS.md). Build time only, read by `vite-inputs.ts`.
 *
 * With `VITE_WB_EDITION=publik` (`bun run wb:build:publik`) the publik
 * sign-in page becomes an extra Vite entry. Without it nothing is added, so
 * the normal build is byte for byte what it was before the editions.
 */
import { resolve } from "path";

export const PUBLIK_EDITION = process.env.VITE_WB_EDITION === "publik";

export function editionInputs(root: string): Record<string, string> {
  if (!PUBLIK_EDITION) return {};
  return {
    // src-tauri/src/fork/publik/gate.rs (`PAGE`).
    "publik-gate": resolve(root, "src/fork/publik/index.html"),
  };
}
