// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * wb-icons: regenerate the Wispr Bro app and tray icons from the SVG sources
 * in `src-tauri/icons-fork/src/` (F-01). The current artwork is a PLACEHOLDER.
 *
 * Usage: bun scripts/wb-icons.ts
 *
 * Outputs
 *   src-tauri/icons-fork/            app icons (`tauri icon`; desktop sizes only,
 *                                    the android/ios folders are removed)
 *   src-tauri/resources/fork/tray/   64×64 tray glyphs, white (`tray_*.png`, for
 *                                    dark menu bars) and black (`tray_*_dark.png`,
 *                                    for light ones), the same naming and theme
 *                                    convention as Handy's `resources/tray_*.png`.
 *
 * Tray glyphs live under `resources/` (not `icons-fork/`) so Handy's
 * `bundle.resources: ["resources/**\/*"]` bundles them in every build, with or
 * without the fork overlay (see docs/fork/ARCHITECTURE.md, "Icons").
 */
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";
import { fileURLToPath } from "url";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const TAURI_DIR = path.join(ROOT, "src-tauri");
const SRC = path.join(TAURI_DIR, "icons-fork", "src");
const APP_OUT = path.join(TAURI_DIR, "icons-fork");
const TRAY_OUT = path.join(TAURI_DIR, "resources", "fork", "tray");

const TRAY_GLYPHS: Record<string, string> = {
  "tray-idle.svg": "tray_idle",
  "tray-recording.svg": "tray_recording",
  "tray-transcribing.svg": "tray_transcribing",
  "tray-idle-warning.svg": "tray_idle_warning",
};

function tauriIcon(args: string[]): void {
  const r = spawnSync("bunx", ["tauri", "icon", ...args], {
    cwd: ROOT,
    stdio: "inherit",
  });
  if (r.status !== 0) throw new Error(`tauri icon ${args.join(" ")} failed`);
}

// App icons.
tauriIcon([path.join(SRC, "app-icon.svg"), "-o", APP_OUT]);
for (const dir of ["android", "ios"]) {
  fs.rmSync(path.join(APP_OUT, dir), { recursive: true, force: true });
}

// Tray glyphs: white for dark menu bars, black (`_dark`) for light ones.
fs.mkdirSync(TRAY_OUT, { recursive: true });
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "wb-icons-"));
try {
  for (const [file, name] of Object.entries(TRAY_GLYPHS)) {
    const svg = fs.readFileSync(path.join(SRC, file), "utf8");
    for (const [suffix, color] of [
      ["", "#FFFFFF"],
      ["_dark", "#000000"],
    ]) {
      const variant = path.join(tmp, `${name}${suffix}.svg`);
      fs.writeFileSync(
        variant,
        svg.replaceAll('fill="#000000"', `fill="${color}"`),
      );
      const out = path.join(tmp, `${name}${suffix}`);
      tauriIcon([variant, "-o", out, "-p", "64"]);
      fs.copyFileSync(
        path.join(out, "64x64.png"),
        path.join(TRAY_OUT, `${name}${suffix}.png`),
      );
    }
  }
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
console.log(`App icons in ${APP_OUT}; tray glyphs in ${TRAY_OUT}`);
