// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The active Flow Bar (S-03): held, hands-free, processing, messages;
 * S-07a adds "Setting up" and the card. Design W4 restyles it after Wispr
 * Flow's bar: an opaque black capsule, 15 px text, a label pill that blurs
 * in above the capsule, and size changes that morph on the bar's spring.
 *
 * The window is exactly the size of the shape it shows (sizes from Rust,
 * `geometry.gen.json`); the backend resizes it once this page has painted
 * a new state. The capsule sits on the window's resting edge (its bottom,
 * or its top when the bar is at the top of the display), so it stays put
 * while the window grows or shrinks around it:
 *
 * - Growing: the capsule morphs once the window has its new size.
 * - Shrinking: the capsule (and a leaving label) morphs inside the old
 *   window first; the page reports the state painted only then, so the
 *   window never clips it.
 * - From rest: the native bridge grows out of the resting line from the
 *   take's appearance time (`grown_at_unix_ms`); this page starts from the
 *   same point of the same curve (`spring.ts`), so the hand-over is seamless.
 *
 * Clicks work without taking focus from the frontmost app (non-activating
 * panel, `acceptsFirstMouse`; RS-03). Hover is not used here: a web view
 * gets no mouse-moved events while its app is inactive.
 */
import { useEffect, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  animate,
  AnimatePresence,
  motion,
  useMotionValue,
  useReducedMotion,
  useTransform,
  type AnimationPlaybackControls,
} from "motion/react";
import { getLanguageDirection } from "@/lib/utils/rtl";
import type { ForkPillState, Shape } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { DUR, EASE, SPRING } from "../ui/motion";
import {
  capsuleRadius,
  capsuleSize,
  SPRING_PARAMS,
  stateCapsule,
  stateSize,
  type Size,
} from "./geometry";
import { springAt } from "./spring";
import { Card } from "./states/Card";
import { DiffCard } from "./states/DiffCard";
import { BarLabel, barLabel } from "./states/Label";
import { Listening } from "./states/Listening";
import { Message } from "./states/Message";
import { SettingUp } from "./states/SettingUp";
import { isShortcutLost, ShortcutLost } from "./states/ShortcutLost";
import { Working } from "./states/Working";
import { usePillState, useReportPainted } from "./usePill";

/** How long a shrink plays before the page reports the state painted. */
const SHRINK_MS = 260;
/** A growing capsule waits at most this long for the window's new size. */
const WINDOW_WAIT_MS = 150;

export function Pill() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const state = usePillState();
  const reduced = useReducedMotion() ?? false;
  const painted = useRef<{ seq: number; size: Size } | null>(null);

  useEffect(() => {
    document.documentElement.dir = getLanguageDirection(i18n.language);
    document.documentElement.lang = i18n.language;
  }, [i18n.language]);

  // A state that makes the window smaller is reported once its shrink has
  // played inside the old window.
  let delay = 0;
  if (state && painted.current && painted.current.seq !== state.seq) {
    const before = painted.current.size;
    const after = stateSize(state);
    if (!reduced && (after.w < before.w || after.h < before.h)) {
      delay = SHRINK_MS;
    }
  }
  useReportPainted(state?.seq ?? null, delay);
  useEffect(() => {
    if (state) painted.current = { seq: state.seq, size: stateSize(state) };
    else painted.current = null;
  }, [state]);

  useEffect(() => {
    document.documentElement.dataset.edge = state?.top ? "top" : "bottom";
  }, [state?.top]);

  if (!state) return <div data-testid="wb-pill-idle" />;

  const size = stateSize(state);
  const label = barLabel(state, t);
  let body: ReactNode;
  switch (state.phase) {
    case "held":
    case "locked":
      body = <Listening state={state} label={label} />;
      break;
    case "transcribing":
    case "cleaning_up":
      body = <Working state={state} />;
      break;
    case "message":
      body = isShortcutLost(state) ? (
        <ShortcutLost state={state} />
      ) : (
        <Message state={state} />
      );
      break;
    case "setting_up":
      body = <SettingUp state={state} />;
      break;
    case "card":
      body = <Card state={state} />;
      break;
    case "diff":
      body = <DiffCard state={state} />;
      break;
  }

  return (
    <div
      className="wb-pill"
      role="group"
      aria-label={t("pill.region")}
      data-testid="wb-pill"
      data-phase={state.phase}
      data-shape={state.shape}
      data-tone={state.tone ?? undefined}
      data-edge={state.top ? "top" : "bottom"}
      style={{ width: size.w, height: size.h }}
    >
      <Capsule
        shape={state.shape}
        capsule={stateCapsule(state)}
        window={size}
        grownAt={state.grown_at_unix_ms}
        reduced={reduced}
        contentKey={state.phase}
        label={
          <AnimatePresence initial={!reduced}>
            {label && (
              <motion.div
                key="label"
                className="wb-pill-label-slot"
                style={{ maxWidth: size.w, x: "-50%" }}
                initial={
                  reduced
                    ? { opacity: 0 }
                    : { opacity: 0, filter: "blur(6px)", scale: 0.96 }
                }
                animate={{
                  opacity: 1,
                  filter: "blur(0px)",
                  scale: 1,
                  transition: reduced
                    ? { duration: DUR.reduced, ease: "linear" }
                    : { duration: DUR.base, ease: EASE.out },
                }}
                exit={{
                  opacity: 0,
                  filter: reduced ? "blur(0px)" : "blur(4px)",
                  transition: reduced
                    ? { duration: DUR.reduced, ease: "linear" }
                    : { duration: DUR.leave, ease: EASE.in },
                }}
              >
                <BarLabel label={label} />
              </motion.div>
            )}
          </AnimatePresence>
        }
      >
        {body}
      </Capsule>
    </div>
  );
}

