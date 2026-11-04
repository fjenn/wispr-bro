// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The lesson sheet (S-31, B-39, B-40; Design W5): one `Sheet` for every
 * lesson. Top: Back (when there is a step to go back to), the progress
 * dashes ("Step n of m" for screen readers) and close (Esc closes too).
 * Then a centred serif heading with one italic word, one line under it,
 * and the step, which renders itself (`Lesson.steps[i].render`). The
 * centred footer has Skip (a gated step, never a dead end) and the black
 * primary button: Continue, "Try it yourself" on the last How it works
 * scene, and the feature's "Turn on X" (or Done when it is on) on the last
 * Try it step. The sheet's name says the lesson and the mode (Try it out /
 * How it works).
 *
 * Focus moves to the step's heading on every step change, and the heading
 * is a polite live region, so the success text ("Text rewritten…") is
 * heard. A new step fades in (a crossfade under Reduce Motion). When the
 * lesson closes after a practice step succeeded, its checklist item ticks
 * (`lessonRegistry.tsx`).
 */
import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { Sheet } from "../ui/Dialog";
import { FORK_NAMESPACE } from "../i18n";
import { markChecklistItem } from "../hub/SetupChecklist";
import { Button } from "../ui";
import { useMotionPresets } from "../ui/motion";
import {
  canContinue,
  canGoBack,
  canSkip,
  initialLessonState,
  isLastStep,
  lessonReducer,
  progress,
  stepDone,
  type LessonAction,
  type LessonMode,
  type LessonShape,
  type LessonState,
} from "./LessonEngine";
import {
  closeLesson,
  getLesson,
  useLessonStore,
  type Lesson,
} from "./lessonRegistry";
import { RichText } from "./RichText";
import "./tutorials.css";

/** Mount once in the Hub: shows the open lesson, if any. */
export function LessonHost() {
  const open = useLessonStore((s) => s.open);
  const lesson = open ? getLesson(open.id) : undefined;
  if (!open || !lesson) return null;
  return <LessonModal key={open.seq} lesson={lesson} mode={open.mode} />;
}

function LessonModal({ lesson, mode }: { lesson: Lesson; mode: LessonMode }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const shape = useMemo<LessonShape>(
    () => ({ try: lesson.steps, how: lesson.howItWorks }),
    [lesson],
  );
  const [state, dispatch] = useReducer(
    (s: LessonState, a: LessonAction) => lessonReducer(shape, s, a),
    mode,
    (m: LessonMode) => initialLessonState(shape, m),
  );
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const m = useMotionPresets();
  const firstRender = useRef(true);

  const list = state.mode === "how" ? (lesson.howItWorks ?? []) : lesson.steps;
  const step = list[state.index];
  const done = stepDone(shape, state);
  const succeeded = lesson.steps.some(
    (s) => s.gated && state.done.includes(`try:${s.id}`),
  );
  const succeededRef = useRef(false);
  succeededRef.current = succeeded;

  const close = () => {
    if (succeededRef.current && lesson.checklist) {
      void markChecklistItem(lesson.checklist.id);
    }
    closeLesson();
  };

  useEffect(() => {
    if (state.finished) {
      close();
      lesson.finish?.after?.();
    }
    // `close` reads refs only.
  }, [state.finished, lesson]);

  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    headingRef.current?.focus();
  }, [state.mode, state.index]);

  if (!step) return null;

  const last = isLastStep(shape, state);
  const finishing = state.mode === "try" && last && !!lesson.finish;
  const featureOn = finishing && lesson.finish!.isOn();
  const primaryLabel =
    state.mode === "how" && last
      ? t("lessons.common.tryIt")
      : finishing
        ? t(featureOn ? lesson.finish!.onLabelKey : lesson.finish!.labelKey)
        : t(step.continueKey ?? "lessons.common.continue");

  const primary = async () => {
    if (!finishing || featureOn) {
      dispatch({ type: "next" });
      return;
    }
    setBusy(true);
    setFailed(false);
    try {
      if (await lesson.finish!.turnOn()) dispatch({ type: "next" });
      else setFailed(true);
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  };

  const { step: n, total } = progress(shape, state);
  const values = step.values?.() ?? {};
  const titleKey =
    done && step.doneTitleKey ? step.doneTitleKey : step.titleKey;
  const bodyKey = done && step.doneBodyKey ? step.doneBodyKey : step.bodyKey;
  const tone = lesson.tone ?? "warm";

  return (
    <Sheet
      open
      onOpenChange={(o) => !o && !busy && close()}
      label={t("lessons.common.sheetLabel", {
        lesson: t(lesson.titleKey),
        mode: t(
          state.mode === "how"
            ? "lessons.common.howItWorks"
            : "lessons.common.tryItOut",
        ),
      })}
      closeLabel={t("lessons.common.close")}
      closeOnBackdrop={false}
      onBack={
        canGoBack(shape, state) ? () => dispatch({ type: "back" }) : undefined
      }
      backLabel={t("lessons.common.back")}
      progress={{
        current: n,
        total,
        label: t("lessons.common.progress", { step: n, total }),
      }}
      tone={tone}
      className="wb-lesson-sheet"
      footer={
        <div className="flex flex-col items-center gap-3">
          {failed && (
            <p role="alert" className="type-small text-(--wb-danger)">
              {t("lessons.common.turnOnFailed")}
            </p>
          )}
          <div className="flex flex-wrap items-center justify-center gap-2">
            {canSkip(shape, state) && (
              <Button
                variant="secondary"
                onClick={() => dispatch({ type: "skip" })}
                data-testid="wb-lesson-skip"
              >
                {t("lessons.common.skip")}
              </Button>
            )}
            <Button
              variant="primary"
              disabled={!canContinue(shape, state) || busy}
              onClick={() => void primary()}
              data-testid="wb-lesson-primary"
              className="min-w-[112px]"
            >
              {primaryLabel}
            </Button>
          </div>
        </div>
      }
    >
      <motion.div
        key={`${state.mode}:${state.index}`}
        {...m.page}
        className="mx-auto flex w-full max-w-[780px] flex-col items-center gap-6"
        data-testid={`wb-lesson-${lesson.id}`}
        data-mode={state.mode}
        data-step={step.id}
        data-tone={tone}
      >
        <div aria-live="polite" className="flex max-w-[680px] flex-col gap-3">
          <h2
            ref={headingRef}
            tabIndex={-1}
            className="type-sheet text-center text-balance text-(--wb-ink) outline-none"
            data-testid="wb-lesson-heading"
          >
            <RichText
              i18nKey={titleKey}
              values={values}
              accent={tone === "plain"}
              keySize="lg"
            />
          </h2>
          {bodyKey && (
            <p
              className="type-body text-center text-balance text-(--wb-ink-2)"
              data-testid="wb-lesson-body"
            >
              <RichText i18nKey={bodyKey} values={values} />
            </p>
          )}
        </div>
        <div className="w-full">
          {step.render({
            lesson,
            mode: state.mode,
            done,
            complete: () => dispatch({ type: "complete", stepId: step.id }),
            next: () => dispatch({ type: "next" }),
            close,
          })}
        </div>
      </motion.div>
    </Sheet>
  );
}
