// SPDX-License-Identifier: GPL-3.0-or-later
// F-01: Handy's own i18n instance, loaded exactly as the app loads it
// (src/i18n/index.ts → registerForkI18n), serves the fork brand overrides.
import { test, expect } from "@playwright/test";
import { installTauriMock } from "./ipc-mock";

type Probe = Record<string, Record<string, string>>;

test("Handy strings are rebranded at load in English and two other locales", async ({
  page,
}) => {
  await page.addInitScript(installTauriMock, {});
  await page.goto("/#/fork/debug");
  const probe: Probe = await page.evaluate(async () => {
    const mod = await import("/src/i18n/index.ts");
    const i18n = mod.default;
    const keys = [
      "settings.about.version.description",
      "onboarding.done.subMac",
      "onboarding.welcome.noMatch",
      "whatsNew.title",
      "settings.debug.updateChecks.lockedDescription",
    ];
    const out: Record<string, Record<string, string>> = {};
    for (const lng of ["en", "de", "tr"]) {
      out[lng] = {};
      for (const key of keys) {
        out[lng][key] = i18n.t(key, {
          lng,
          query: "x",
          version: "1.0.0-alpha.1",
        });
      }
      // The fork namespace is still separate from Handy's.
      out[lng]["fork:rust.appName"] = i18n.t("rust.appName", {
        lng,
        ns: "fork",
      });
    }
    return out;
  });

  for (const [lng, strings] of Object.entries(probe)) {
    for (const [key, value] of Object.entries(strings)) {
      expect(value, `${lng} ${key}`).not.toMatch(/Handy|HANDY_/);
      expect(value, `${lng} ${key}`).not.toBe(key.replace(/^fork:/, ""));
    }
    expect(strings["fork:rust.appName"]).toBe("Wispr Bro");
    expect(strings["whatsNew.title"]).toContain("Wispr Bro");
    expect(strings["whatsNew.title"]).toContain("1.0.0-alpha.1");
  }
  expect(probe.en["settings.about.version.description"]).toBe(
    "Current version of Wispr Bro",
  );
  expect(probe.tr["onboarding.welcome.noMatch"]).toMatch(/Wispr Bro['’]nun/);
  expect(probe.de["settings.debug.updateChecks.lockedDescription"]).toContain(
    "WISPRBRO_DISABLE_UPDATER",
  );
});
