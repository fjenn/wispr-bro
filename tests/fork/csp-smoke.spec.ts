// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * F-08 CSP smoke panel under the overlay's `devCsp` (served as a response
 * header by the test, since Vite's dev server sends none; Tauri itself only
 * applies a CSP to pages it serves, i.e. release builds on desktop). Vite's
 * dev HTML carries one inline script (the React-refresh preamble) that a
 * built `index.html` does not have, so the test adds `'unsafe-inline'` to
 * `script-src` for that page only. The real asset
 * protocol only exists in a Tauri build; here `fork_csp_smoke_media` points at
 * a same-origin WAV and the mock's `convertFileSrc` returns it unchanged. The
 * release check (`WISPRBRO_SELFCHECK=1` on a `wb:build`) covers the rest.
 */
import { readFileSync } from "node:fs";
import { test, expect } from "@playwright/test";
import { installTauriMock } from "./ipc-mock";

const overlay = JSON.parse(
  readFileSync("src-tauri/tauri.fork.conf.json", "utf8"),
) as { app: { security: { devCsp: string } } };
const devCspForViteDev = overlay.app.security.devCsp.replace(
  "script-src 'self'",
  "script-src 'self' 'unsafe-inline'",
);

/** 0.1 s of 16 kHz mono 16-bit silence (same as `fork/commands/csp.rs`). */
function fixtureWav(): Buffer {
  const samples = 1600;
  const dataLen = samples * 2;
  const b = Buffer.alloc(44 + dataLen);
  b.write("RIFF", 0);
  b.writeUInt32LE(36 + dataLen, 4);
  b.write("WAVEfmt ", 8);
  b.writeUInt32LE(16, 16);
  b.writeUInt16LE(1, 20);
  b.writeUInt16LE(1, 22);
  b.writeUInt32LE(16000, 24);
  b.writeUInt32LE(32000, 28);
  b.writeUInt16LE(2, 32);
  b.writeUInt16LE(16, 34);
  b.write("data", 36);
  b.writeUInt32LE(dataLen, 40);
  return b;
}

test("CSP smoke panel passes under the overlay devCsp", async ({ page }) => {
  await page.route("**/*", async (route) => {
    const req = route.request();
    if (new URL(req.url()).pathname === "/__wb_csp_fixture.wav") {
      return route.fulfill({
        status: 200,
        contentType: "audio/wav",
        body: fixtureWav(),
      });
    }
    if (req.resourceType() === "document") {
      const response = await route.fetch();
      return route.fulfill({
        response,
        headers: {
          ...response.headers(),
          "content-security-policy": devCspForViteDev,
        },
      });
    }
    return route.continue();
  });
  await page.addInitScript(installTauriMock, {
    fork_ping: {
      message: "pong",
      seq: 0,
      ready: true,
      debug_build: true,
      cli_debug: false,
    },
    fork_model_status: { known_models: 1, downloaded_model_ids: [] },
    fork_csp_smoke_media: { path: "/__wb_csp_fixture.wav", source: "fixture" },
  });
  await page.goto("/#/fork/debug");

  const section = page.getByTestId("fork-debug-section-csp");
  await expect(section).toBeVisible();
  await section.getByTestId("fork-csp-smoke-run").click();
  await expect(page.getByTestId("fork-csp-smoke-verdict")).toHaveText(
    "Passed: no unexpected CSP violation.",
    { timeout: 15000 },
  );
  for (const key of ["media", "dataImage", "blobImage", "forkCommand"]) {
    await expect(page.getByTestId(`fork-csp-check-${key}`)).toContainText(
      "ok (",
    );
  }
  await expect(page.getByTestId("fork-csp-check-probe")).toContainText(
    "blocked by img-src",
  );
  await expect(page.getByTestId("fork-csp-violations")).toHaveText(
    "No CSP violations recorded.",
  );

  // Exposed for the WISPRBRO_SELFCHECK script.
  const headless = await page.evaluate(async () => {
    const w = window as unknown as {
      __wbCspSmoke?: () => Promise<{ ok: boolean }>;
    };
    return w.__wbCspSmoke ? (await w.__wbCspSmoke()).ok : null;
  });
  expect(headless).toBe(true);
});
