// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Bar sizes and the bar's spring, generated from
 * `src-tauri/src/fork/pill/geometry.rs` (one source of truth; a Rust test
 * fails when the JSON is stale).
 *
 * - `shapeSize`: the window (the web view is exactly this size).
 * - `capsuleSize`: the capsule inside it. Smaller only for the labelled
 *   shapes, whose label pill (`LABEL.h` tall, `LABEL.gap` away) floats above
 *   the capsule, or below it when the bar sits at the top of the display.
 */
import type { Shape } from "../bindings";
import geometry from "./geometry.gen.json";

export type Size = { w: number; h: number };

export const SHAPES = geometry.shapes as Record<Shape, Size>;
export const CAPSULES = geometry.capsules as Record<Shape, Size>;
export const LABEL: { h: number; gap: number } = geometry.label;
export const BARS: number = geometry.bars;
export const SPRING_PARAMS: {
  stiffness: number;
  damping: number;
  mass: number;
  settleMs: number;
} = geometry.spring;

export function shapeSize(shape: Shape): Size {
  return SHAPES[shape];
}

export function capsuleSize(shape: Shape): Size {
  return CAPSULES[shape];
}

export function labelled(shape: Shape): boolean {
  const s = SHAPES[shape];
  const c = CAPSULES[shape];
  return s.w !== c.w || s.h !== c.h;
}

/** Corner radius of a capsule `h` tall: round ends, cards 18 px. */
export function capsuleRadius(h: number): number {
  return Math.min(h / 2, 18);
}
