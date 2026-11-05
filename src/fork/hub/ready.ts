// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Tells Rust the Hub page is listening (S-38, `fork::hub::park`).
 *
 * The main window is destroyed while it stays hidden and rebuilt when it is
 * needed again; a native notification's Open or a View changes handoff that
 * arrives with the rebuild waits in Rust until this call, so the page's
 * listeners exist when the event is sent. The call also lets a Hub that
 * started hidden (login item) be parked once Handy's start-up work in
 * `App.tsx` has run. Called once per page load, after `listeners` resolve.
 */
import { commands } from "../bindings";

let reported: Promise<void> | null = null;

export function reportHubReady(listeners: Promise<unknown>[]): Promise<void> {
  reported ??= Promise.allSettled(listeners)
    .then(() => commands.forkHubReady())
    .then(() => undefined)
    .catch((error) => {
      reported = null;
      console.warn("hub: could not report the page ready", error);
    });
  return reported;
}
