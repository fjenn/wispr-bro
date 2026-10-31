// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Pill sizes, generated from `src-tauri/src/fork/pill/geometry.rs` (one
 * source of truth; a Rust test fails when the JSON is stale).
 */
import type { Shape } from "../bindings";
import geometry from "./geometry.gen.json";

export type Size = { w: number; h: number };

export const SHAPES = geometry.shapes as Record<Shape, Size>;
export const BARS: number = geometry.bars;

export function shapeSize(shape: Shape): Size {
  return SHAPES[shape];
}
