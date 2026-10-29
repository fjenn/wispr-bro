// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * CSP smoke check (F-08, DEC-43, R5 §4.11). Exercises every source the CSP
 * must allow and one it must block, then reports what happened:
 *
 * - `media`: loads a local audio file (`fork_csp_smoke_media`: newest
 *   recording, else a silent fixture) through `convertFileSrc(…, "asset")`,
 *   the same path Handy's history page uses, and tries to play it;
 * - `dataImage` / `blobImage`: draws an element to a canvas and loads it back
 *   as a `data:` and a `blob:` image ("Save as image", S-25);
 * - `forkCommand`: calls `fork_ping` over IPC;
 * - `probe`: a loopback image that is not in `img-src` must be blocked (proves
 *   the CSP is active at all). Loopback only, so even a missing CSP could not
 *   reach the network.
 *
 * `ok` is true when every allowed check passed, the probe was blocked and no
 * other violation was recorded since the page started. Used by
 * `CspSmoke.tsx` (debug panel) and, as `window.__wbCspSmoke`, by the
 * `WISPRBRO_SELFCHECK` script.
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import { commands } from "../bindings";
import { cspViolations, type CspViolation } from "./cspViolations";

export type CheckState = "ok" | "failed";

export interface CspCheck {
  state: CheckState;
  detail: string;
}

export interface CspSmokeResult {
  ok: boolean;
  /** Content-Security-Policy as the page sees it (meta tag), if any. */
  policy: string | null;
  checks: {
    media: CspCheck;
    dataImage: CspCheck;
    blobImage: CspCheck;
    forkCommand: CspCheck;
    probe: CspCheck;
  };
  /** Violations other than the expected probe block. */
  violations: CspViolation[];
}

/** Port 9 (discard) on loopback: never a real server, never remote. */
export const PROBE_URL = "http://127.0.0.1:9/wb-csp-probe.png";

const TIMEOUT_MS = 4000;

function withTimeout<T>(promise: Promise<T>, label: string): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`${label}: timed out`)), TIMEOUT_MS),
    ),
  ]);
}

const ok = (detail: string): CspCheck => ({ state: "ok", detail });
const failed = (e: unknown): CspCheck => ({
  state: "failed",
  detail: e instanceof Error ? e.message : String(e),
});

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () =>
      reject(new Error(`image did not load (${src.slice(0, 16)}…)`));
    img.src = src;
  });
}

function renderCanvas(): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = 32;
  canvas.height = 16;
  const ctx = canvas.getContext("2d");
  if (ctx) {
    ctx.fillStyle = "#4f46e5";
    ctx.fillRect(0, 0, 32, 16);
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(4, 4, 8, 8);
  }
  return canvas;
}

async function checkMedia(): Promise<CspCheck> {
  const media = await commands.forkCspSmokeMedia();
  if (media.status === "error") throw new Error(JSON.stringify(media.error));
  const src = convertFileSrc(media.data.path, "asset");
  const audio = new Audio();
  audio.preload = "auto";
  audio.muted = true;
  const loaded = new Promise<void>((resolve, reject) => {
    audio.addEventListener("loadedmetadata", () => resolve(), { once: true });
    audio.addEventListener(
      "error",
      () => reject(new Error(`audio error ${audio.error?.code ?? "?"}`)),
      { once: true },
    );
  });
  audio.src = src;
  audio.load();
  await withTimeout(loaded, "audio");
  let played = "loaded";
  try {
    await withTimeout(audio.play(), "play");
    played = "played";
    audio.pause();
  } catch (e) {
    // Autoplay rules may refuse play() without a gesture; loading through
    // the asset protocol is what the CSP decides.
    played = `loaded (play: ${e instanceof Error ? e.name : String(e)})`;
  }
  return ok(`${media.data.source} ${played}, ${audio.duration.toFixed(2)} s`);
}

async function checkDataImage(): Promise<CspCheck> {
  const url = renderCanvas().toDataURL("image/png");
  const img = await withTimeout(loadImage(url), "data image");
  return ok(`${img.naturalWidth}x${img.naturalHeight}`);
}

async function checkBlobImage(): Promise<CspCheck> {
  const blob = await new Promise<Blob>((resolve, reject) =>
    renderCanvas().toBlob(
      (b) => (b ? resolve(b) : reject(new Error("toBlob returned null"))),
      "image/png",
    ),
  );
  const url = URL.createObjectURL(blob);
  try {
    const img = await withTimeout(loadImage(url), "blob image");
    return ok(`${img.naturalWidth}x${img.naturalHeight}`);
  } finally {
    URL.revokeObjectURL(url);
  }
}

async function checkForkCommand(): Promise<CspCheck> {
  const reply = await commands.forkPing();
  if (reply.status === "error") throw new Error(JSON.stringify(reply.error));
  return ok(`fork_ping ${reply.data.message} seq=${reply.data.seq}`);
}

async function checkProbe(): Promise<CspCheck> {
  const before = cspViolations().length;
  try {
    await withTimeout(loadImage(PROBE_URL), "probe");
    return failed(new Error("probe image loaded: the CSP is not enforced"));
  } catch {
    // Give the violation event a moment to be dispatched.
    for (let i = 0; i < 20; i++) {
      const fresh = cspViolations().slice(before);
      if (fresh.some((v) => v.blockedURI.startsWith("http://127.0.0.1:9"))) {
        return ok("blocked by img-src");
      }
      await new Promise((r) => setTimeout(r, 50));
    }
    return failed(new Error("probe failed without a CSP violation"));
  }
}

async function attempt(check: () => Promise<CspCheck>): Promise<CspCheck> {
  try {
    return await check();
  } catch (e) {
    return failed(e);
  }
}

/** Runs every check once. */
export async function runCspSmoke(): Promise<CspSmokeResult> {
  const checks = {
    media: await attempt(checkMedia),
    dataImage: await attempt(checkDataImage),
    blobImage: await attempt(checkBlobImage),
    forkCommand: await attempt(checkForkCommand),
    probe: await attempt(checkProbe),
  };
  const violations = cspViolations().filter(
    (v) => !v.blockedURI.startsWith("http://127.0.0.1:9"),
  );
  const policy =
    document
      .querySelector('meta[http-equiv="Content-Security-Policy"]')
      ?.getAttribute("content") ?? null;
  return {
    ok:
      Object.values(checks).every((c) => c.state === "ok") &&
      violations.length === 0,
    policy,
    checks,
    violations,
  };
}
