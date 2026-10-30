// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * File drops routed by page (S-13, C-205, RE-33).
 *
 * Handy sent every file dropped anywhere on the window to transcription
 * (`FileDropOverlay`), so a `.csv` dropped on an import step was transcribed.
 * In the Hub, a drop only does something over a registered drop zone: the
 * page (or dialog) that owns the zone decides what the files are for. A drop
 * outside every zone is ignored.
 *
 * - `useDropRouter()` (mounted once by `ForkApp`) listens to the webview's
 *   drag-and-drop events and hit-tests their position against the zones.
 * - `useDropZone(ref, options)` registers an element as a zone and returns
 *   whether files are being dragged over it, for the zone's own highlight.
 *
 * When zones overlap (a dialog over a page), the most recently registered one
 * wins, so a dialog's zone takes precedence over the page behind it.
 */
import { useEffect, useRef, useState, type RefObject } from "react";
import { create } from "zustand";
import { getCurrentWebview } from "@tauri-apps/api/webview";

export interface DropZoneOptions {
  /** Called with the dropped paths. */
  onDrop: (paths: string[]) => void | Promise<void>;
  /** Whether the zone takes these paths (default: any). */
  accepts?: (paths: string[]) => boolean;
  /** Stops the zone from taking drops (e.g. while busy). */
  disabled?: boolean;
}

interface Zone {
  id: number;
  element: HTMLElement;
  options: RefObject<DropZoneOptions>;
}

interface DropRouterState {
  /** Id of the zone under a drag that it accepts, or null. */
  activeZone: number | null;
}

const useDropRouterStore = create<DropRouterState>()(() => ({
  activeZone: null,
}));

const zones: Zone[] = [];
let nextZoneId = 1;

/** Webview position (physical px) → CSS px. */
function toCssPoint(position: { x: number; y: number }) {
  const scale = window.devicePixelRatio || 1;
  return { x: position.x / scale, y: position.y / scale };
}

/** The topmost zone that contains `point` and accepts `paths`, if any. */
export function zoneAt(
  point: { x: number; y: number },
  paths: string[],
): Zone | null {
  for (let i = zones.length - 1; i >= 0; i--) {
    const zone = zones[i];
    const options = zone.options.current;
    if (!options || options.disabled || !zone.element.isConnected) continue;
    const r = zone.element.getBoundingClientRect();
    const inside =
      point.x >= r.left &&
      point.x < r.right &&
      point.y >= r.top &&
      point.y < r.bottom;
    if (!inside) continue;
    if (options.accepts && !options.accepts(paths)) continue;
    return zone;
  }
  return null;
}

/** Listens to the webview's drag-and-drop events. Mount once. */
export function useDropRouter(): void {
  useEffect(() => {
    let paths: string[] = [];
    const set = (zone: Zone | null) =>
      useDropRouterStore.setState({ activeZone: zone?.id ?? null });

    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      switch (payload.type) {
        case "enter":
          paths = payload.paths;
          set(zoneAt(toCssPoint(payload.position), paths));
          break;
        case "over":
          set(zoneAt(toCssPoint(payload.position), paths));
          break;
        case "leave":
          paths = [];
          set(null);
          break;
        case "drop": {
          const dropped = payload.paths.length > 0 ? payload.paths : paths;
          paths = [];
          set(null);
          if (dropped.length === 0) return;
          const zone = zoneAt(toCssPoint(payload.position), dropped);
          if (!zone) {
            // RE-33: nothing under the pointer wants files; never transcribe.
            console.info("fork: file drop outside any drop zone ignored");
            return;
          }
          void Promise.resolve(zone.options.current?.onDrop(dropped)).catch(
            (e) => console.error("fork: drop handler failed", e),
          );
          break;
        }
      }
    });
    return () => {
      void unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, []);
}

/**
 * Makes `ref`'s element a drop zone while mounted. Returns `true` while
 * files it accepts are dragged over it.
 */
export function useDropZone(
  ref: RefObject<HTMLElement | null>,
  options: DropZoneOptions,
): boolean {
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const [id, setId] = useState<number | null>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const zone: Zone = { id: nextZoneId++, element, options: optionsRef };
    zones.push(zone);
    setId(zone.id);
    return () => {
      const index = zones.indexOf(zone);
      if (index >= 0) zones.splice(index, 1);
      if (useDropRouterStore.getState().activeZone === zone.id) {
        useDropRouterStore.setState({ activeZone: null });
      }
      setId(null);
    };
  }, [ref]);

  const active = useDropRouterStore((s) => s.activeZone);
  return id !== null && active === id;
}