/**
 * The black capsule. Its size follows the state's capsule on the bar's
 * spring (Reduce Motion: at once); the content is laid out at the target
 * size and revealed as the capsule grows.
 */
function Capsule({
  shape,
  capsule: target,
  window: win,
  grownAt,
  reduced,
  contentKey,
  label,
  children,
}: {
  shape: Shape;
  capsule: Size;
  window: Size;
  grownAt: number | null;
  reduced: boolean;
  contentKey: string;
  label: ReactNode;
  children: ReactNode;
}) {
  const first = useRef(true);
  const start = useRef(initialSize(target, grownAt, reduced));
  const w = useMotionValue(start.current.w);
  const h = useMotionValue(start.current.h);
  const radius = useTransform(h, capsuleRadius);

  useEffect(() => {
    const runs: AnimationPlaybackControls[] = [];
    const isFirst = first.current;
    first.current = false;
    if (reduced) {
      w.set(target.w);
      h.set(target.h);
      return;
    }
    const spring = { ...SPRING.bar };
    if (isFirst) {
      // Continue the bridge's grow out of the resting line, if it runs.
      const s = start.current;
      if (s.w === target.w && s.h === target.h) return;
      const v = s.progressVelocity;
      runs.push(
        animate(w, target.w, { ...spring, velocity: s.dw * v }),
        animate(h, target.h, { ...spring, velocity: s.dh * v }),
      );
      return () => runs.forEach((r) => r.stop());
    }
    if (w.get() === target.w && h.get() === target.h) return;
    // Grow once the window has its new size (the backend resizes it after
    // this state is painted); shrink at once, inside the old window.
    let timer = 0;
    const go = () => {
      window.removeEventListener("resize", check);
      window.clearTimeout(timer);
      runs.push(animate(w, target.w, spring), animate(h, target.h, spring));
    };
    const check = () => {
      if (window.innerWidth >= win.w - 0.5 && window.innerHeight >= win.h - 0.5)
        go();
    };
    const growing = target.w > w.get() || target.h > h.get();
    if (!growing) go();
    else {
      window.addEventListener("resize", check);
      timer = window.setTimeout(go, WINDOW_WAIT_MS);
      check();
    }
    return () => {
      window.removeEventListener("resize", check);
      window.clearTimeout(timer);
      runs.forEach((r) => r.stop());
    };
    // `target` and `win` follow the state's shape and size.
  }, [shape, target.w, target.h, reduced]);

  return (
    <div className="wb-pill-anchor">
      {label}
      <motion.div
        className="wb-pill-capsule"
        data-testid="wb-pill-capsule"
        style={{ width: w, height: h, borderRadius: radius }}
      >
        <motion.div
          key={contentKey}
          className="wb-pill-content"
          style={{ width: target.w, height: target.h }}
          initial={reduced || first.current ? false : { opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: DUR.fast, delay: 0.04, ease: EASE.out }}
        >
          {children}
        </motion.div>
      </motion.div>
    </div>
  );
}

/**
 * Where a newly mounted capsule starts: on the bridge's curve when the
 * take's grow from the resting line is still running, else at its size.
 */
function initialSize(
  target: Size,
  grownAt: number | null,
  reduced: boolean,
): Size & { dw: number; dh: number; progressVelocity: number } {
  // A stamp slightly ahead of this clock (both read the wall clock, from
  // two processes) counts as the start of the grow.
  const raw = grownAt == null ? Infinity : Date.now() - grownAt;
  const elapsed = raw < 0 && raw > -1000 ? 0 : raw;
  if (reduced || !(elapsed >= 0 && elapsed < SPRING_PARAMS.settleMs)) {
    return { ...target, dw: 0, dh: 0, progressVelocity: 0 };
  }
  const rest = capsuleSize("rest");
  const { x, v } = springAt(elapsed);
  const dw = target.w - rest.w;
  const dh = target.h - rest.h;
  return {
    w: rest.w + dw * x,
    h: rest.h + dh * x,
    dw,
    dh,
    progressVelocity: v,
  };
}
