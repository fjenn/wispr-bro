// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * wb-dmg-background (OT2-E): draws the install window's background for the
 * DMG, `src-tauri/icons-fork/dmg/background.png` (1x) and `background@2x.png`
 * (2x), from the layout in `src-tauri/tauri.fork.conf.json`
 * (`bundle.macOS.dmg`: window size and the two icon centres), so the arrow
 * and the tray always sit under the icons `scripts/wb-release/dmg/build.sh`
 * places. Our own art and wording, in the Hub's type and palette: a serif
 * heading with one italic word, one line of help, a mid-tone tray behind
 * the two icons and a drawn arrow between them.
 *
 * The tray carries Finder's icon labels. Finder draws them dark in the
 * light appearance and light in the dark one, over the same picture, so the
 * tray's tone is picked for ≥ 4.5:1 against both black and white (relative
 * luminance ≈ 0.18); everything else is light.
 *
 * Usage: bun scripts/wb-release/dmg/background.ts [--check]
 *   --check  render to a temp dir and fail when the committed PNGs' sizes
 *            differ (the pixels vary with the browser's font rasteriser, so
 *            they are not compared byte for byte).
 * Renders with Playwright on installed Chrome (`WB_PW_CHANNEL`, default
 * "chrome"); nothing is fetched (fonts are the bundled woff2 files).
 */
import fs from "fs";
import os from "os";
import path from "path";
import { fileURLToPath, pathToFileURL } from "url";
import { chromium } from "@playwright/test";

const ROOT = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../..",
);
const CONFIG = path.join(ROOT, "src-tauri/tauri.fork.conf.json");
const FONTS = path.join(ROOT, "src/assets/fonts/fork");

export interface DmgLayout {
  background: string;
  width: number;
  height: number;
  app: { x: number; y: number };
  applications: { x: number; y: number };
}

/** The DMG layout from the fork overlay (Tauri's `bundle.macOS.dmg`). */
export function dmgLayout(configPath = CONFIG): DmgLayout {
  const conf = JSON.parse(fs.readFileSync(configPath, "utf8"));
  const dmg = conf?.bundle?.macOS?.dmg;
  if (!dmg) throw new Error("bundle.macOS.dmg missing in the fork overlay");
  return {
    background: dmg.background,
    width: dmg.windowSize.width,
    height: dmg.windowSize.height,
    app: dmg.appPosition,
    applications: dmg.applicationFolderPosition,
  };
}

/** Icon size the window uses (dmg.sh passes the same to dmgbuild). */
export const ICON_SIZE = 128;
/** Mid-tone tray: relative luminance 0.179, 4.58:1 to white and to black. */
export const TRAY = "#827365";

function font(file: string): string {
  return pathToFileURL(path.join(FONTS, file)).href;
}

