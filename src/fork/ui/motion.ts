// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The fork's motion layer (Design Phase 0, SYSTEM-PLAN §1.6 and §5) on
 * `motion` (MIT, the successor of Framer Motion; React API from
 * `motion/react`). Components never hand-roll durations or springs: they
 * take a preset from here, so every surface moves the same way and Reduce
 * Motion is handled in one place.
 *
 * Character: quick ease-outs and critically damped springs. Small things
 * (press, hover, chips) settle in 90-250 ms; large surfaces (dialogs, sheets,
 * pages) in 300-450 ms with no overshoot. The reference recordings are too
 * slow (0.5-1.2 fps) to measure these; the values are our own call
 * (REFERENCE-SPEC §8).
 *
 * Reduce Motion (`prefers-reduced-motion: reduce`): every preset becomes a
 * 120 ms opacity crossfade; no scale, slide, blur, path drawing, number
 * rolling or stagger. `useMotionPresets()` reads the setting; `MotionRoot`
 * (wrap each window's root) also tells `motion` itself to honour it.
 *
 * ```tsx
 * const m = useMotionPresets();
 * <motion.button {...m.press}>…</motion.button>
 * <AnimatePresence>{open && <motion.div {...m.dialog.panel} />}</AnimatePresence>
 * ```
 */
import { createElement, type ReactNode } from "react";
import {
  MotionConfig,
  useReducedMotion,
  type TargetAndTransition,
  type Transition,
  type Variants,
} from "motion/react";

// ---- Durations (seconds) and easings, mirrored as CSS tokens in tokens.css.

export const DUR = {
  instant: 0.09,
  fast: 0.14,
  base: 0.22,
  slow: 0.32,
  stage: 0.08,
  reduced: 0.12,
  /** Leaving after a `base` entrance (scrims, the bar's label, a row
   *  collapsing): ≈ 0.7-0.8 × the enter time. */
  leave: 0.16,
  /** A meter or bar growing to its value (the celebration's speed bars). */
  grow: 0.6,
  /** A path drawing itself (celebration arrow). */
  draw: 0.7,
} as const;

export const EASE = {
  out: [0.22, 1, 0.36, 1],
  in: [0.55, 0, 1, 0.45],
  inOut: [0.65, 0, 0.35, 1],
} as const satisfies Record<string, [number, number, number, number]>;

// ---- Named springs.

export const SPRING = {
  /** Buttons, toggles, chips, list reorder: ~200 ms, no visible overshoot. */
  snappy: { type: "spring", stiffness: 520, damping: 42, mass: 1 },
  /** Popovers, cards, tooltips: ~260 ms, critically damped. */
  soft: { type: "spring", stiffness: 380, damping: 38, mass: 1 },
  /** Dialogs, sheets, pages: ~380 ms, no bounce on large surfaces. */
  gentle: { type: "spring", stiffness: 300, damping: 34, mass: 1 },
  /** Flow Bar size morphs (W4). */
  bar: { type: "spring", stiffness: 600, damping: 38, mass: 0.8 },
  /** Celebration stickers: the one place a little overshoot is wanted. */
  pop: { type: "spring", stiffness: 460, damping: 22, mass: 0.9 },
} as const satisfies Record<string, Transition>;

/** Leaving is quicker than entering (≈ 0.7 × the enter time). */
const exitFast: Transition = { duration: DUR.fast, ease: EASE.in };
const reducedFade: Transition = { duration: DUR.reduced, ease: "linear" };

/** Props for a `motion.*` element: initial/animate/exit and a transition. */
export interface Preset {
  initial?: TargetAndTransition | false;
  animate?: TargetAndTransition;
  exit?: TargetAndTransition;
  transition?: Transition;
}

const FADE_REDUCED: Preset = {
  initial: { opacity: 0 },
  animate: { opacity: 1, transition: reducedFade },
  exit: { opacity: 0, transition: reducedFade },
};

/** Press feedback for a `motion.button` (spread it). */
export interface PressPreset {
  whileTap?: TargetAndTransition;
  transition?: Transition;
}

export interface MotionPresets {
  reduced: boolean;
  /** Buttons and chips: scale 0.97 on press, snappy spring back. */
  press: PressPreset;
  /** Colour/background hover, as a CSS transition string (140 ms). */
  hoverTransition: string;
  /** Dialog: the scrim fades, the panel springs up from 0.97 / 8 px. */
  dialog: { scrim: Preset; panel: Preset };
  /** Large sheet (lessons, chooser): rises 24 px, no bounce. */
  sheet: { scrim: Preset; panel: Preset };
  /** Popover, menu, select list: a small drop from its trigger. */
  popover: Preset;
  /** Tooltip: a quick fade and 2 px rise. */
  tooltip: Preset;
  /** Hub page change, inside the content panel only. */
  page: Preset;
  /** Onboarding step change: slides 24 px in the direction of travel. */
  step: (direction: 1 | -1) => Variants;
  /** Staged reveal: container + item variants (80 ms apart, max 6). */
  stagger: { container: Variants; item: Variants };
  /** Celebration choreography (W1): stage delays and the parts. */
  celebration: {
    /** Seconds from the start for headline, numbers, arrow, sticker, caption, button. */
    stages: readonly number[];
    headline: Preset;
    pop: Preset;
    /** For an SVG path: draws itself (`pathLength` 0 → 1, 700 ms). */
    draw: Preset;
  };
  /** Row actions revealed on hover (140 ms fade). */
  reveal: Preset;
  /** Collapsible height (accordion rows, banners leaving). */
  collapse: Preset;
}

/** The presets for a given Reduce Motion state. Pure, for tests. */
export function motionPresets(reduced: boolean): MotionPresets {
  if (reduced) {
    const none: Variants = {
      hidden: { opacity: 0 },
      shown: { opacity: 1, transition: reducedFade },
      gone: { opacity: 0, transition: reducedFade },
    };
    return {
      reduced: true,
      press: {},
      hoverTransition: "none",
      dialog: { scrim: FADE_REDUCED, panel: FADE_REDUCED },
      sheet: { scrim: FADE_REDUCED, panel: FADE_REDUCED },
      popover: FADE_REDUCED,
      tooltip: FADE_REDUCED,
      page: FADE_REDUCED,
      step: () => none,
      stagger: {
        container: { hidden: {}, shown: {} },
        item: none,
      },
      celebration: {
        stages: [0, 0, 0, 0, 0, 0],
        headline: FADE_REDUCED,
        pop: FADE_REDUCED,
        // A still, fully drawn path.
        draw: { initial: false, animate: { pathLength: 1, opacity: 1 } },
      },
      reveal: FADE_REDUCED,
      collapse: FADE_REDUCED,
    };
  }

  const scrim: Preset = {
    initial: { opacity: 0 },
    animate: { opacity: 1, transition: { duration: DUR.base, ease: EASE.out } },
    exit: { opacity: 0, transition: { duration: DUR.leave, ease: EASE.in } },
  };
  return {
    reduced: false,
    press: { whileTap: { scale: 0.97 }, transition: SPRING.snappy },
    hoverTransition: `color ${DUR.fast}s, background-color ${DUR.fast}s, border-color ${DUR.fast}s, box-shadow ${DUR.fast}s, opacity ${DUR.fast}s`,
    dialog: {
      scrim,
      panel: {
        initial: { opacity: 0, scale: 0.97, y: 8 },
        animate: { opacity: 1, scale: 1, y: 0, transition: SPRING.gentle },
        exit: { opacity: 0, scale: 0.98, y: 0, transition: exitFast },
      },
    },
    sheet: {
      scrim,
      panel: {
        initial: { opacity: 0, y: 24, scale: 0.985 },
        animate: { opacity: 1, y: 0, scale: 1, transition: SPRING.gentle },
        exit: {
          opacity: 0,
          y: 12,
          transition: { duration: 0.2, ease: EASE.in },
        },
      },
    },
    popover: {
      initial: { opacity: 0, scale: 0.98, y: -4 },
      animate: { opacity: 1, scale: 1, y: 0, transition: SPRING.soft },
      exit: { opacity: 0, scale: 0.98, transition: { duration: 0.1 } },
    },
    tooltip: {
      initial: { opacity: 0, y: 2 },
      animate: {
        opacity: 1,
        y: 0,
        transition: { duration: DUR.fast, ease: EASE.out },
      },
      exit: { opacity: 0, transition: { duration: 0.08 } },
    },
    page: {
      initial: { opacity: 0, y: 6 },
      animate: {
        opacity: 1,
        y: 0,
        transition: { duration: DUR.base, ease: EASE.out },
      },
      exit: { opacity: 0, transition: { duration: 0.12, ease: EASE.in } },
    },
    step: (direction) => ({
      hidden: { opacity: 0, x: 24 * direction },
      shown: {
        opacity: 1,
        x: 0,
        transition: { duration: DUR.slow, ease: EASE.out },
      },
      gone: {
        opacity: 0,
        x: -24 * direction,
        transition: { duration: 0.2, ease: EASE.in },
      },
    }),
    stagger: {
      container: {
        hidden: {},
        shown: { transition: { staggerChildren: DUR.stage } },
      },
      item: {
        hidden: { opacity: 0, y: 6 },
        shown: {
          opacity: 1,
          y: 0,
          transition: { duration: DUR.base, ease: EASE.out },
        },
      },
    },
    celebration: {
      stages: [0, 0.6, 1.2, 1.9, 2.5, 2.9],
      headline: {
        initial: { opacity: 0, y: 12 },
        animate: {
          opacity: 1,
          y: 0,
          transition: { duration: 0.5, ease: EASE.out },
        },
      },
      pop: {
        initial: { opacity: 0, scale: 0.6 },
        animate: { opacity: 1, scale: 1, transition: SPRING.pop },
      },
      draw: {
        initial: { pathLength: 0, opacity: 0 },
        animate: {
          pathLength: 1,
          opacity: 1,
          transition: { duration: DUR.draw, ease: EASE.inOut },
        },
      },
    },
    reveal: {
      initial: { opacity: 0 },
      animate: { opacity: 1, transition: { duration: DUR.fast } },
      exit: { opacity: 0, transition: { duration: 0.1 } },
    },
    collapse: {
      initial: { opacity: 0, height: 0 },
      animate: {
        opacity: 1,
        height: "auto",
        transition: { duration: DUR.base, ease: EASE.out },
      },
      exit: {
        opacity: 0,
        height: 0,
        transition: { duration: DUR.leave, ease: EASE.in },
      },
    },
  };
}

const FULL = motionPresets(false);
const REDUCED = motionPresets(true);

/** Delay (s) of the `index`-th item of a staged reveal: 80 ms apart, max 6. */
export function staggerDelay(index: number, reduced = false): number {
  return reduced ? 0 : Math.min(Math.max(index, 0), 6) * DUR.stage;
}

/** The presets for the user's current Reduce Motion setting. */
export function useMotionPresets(): MotionPresets {
  return useReducedMotion() ? REDUCED : FULL;
}

/**
 * Wrap a window's React root: `motion` then skips transform and layout
 * animations by itself under Reduce Motion, as a second line of defence for
 * any animation that does not come from a preset.
 */
export function MotionRoot({ children }: { children: ReactNode }) {
  return createElement(MotionConfig, { reducedMotion: "user" }, children);
}
