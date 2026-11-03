// SPDX-License-Identifier: GPL-3.0-or-later
// Direction for the `fork-pill` page in tests (S-38).
//
// The page sets `<html dir>` itself from the app language: Pill.tsx's effect
// after mount, and Handy's i18n when the language from `get_app_settings`
// arrives. A test that wrote `dir = "rtl"` on its own raced both, and under
// load one of them could run after the write and put `ltr` back (the RTL
// specs failed now and then). RTL tests therefore ask for an RTL app
// language and wait until the page has applied it.
import type { Page } from "@playwright/test";
import { expect } from "./fixtures";

/** Handy's settings with an RTL app language (Arabic). */
export const RTL_APP_SETTINGS = { app_language: "ar" };

/** Waits until the page itself has switched to right-to-left. */
export async function expectRtl(page: Page): Promise<void> {
  await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
  await expect(page.locator("html")).toHaveAttribute("lang", "ar");
}
