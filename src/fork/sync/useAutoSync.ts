// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * `useAutoSync()`: automatic sync as the pages show it
 * (`src-tauri/src/fork/sync/auto.rs`): paired devices, whether each is
 * connected, the Macs found while searching, and a pairing in progress.
 *
 * The status is read on mount, after every `fork-sync-auto-changed` event
 * and twice a minute (the "2 min ago" on screen ages by itself).
 */
import { useCallback, useEffect, useState } from "react";
import type { TFunction } from "i18next";
import {
  commands,
  events,
  type AutoStatus,
  type PairedDevice,
} from "../bindings";

export function useAutoSync() {
  const [status, setStatus] = useState<AutoStatus | null>(null);
  const [, setTick] = useState(0);

  const refresh = useCallback(async () => {
    try {
      const next = await commands.forkSyncAutoStatus();
      if (next) setStatus(next);
    } catch {
      // A backend without automatic sync: the pages show the file route.
    }
  }, []);

  useEffect(() => {
    let alive = true;
    void refresh();
    const unlisten = events.forkSyncAutoChanged.listen(() => {
      if (alive) void refresh();
    });
    const timer = window.setInterval(() => {
      void refresh();
      setTick((n) => n + 1);
    }, 30_000);
    return () => {
      alive = false;
      window.clearInterval(timer);
      void unlisten.then((f) => f()).catch(() => undefined);
    };
  }, [refresh]);

  const act = useCallback(
    (run: () => Promise<unknown>) => {
      void run()
        .catch(() => undefined)
        .then(refresh);
    },
    [refresh],
  );

  return {
    status,
    refresh,
    enable: (on: boolean) => act(() => commands.forkSyncAutoEnable(on)),
    search: (on: boolean) => act(() => commands.forkSyncSearch(on)),
    pair: (service: string) => act(() => commands.forkSyncPair(service)),
    sendCode: (code: string | null) =>
      act(() => commands.forkSyncPairCode(code)),
    unpair: (deviceId: string) => act(() => commands.forkSyncUnpair(deviceId)),
    syncNow: () => act(() => commands.forkSyncNow()),
  };
}

export type AutoSync = ReturnType<typeof useAutoSync>;

/** "2 minutes ago" in the reader's language; "just now" under a minute. */
export function timeAgo(
  t: TFunction<"fork">,
  ms: number,
  language: string,
  now: number = Date.now(),
): string {
  const seconds = Math.max(0, Math.round((now - ms) / 1000));
  if (seconds < 60) return t("sync.auto.justNow");
  try {
    const format = new Intl.RelativeTimeFormat(language, { numeric: "always" });
    if (seconds < 3600)
      return format.format(-Math.round(seconds / 60), "minute");
    if (seconds < 86_400)
      return format.format(-Math.round(seconds / 3600), "hour");
    return format.format(-Math.round(seconds / 86_400), "day");
  } catch {
    return new Date(ms).toLocaleString();
  }
}

export type DeviceState = "in_sync" | "connecting" | "away";

/** Where a paired device stands, and the one line that says so. */
export function deviceLine(
  t: TFunction<"fork">,
  device: PairedDevice,
  language: string,
  /** `phone`: this is the iPhone looking at its Mac. */
  role: AutoStatus["role"],
): { state: DeviceState; text: string } {
  const ago =
    device.last_sync_at != null
      ? timeAgo(t, device.last_sync_at, language)
      : null;
  if (device.needs_update && !device.connected) {
    return { state: "away", text: t("sync.auto.needsUpdate") };
  }
  if (device.connected) {
    return {
      state: "in_sync",
      text: ago
        ? t("sync.auto.inSyncAgo", { when: ago })
        : t("sync.auto.inSync"),
    };
  }
  if (role === "phone" && device.found) {
    return { state: "connecting", text: t("sync.auto.connecting") };
  }
  const away = t(
    role === "phone" ? "sync.auto.macAway" : "sync.auto.phoneAway",
  );
  return {
    state: "away",
    text: ago ? `${away} ${t("sync.auto.lastSynced", { when: ago })}` : away,
  };
}
