// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The lesson modal (S-31, B-39, B-40): one dialog for every lesson. Header:
 * the lesson's title and close (Esc closes too); above the step: Back and
 * progress dashes ("Step n of m" for screen readers). The step renders
 * itself (`Lesson.steps[i].render`); the footer has Skip (a gated step,
 * never a dead end) and the primary button: Continue, "Try it yourself" on
 * the last How it works scene, and the feature's "Turn on X" (or Done when
 * it is on) on the last Try it step.
 *
 * Focus moves to the step's heading on every step change, and the heading
 * is a polite live region, so the success text ("Text rewritten…") is
 * heard. When the lesson closes after a practice step succeeded, its
 * checklist item ticks (`lessonRegistry.tsx`).
 */
import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { ArrowLeft } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { Dialog } from "../ui/Dialog";
import { FORK_NAMESPACE } from "../i18n";
import { markChecklistItem } from "../hub/SetupChecklist";
import { Button, FOCUS_RING } from "../ui";
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
  const title = t(
    done && step.doneTitleKey ? step.doneTitleKey : step.titleKey,
    values,
  );
  const bodyKey = done && step.doneBodyKey ? step.doneBodyKey : step.bodyKey;

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && !busy && close()}
      title={
        <span className="font-(family-name:--wb-font-display) text-[20px] font-medium">
          {t(lesson.titleKey)}
        </span>
      }
      closeLabel={t("lessons.common.close")}
      closeOnBackdrop={false}
      className="max-w-[760px]!"
      contentFades={false}
      footer={
        <div className="flex flex-wrap items-center justify-end gap-2">
          {failed && (
            <p
              role="alert"
              className="me-auto text-sm text-red-700 dark:text-red-300"
            >
              {t("lessons.common.turnOnFailed")}
            </p>
          )}
          {canSkip(shape, state) && (
            <Button
              variant="ghost"
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
          >
            {primaryLabel}
          </Button>
        </div>
      }
    >
      <div
        className="flex flex-col gap-4"
        data-testid={`wb-lesson-${lesson.id}`}
        data-mode={state.mode}
        data-step={step.id}
      >
        <div className="flex items-center gap-3">
          {canGoBack(shape, state) ? (
            <button
              type="button"
              onClick={() => dispatch({ type: "back" })}
              aria-label={t("lessons.common.back")}
              title={t("lessons.common.back")}
              data-testid="wb-lesson-back"
              className={`grid size-7 shrink-0 cursor-pointer place-items-center rounded-full text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
            >
              <ArrowLeft
                size={16}
                aria-hidden="true"
                className="rtl:-scale-x-100"
              />
            </button>
          ) : (
            <span className="size-7 shrink-0" aria-hidden="true" />
          )}
          <span className="text-[12px] font-medium tracking-wide text-(--wb-ink-2) uppercase">
            {t(
              state.mode === "how"
                ? "lessons.common.howItWorks"
                : "lessons.common.tryItOut",
            )}
          </span>
          <ol
            className="wb-lesson-dashes ms-auto flex gap-1.5"
            aria-label={t("lessons.common.progress", { step: n, total })}
            data-testid="wb-lesson-progress"
          >
            {list.map((s, i) => (
              <li
                key={s.id}
                aria-current={i === state.index ? "step" : undefined}
                data-on={i <= state.index || undefined}
              >
                <span className="sr-only">
                  {t("lessons.common.progress", { step: i + 1, total })}
                </span>
              </li>
            ))}
          </ol>
        </div>
        <div aria-live="polite">
          <h3
            ref={headingRef}
            tabIndex={-1}
            className="font-(family-name:--wb-font-display) text-[22px] leading-tight font-medium outline-none"
            data-testid="wb-lesson-heading"
          >
            {title}
          </h3>
          {bodyKey && (
            <p
              className="mt-1.5 text-[14px] leading-5 text-(--wb-ink-2)"
              data-testid="wb-lesson-body"
            >
              {t(bodyKey, values)}
            </p>
          )}
        </div>
        {step.render({
          lesson,
          mode: state.mode,
          done,
          complete: () => dispatch({ type: "complete", stepId: step.id }),
          next: () => dispatch({ type: "next" }),
          close,
        })}
      </div>
    </Dialog>
  );
}
