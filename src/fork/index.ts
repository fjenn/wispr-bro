// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Wispr Bro frontend entry.
 *
 * The only frontend hook in F-05 is one call in `src/i18n/index.ts`:
 * `registerForkI18n(i18n)`. Besides loading the `fork` namespace, it starts
 * the fork frontend in the main window: today that is only the hidden debug
 * host (⌘⇧D / Ctrl+Shift+D or `#/fork/debug`). S-13 moves the fork root into
 * `App.tsx` (`ForkApp`), and this side effect goes away.
 */
import type { i18n as I18n } from "i18next";
import { installCspViolationRecorder } from "./debug/cspViolations";
import { registerForkI18n as registerForkNamespace } from "./i18n";

export { FORK_NAMESPACE, forkResources } from "./i18n";

let started = false;

/** Label of the webview this script runs in, or null outside Tauri. */
function currentWebviewLabel(): string | null {
  const internals = (
    window as unknown as {
      __TAURI_INTERNALS__?: {
        metadata?: { currentWebview?: { label?: string } };
      };
    }
  ).__TAURI_INTERNALS__;
  return internals?.metadata?.currentWebview?.label ?? null;
}

/** Starts the fork frontend once, in the main window only. */
export function startForkFrontend(): void {
  if (started || typeof document === "undefined") return;
  if (currentWebviewLabel() !== "main") return;
  started = true;
  installCspViolationRecorder();
  // Loaded lazily so other webviews (the overlay) never bundle the panel.
  void import("./debug/mount").then(({ mountForkDebugHost }) =>
    mountForkDebugHost(),
  );
}

/** The `src/i18n/index.ts` hook. */
export function registerForkI18n(i18n: I18n): void {
  registerForkNamespace(i18n);
  startForkFrontend();
}
