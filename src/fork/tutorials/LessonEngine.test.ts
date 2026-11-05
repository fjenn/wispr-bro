// SPDX-License-Identifier: GPL-3.0-or-later
// Run: bun src/fork/tutorials/LessonEngine.test.ts
// S-31: the lesson state machine. Gated practice steps, Skip (never a dead
// end), finishing, How it works → Try it, Back across both lists.
import assert from "node:assert/strict";
import {
  canContinue,
  canGoBack,
  canSkip,
  currentStep,
  initialLessonState,
  isLastStep,
  lessonReducer,
  progress,
  stepDone,
  type LessonAction,
  type LessonShape,
  type LessonState,
} from "./LessonEngine";

const shape: LessonShape = {
  try: [{ id: "intro" }, { id: "practice", gated: true }],
  how: [{ id: "one" }, { id: "two" }, { id: "three" }],
};
const run = (state: LessonState, ...actions: LessonAction[]) =>
  actions.reduce((s, a) => lessonReducer(shape, s, a), state);

// Try it: intro → practice (gated) → finished.
let s = initialLessonState(shape, "try");
assert.equal(currentStep(shape, s)?.id, "intro");
assert.deepEqual(progress(shape, s), { step: 1, total: 2 });
assert.ok(canContinue(shape, s));
assert.ok(!canGoBack(shape, s));
assert.ok(!canSkip(shape, s), "an ungated step has nothing to skip");
s = run(s, { type: "next" });
assert.equal(currentStep(shape, s)?.id, "practice");
assert.ok(isLastStep(shape, s));
assert.ok(!canContinue(shape, s), "practice waits for success");
assert.ok(canSkip(shape, s), "but can always be skipped");
assert.equal(run(s, { type: "next" }), s, "Continue does nothing yet");
s = run(s, { type: "complete", stepId: "practice" });
assert.ok(stepDone(shape, s));
assert.ok(canContinue(shape, s));
assert.ok(!canSkip(shape, s));
assert.equal(run(s, { type: "complete", stepId: "practice" }), s, "idempotent");
s = run(s, { type: "next" });
assert.ok(s.finished);
assert.ok(!canContinue(shape, s));
assert.ok(!canGoBack(shape, s));

// Skip instead of success: Continue then finishes, nothing marked done.
s = run(initialLessonState(shape), { type: "next" }, { type: "skip" });
assert.ok(canContinue(shape, s));
assert.ok(!stepDone(shape, s));
assert.ok(run(s, { type: "next" }).finished);

// A gated step that may not be skipped.
const strict: LessonShape = {
  try: [{ id: "p", gated: true, skippable: false }],
};
const t0 = initialLessonState(strict);
assert.ok(!canSkip(strict, t0));
assert.equal(lessonReducer(strict, t0, { type: "skip" }), t0);

// Unknown step ids are ignored.
s = initialLessonState(shape);
assert.equal(run(s, { type: "complete", stepId: "nope" }), s);

// How it works: three scenes, then the first Try step; Back returns there.
s = initialLessonState(shape, "how");
assert.equal(s.mode, "how");
assert.deepEqual(progress(shape, s), { step: 1, total: 3 });
s = run(s, { type: "next" }, { type: "next" });
assert.equal(currentStep(shape, s)?.id, "three");
s = run(s, { type: "back" });
assert.equal(currentStep(shape, s)?.id, "two");
s = run(s, { type: "next" }, { type: "next" });
assert.equal(s.mode, "try");
assert.equal(currentStep(shape, s)?.id, "intro");
assert.ok(canGoBack(shape, s), "Back returns to How it works");
s = run(s, { type: "back" });
assert.equal(s.mode, "how");
assert.equal(currentStep(shape, s)?.id, "three");

// Switching lists by hand.
s = run(initialLessonState(shape, "how"), { type: "mode", mode: "try" });
assert.equal(s.mode, "try");
assert.ok(s.cameFromHow);
// A lesson without How it works opens on Try it and ignores the switch.
const tryOnly: LessonShape = { try: [{ id: "a" }] };
const u = initialLessonState(tryOnly, "how");
assert.equal(u.mode, "try");
assert.equal(lessonReducer(tryOnly, u, { type: "mode", mode: "how" }), u);
assert.ok(!canGoBack(tryOnly, u));

// Reopening starts over.
s = run(
  initialLessonState(shape),
  { type: "next" },
  { type: "complete", stepId: "practice" },
);
s = run(s, { type: "open", mode: "try" });
assert.equal(s.index, 0);
assert.deepEqual(s.done, []);
console.log("LessonEngine: all assertions passed");
