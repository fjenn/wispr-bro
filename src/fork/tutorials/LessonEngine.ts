// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The lesson engine's state machine (S-31, B-40). A lesson is data: two
 * ordered lists of steps, **Try it** (scenes, then a practice step with
 * success detection, then the feature's "Turn on X") and an optional
 * **How it works** (animated scenes, no video, DEC-15). The engine knows
 * nothing about Transforms; S-32 (whisper) and N-12 (Notetaker tour) are
 * further lists of steps on the same machine.
 *
 * Rules:
 * - a `gated` step (practice) lets the user continue only once it reported
 *   success (`complete`) or was skipped; a gated step is `skippable` unless
 *   it says otherwise, so a lesson never dead-ends (S-32 exit criterion);
 * - Continue on the last Try step finishes the lesson (`finished`); on the
 *   last How step it moves to the first Try step ("Try it yourself");
 * - Back goes one step back within the current list; from the first Try
 *   step it returns to the How list's last step if the user came from there.
 *
 * Pure functions only (unit-tested in `LessonEngine.test.ts`); the modal
 * (`LessonModal.tsx`) holds the state in a reducer.
 */

export type LessonMode = "try" | "how";

/** What the engine needs to know about a step. */
export interface LessonStepShape {
  /** Stable within its lesson (`a-z0-9_-`). */
  id: string;
  /** Continue waits for `complete` (or Skip). */
  gated?: boolean;
  /** Offer Skip while gated (default true). */
  skippable?: boolean;
}

export interface LessonShape {
  try: readonly LessonStepShape[];
  how?: readonly LessonStepShape[];
}

export interface LessonState {
  mode: LessonMode;
  index: number;
  /** Steps that reported success, as `mode:id`. */
  done: readonly string[];
  /** Gated steps the user skipped, as `mode:id`. */
  skipped: readonly string[];
  /** The user reached Try from How (Back may return there). */
  cameFromHow: boolean;
  finished: boolean;
}

export type LessonAction =
  | { type: "open"; mode: LessonMode }
  | { type: "next" }
  | { type: "back" }
  | { type: "complete"; stepId: string }
  | { type: "skip" }
  | { type: "mode"; mode: LessonMode };

function steps(
  shape: LessonShape,
  mode: LessonMode,
): readonly LessonStepShape[] {
  return mode === "how" ? (shape.how ?? []) : shape.try;
}

const key = (mode: LessonMode, id: string) => `${mode}:${id}`;

export function initialLessonState(
  shape: LessonShape,
  mode: LessonMode = "try",
): LessonState {
  return {
    mode: mode === "how" && (shape.how?.length ?? 0) > 0 ? "how" : "try",
    index: 0,
    done: [],
    skipped: [],
    cameFromHow: false,
    finished: false,
  };
}

export function currentStep(
  shape: LessonShape,
  state: LessonState,
): LessonStepShape | undefined {
  return steps(shape, state.mode)[state.index];
}

export function stepDone(shape: LessonShape, state: LessonState): boolean {
  const step = currentStep(shape, state);
  return !!step && state.done.includes(key(state.mode, step.id));
}

export function canContinue(shape: LessonShape, state: LessonState): boolean {
  const step = currentStep(shape, state);
  if (!step || state.finished) return false;
  const id = key(state.mode, step.id);
  return !step.gated || state.done.includes(id) || state.skipped.includes(id);
}

export function canSkip(shape: LessonShape, state: LessonState): boolean {
  const step = currentStep(shape, state);
  return (
    !!step &&
    !!step.gated &&
    step.skippable !== false &&
    !canContinue(shape, state)
  );
}

export function canGoBack(shape: LessonShape, state: LessonState): boolean {
  if (state.finished) return false;
  return (
    state.index > 0 ||
    (state.mode === "try" && state.cameFromHow && (shape.how?.length ?? 0) > 0)
  );
}

/** The last step of the current list. */
export function isLastStep(shape: LessonShape, state: LessonState): boolean {
  return state.index === steps(shape, state.mode).length - 1;
}

/** "Step n of m" within the current list. */
export function progress(
  shape: LessonShape,
  state: LessonState,
): { step: number; total: number } {
  return { step: state.index + 1, total: steps(shape, state.mode).length };
}

export function lessonReducer(
  shape: LessonShape,
  state: LessonState,
  action: LessonAction,
): LessonState {
  switch (action.type) {
    case "open":
      return initialLessonState(shape, action.mode);
    case "mode":
      if (action.mode === "how" && !(shape.how?.length ?? 0)) return state;
      return {
        ...state,
        mode: action.mode,
        index: 0,
        cameFromHow: action.mode === "try" && state.mode === "how",
        finished: false,
      };
    case "complete": {
      const step = currentStep(shape, state);
      if (!step || step.id !== action.stepId) {
        // Success from a step that is no longer shown counts for it anyway.
        const known = steps(shape, state.mode).some(
          (s) => s.id === action.stepId,
        );
        if (!known) return state;
      }
      const id = key(state.mode, action.stepId);
      if (state.done.includes(id)) return state;
      return { ...state, done: [...state.done, id] };
    }
    case "skip": {
      if (!canSkip(shape, state)) return state;
      const step = currentStep(shape, state)!;
      return {
        ...state,
        skipped: [...state.skipped, key(state.mode, step.id)],
      };
    }
    case "next": {
      if (!canContinue(shape, state)) return state;
      if (!isLastStep(shape, state))
        return { ...state, index: state.index + 1 };
      if (state.mode === "how") {
        return { ...state, mode: "try", index: 0, cameFromHow: true };
      }
      return { ...state, finished: true };
    }
    case "back": {
      if (!canGoBack(shape, state)) return state;
      if (state.index > 0) return { ...state, index: state.index - 1 };
      const how = shape.how ?? [];
      return {
        ...state,
        mode: "how",
        index: how.length - 1,
        cameFromHow: false,
      };
    }
  }
}