export function html(l: DmgLayout): string {
  const half = ICON_SIZE / 2;
  // The tray spans both icons and their labels (Finder draws a label about
  // 6 pt under the icon, two lines at most).
  const trayX = l.app.x - half - 44;
  const trayY = l.app.y - half - 22;
  const trayW = l.applications.x - l.app.x + ICON_SIZE + 88;
  const trayH = ICON_SIZE + 22 + 58;
  // The arrow runs between the icons' edges, a little above their centres.
  const x0 = l.app.x + half + 22;
  const x1 = l.applications.x - half - 22;
  const y = l.app.y - 6;
  const mid = (x0 + x1) / 2;
  return `<!doctype html><html><head><meta charset="utf-8"><style>
@font-face{font-family:"WB Figtree";src:url("${font("figtree-latin-wght-normal.woff2")}") format("woff2");font-weight:300 900}
@font-face{font-family:"WB Garamond";src:url("${font("eb-garamond-latin-wght-normal.woff2")}") format("woff2");font-weight:400 800}
@font-face{font-family:"WB Garamond";font-style:italic;src:url("${font("eb-garamond-latin-wght-italic.woff2")}") format("woff2");font-weight:400 800}
html,body{margin:0;width:${l.width}px;height:${l.height}px;overflow:hidden}
body{position:relative;background:
  radial-gradient(60% 70% at 8% 0%,rgba(246,185,138,.28) 0%,transparent 70%),
  radial-gradient(55% 65% at 100% 6%,rgba(154,114,186,.20) 0%,transparent 70%),
  #f7f6f2;color:#1a1a1a;font-family:"WB Figtree",sans-serif;-webkit-font-smoothing:antialiased}
h1{position:absolute;left:0;right:0;top:38px;margin:0;text-align:center;font:400 34px/40px "WB Garamond",serif;letter-spacing:-.01em}
h1 em{font-style:italic}
p{position:absolute;left:0;right:0;top:86px;margin:0;text-align:center;font:500 14px/20px "WB Figtree",sans-serif;color:#555}
.tray{position:absolute;left:${trayX}px;top:${trayY}px;width:${trayW}px;height:${trayH}px;border-radius:28px;background:${TRAY};box-shadow:inset 0 1px 0 rgba(255,255,255,.18),0 1px 2px rgba(60,40,20,.10),0 8px 24px rgba(60,40,20,.10)}
svg{position:absolute;left:0;top:0}
</style></head><body>
<h1>Drag to <em>install</em></h1>
<p>Drop Wispr Bro on the Applications folder, then open it from there.</p>
<div class="tray"></div>
<svg width="${l.width}" height="${l.height}" viewBox="0 0 ${l.width} ${l.height}" fill="none" stroke="#ffffff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">
  <path d="M${x0} ${y + 8}C${mid - 30} ${y - 22} ${mid + 24} ${y - 22} ${x1 - 4} ${y + 2}"/>
  <path d="M${x1 - 20} ${y - 12}L${x1 - 2} ${y + 3}L${x1 - 22} ${y + 13}"/>
</svg>
</body></html>`;
}

async function render(outDir: string, l: DmgLayout): Promise<string[]> {
  const browser = await chromium.launch({
    channel: process.env.WB_PW_CHANNEL ?? "chrome",
  });
  const written: string[] = [];
  try {
    const page0 = path.join(outDir, "dmg-background.html");
    fs.writeFileSync(page0, html(l));
    const base = path.basename(l.background, ".png");
    for (const scale of [1, 2]) {
      const ctx = await browser.newContext({
        viewport: { width: l.width, height: l.height },
        deviceScaleFactor: scale,
        colorScheme: "light",
      });
      const page = await ctx.newPage();
      await page.goto(pathToFileURL(page0).href);
      await page.evaluate(() => document.fonts.ready);
      const file = path.join(
        outDir,
        scale === 1 ? `${base}.png` : `${base}@2x.png`,
      );
      await page.screenshot({ path: file, omitBackground: false });
      written.push(file);
      await ctx.close();
    }
    fs.rmSync(page0);
  } finally {
    await browser.close();
  }
  return written;
}

/** PNG width × height from the IHDR chunk. */
export function pngSize(file: string): [number, number] {
  const b = fs.readFileSync(file);
  return [b.readUInt32BE(16), b.readUInt32BE(20)];
}

async function main(): Promise<void> {
  const l = dmgLayout();
  const target = path.join(ROOT, "src-tauri", path.dirname(l.background));
  if (process.argv.includes("--check")) {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "wb-dmg-bg-"));
    const files = await render(tmp, l);
    let bad = 0;
    for (const f of files) {
      const committed = path.join(target, path.basename(f));
      if (!fs.existsSync(committed)) {
        console.log(`✗ missing ${committed}`);
        bad++;
        continue;
      }
      const [a, b] = [pngSize(f), pngSize(committed)];
      if (a[0] !== b[0] || a[1] !== b[1]) {
        console.log(`✗ ${path.basename(f)}: ${b.join("×")} ≠ ${a.join("×")}`);
        bad++;
      } else console.log(`✓ ${path.basename(f)} ${a.join("×")}`);
    }
    fs.rmSync(tmp, { recursive: true, force: true });
    if (bad) process.exit(1);
    return;
  }
  fs.mkdirSync(target, { recursive: true });
  for (const f of await render(target, l)) {
    const [w, h] = pngSize(f);
    console.log(`wrote ${path.relative(ROOT, f)} (${w}×${h})`);
  }
}

if (import.meta.main) await main();
