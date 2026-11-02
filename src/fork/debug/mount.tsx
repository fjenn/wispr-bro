// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hidden host for the fork debug panel and onboarding's Shortcuts dialog.
 *
 * The debug panel opens with the same chord as Handy's debug-mode toggle (⌘⇧D on macOS,
 * Ctrl+Shift+D elsewhere; see `App.tsx`) or the route `#/fork/debug`, but only
 * when the backend reports a debug build or a `--debug` launch (`fork_ping`).
 * In a release build without `--debug` the chord keeps toggling Handy's debug
 * mode and the fork panel never renders.
 */
import React, { useCallback, useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { commands } from "../bindings";
import { OnboardingHotkeysLink } from "../onboarding/HotkeysLink";
import { CspSmoke } from "./CspSmoke";
import { NotificationsDebug } from "./NotificationsDebug";
import { UpdateDebug } from "./UpdateDebug";
import { runCspSmoke } from "./cspSmokeChecks";
import { DebugPanel, registerForkDebugSection } from "./DebugPanel";

// Product settings live in S-14's Settings and Shortcuts dialogs. Only
// diagnostics remain on this panel.

// S-18: macOS permission and a sample poster for the Mac test script.
registerForkDebugSection({
  id: "notifications",
  titleKey: "debug.notifications.title",
  render: () => <NotificationsDebug />,
});

// F-02: update channel, Check now and Install (S-28 adds the product UI).
registerForkDebugSection({
  id: "update",
  titleKey: "debug.update.title",
  render: () => <UpdateDebug />,
});

// F-08: CSP smoke check. Also exposed to the WISPRBRO_SELFCHECK script
// (`fork/commands/selfcheck.rs`), which runs it headlessly in any build.
registerForkDebugSection({
  id: "csp",
  titleKey: "debug.csp.title",
  render: () => <CspSmoke />,
});
(window as unknown as { __wbCspSmoke?: typeof runCspSmoke }).__wbCspSmoke =
  runCspSmoke;

export const FORK_DEBUG_ROUTE = "#/fork/debug";
const ROOT_ID = "fork-debug-root";

export function isDebugChord(event: KeyboardEvent): boolean {
  return (
    event.shiftKey &&
    event.key.toLowerCase() === "d" &&
    (event.ctrlKey || event.metaKey)
  );
}

/** Asks the backend whether the debug panel may open. */
async function debugPanelAllowed(): Promise<boolean> {
  try {
    const result = await commands.forkPing();
    return (
      result.status === "ok" &&
      (result.data.debug_build || result.data.cli_debug)
    );
  } catch (e) {
    console.warn("fork: fork_ping failed", e);
    return false;
  }
}

function ForkDebugHost() {
  const [open, setOpen] = useState(false);

  const openIfAllowed = useCallback(async () => {
    if (await debugPanelAllowed()) setOpen(true);
  }, []);

  const close = useCallback(() => {
    setOpen(false);
    if (window.location.hash === FORK_DEBUG_ROUTE) {
      history.replaceState(
        null,
        "",
        window.location.pathname + window.location.search,
      );
    }
  }, []);

  useEffect(() => {
    const onRoute = () => {
      if (window.location.hash === FORK_DEBUG_ROUTE) void openIfAllowed();
    };
    onRoute();
    window.addEventListener("hashchange", onRoute);
    return () => window.removeEventListener("hashchange", onRoute);
  }, [openIfAllowed]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!isDebugChord(event)) return;
      // Handy's own listener on `document` also sees this event and toggles
      // its debug mode; we only add the fork panel on top.
      if (open) close();
      else void openIfAllowed();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open, close, openIfAllowed]);

  return (
    <>
      <OnboardingHotkeysLink />
      {open && <DebugPanel onClose={close} />}
    </>
  );
}

export function mountForkDebugHost(): void {
  if (document.getElementById(ROOT_ID)) return;
  const container = document.createElement("div");
  container.id = ROOT_ID;
  document.body.appendChild(container);
  ReactDOM.createRoot(container).render(
    <React.StrictMode>
      <ForkDebugHost />
    </React.StrictMode>,
  );
}
