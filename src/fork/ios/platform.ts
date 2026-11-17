// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Wispr Bro for iPhone: platform flag.
 *
 * The iPhone app is the same React bundle as the Mac app, run inside a
 * Tauri iOS web view. Everything iPhone-specific sits behind `isIos()`:
 *
 * - on a device, `@tauri-apps/plugin-os` reports `platform() === "ios"`;
 * - for development in a desktop browser, `?platform=ios` on the URL (kept
 *   in `localStorage["wb.platform"]`, so reloads stay on the phone layout)
 *   forces it; `?platform=auto` clears the override.
 *
 * `applyIosFlag()` runs once from `main.tsx` before the first render: it
 * marks `<html data-wb-ios>` (what `ios.css` scopes on) and widens the
 * viewport under the notch and home indicator (`viewport-fit=cover`), with
 * zoom locked so a focused field never zooms the page.
 */
import { platform } from "@tauri-apps/plugin-os";

export const IOS_OVERRIDE_KEY = "wb.platform";

let cached: boolean | null = null;
let forced = false;

function readOverride(): boolean {
  if (typeof window === "undefined") return false;
  try {
    const param = new URLSearchParams(window.location.search).get("platform");
    if (param === "ios") {
      localStorage.setItem(IOS_OVERRIDE_KEY, "ios");
    } else if (param) {
      localStorage.removeItem(IOS_OVERRIDE_KEY);
    }
    return localStorage.getItem(IOS_OVERRIDE_KEY) === "ios";
  } catch {
    return false;
  }
}

/** True on an iPhone/iPad, or when the dev override is on. */
export function isIos(): boolean {
  if (cached !== null) return cached;
  forced = readOverride();
  let real = false;
  try {
    real = platform() === "ios";
  } catch {
    real = false;
  }
  cached = real || forced;
  return cached;
}

/**
 * True when the phone layout is forced in a desktop browser (no iOS
 * backend): the iOS commands then fall back to an in-memory stand-in.
 */
export function isIosDevOverride(): boolean {
  isIos();
  return forced;
}

/** Marks the document for the phone layout. Call before the first render. */
export function applyIosFlag(): void {
  if (!isIos() || typeof document === "undefined") return;
  const root = document.documentElement;
  root.dataset.wbIos = "";
  root.dataset.platform = "ios";
  const content =
    "width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover";
  let meta = document.querySelector<HTMLMetaElement>('meta[name="viewport"]');
  if (!meta) {
    meta = document.createElement("meta");
    meta.name = "viewport";
    document.head.appendChild(meta);
  }
  meta.content = content;
}
