// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The bar's spring in closed form, the same function as
 * `geometry::spring` in Rust (and the same constants as `SPRING.bar`):
 * progress 0 → 1 and velocity (per second), `tMs` after a release from
 * rest. The native bridge grows out of the resting line on it from the
 * take's appearance time; a page that paints mid-way starts from the same
 * point of the curve, so the hand-over is seamless.
 */
import { SPRING_PARAMS } from "./geometry";

export function springAt(tMs: number): { x: number; v: number } {
  if (!Number.isFinite(tMs) || tMs <= 0) return { x: 0, v: 0 };
  if (tMs >= SPRING_PARAMS.settleMs) return { x: 1, v: 0 };
  const { stiffness: k, damping: c, mass: m } = SPRING_PARAMS;
  const t = tMs / 1000;
  const w0 = Math.sqrt(k / m);
  const zeta = c / (2 * Math.sqrt(k * m));
  const a = zeta * w0;
  const wd = w0 * Math.sqrt(1 - zeta * zeta);
  const decay = Math.exp(-a * t);
  return {
    x: 1 - decay * (Math.cos(wd * t) + (a / wd) * Math.sin(wd * t)),
    v: decay * ((w0 * w0) / wd) * Math.sin(wd * t),
  };
}
