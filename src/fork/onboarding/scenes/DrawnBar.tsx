// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Flow Bar as drawn in scenes and lessons (design polish pass): one
 * drawing for the first-use scenes, the Transforms lesson and the whisper
 * lesson, at the real bar's proportions (Design W4, `pill/geometry.rs`):
 *
 * - `rest`: the 38 × 6 resting line, a touch translucent.
 * - `rec`: the 72 × 30 capsule with eleven 2 px bars 2.5 px apart, tallest
 *   in the middle; a dotted line until `active` (the first audio frame).
 * - `work`: the split capsule, the text on the dark segment and the
 *   eight-spoke spinner on the black end ("Transcribing…").
 * - `done`: a tick, the text and a white action pill (the bar's Fix pill).
 *
 * Text is 15 px like the bar. Decorative unless the caller names it (the
 * lesson's practice bar is a group with a real button in `action`).
 * Motion: the bars and spinner animate `transform` only; Reduce Motion
 * stills them (`drawnBar.css`).
 */
import type { HTMLAttributes, ReactNode } from "react";
import { Check } from "../../ui/icons";
import "./drawnBar.css";

export type DrawnBarState = "rest" | "rec" | "work" | "done";

/** The real bar draws 11 bars (`geometry::BARS`). */
const BARS = 11;
/** Resting envelope, tallest in the middle like the live waveform. */
const ENVELOPE = [0.3, 0.42, 0.56, 0.72, 0.88, 1, 0.88, 0.72, 0.56, 0.42, 0.3];

export interface DrawnBarProps extends HTMLAttributes<HTMLDivElement> {
  state: DrawnBarState;
  /** `rec`: the voice is coming in (bars move); else the dotted line. */
  active?: boolean;
  /** `rec`: a quiet voice (the whisper lesson) moves the bars less. */
  level?: "normal" | "soft";
  /** `work` and `done`: the text on the bar. */
  text?: ReactNode;
  /** `done`: the white action pill (a button, or a span when drawn). */
  action?: ReactNode;
}

export function DrawnBar({
  state,
  active = true,
  level = "normal",
  text,
  action,
  className = "",
  ...rest
}: DrawnBarProps) {
  return (
    <div
      className={`wb-drawn-bar ${className}`}
      data-bar={state}
      data-level={level}
      {...rest}
    >
      {state === "rec" && (
        <span
          className="wb-drawn-wave"
          data-active={active || undefined}
          aria-hidden="true"
        >
          {ENVELOPE.slice(0, BARS).map((env, i) => (
            <i
              key={i}
              style={{
                ["--env" as string]: env,
                animationDelay: `${(i % 6) * 90}ms`,
              }}
            />
          ))}
        </span>
      )}
      {state === "work" && (
        <>
          <span className="wb-drawn-work-text">{text}</span>
          <span className="wb-drawn-work-spin" aria-hidden="true">
            <Spinner />
          </span>
        </>
      )}
      {state === "done" && (
        <>
          <Check size={16} aria-hidden="true" />
          <span className="wb-drawn-done-text">{text}</span>
          {action}
        </>
      )}
    </div>
  );
}

/** The bar's eight-spoke spinner (our own drawing, as in `pill/`). */
function Spinner() {
  return (
    <svg
      className="wb-drawn-spinner"
      viewBox="0 0 16 16"
      aria-hidden
      focusable={false}
    >
      {Array.from({ length: 8 }, (_, i) => (
        <line
          key={i}
          x1="8"
          y1="2"
          x2="8"
          y2="4.6"
          transform={`rotate(${i * 45} 8 8)`}
          style={{ opacity: 0.25 + (0.75 * i) / 7 }}
          data-spoke={i % 2 ? "a" : "b"}
        />
      ))}
    </svg>
  );
}
