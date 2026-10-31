// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Save as image" (S-25, C-56): renders an element to a PNG on this Mac
 * with html-to-image 1.11.13 (MIT, R4 "Charts"; loaded only when used) and
 * writes it where the user chose in the save panel
 * (`fork_insights_save_image`). Social sharing is out (R2 C-56).
 *
 * Under the app's CSP (R5 §4.11) html-to-image needs only what the CSP
 * already allows: the page's own fonts are read from 'self' and inlined as
 * `data:` URLs, the element is drawn through a `data:` SVG image onto a
 * canvas, and the canvas becomes a `data:` PNG. Nothing is fetched from
 * anywhere else. Elements marked `data-export-skip` are left out.
 */
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { commands } from "../bindings";

export type SaveOutcome = "saved" | "cancelled" | "failed";

/** The page background, so the PNG is not transparent. */
function canvasColor(): string {
  const v = getComputedStyle(document.documentElement)
    .getPropertyValue("--wb-canvas")
    .trim();
  return v || "#ffffff";
}

/** `node` as a PNG, base64 without the `data:` prefix. */
export async function renderPng(node: HTMLElement): Promise<string> {
  const { toPng } = await import("html-to-image");
  // A margin round the picture: the cards otherwise touch its edges.
  const margin = 24;
  const options = {
    pixelRatio: 2,
    backgroundColor: canvasColor(),
    width: node.offsetWidth + 2 * margin,
    height: node.offsetHeight + 2 * margin,
    style: { padding: `${margin}px`, boxSizing: "border-box" },
    filter: (n: HTMLElement) =>
      !(n instanceof HTMLElement && n.dataset.exportSkip !== undefined),
  };
  // WebKit sometimes paints the SVG before its inlined fonts have loaded;
  // a first pass warms them up (html-to-image's documented workaround).
  await toPng(node, options);
  const url = await toPng(node, options);
  const comma = url.indexOf(",");
  if (!url.startsWith("data:image/png") || comma < 0) {
    throw new Error("render did not produce a PNG");
  }
  return url.slice(comma + 1);
}

export async function saveAsImage(
  node: HTMLElement,
  defaultName: string,
  filterName: string,
): Promise<SaveOutcome> {
  try {
    const path = await saveFileDialog({
      defaultPath: defaultName,
      filters: [{ name: filterName, extensions: ["png"] }],
    });
    if (!path) return "cancelled";
    const png = await renderPng(node);
    const target = /\.png$/i.test(path) ? path : `${path}.png`;
    const result = await commands.forkInsightsSaveImage(target, png);
    return result.status === "ok" ? "saved" : "failed";
  } catch (e) {
    console.warn("fork: saving insights as an image failed", e);
    return "failed";
  }
}
