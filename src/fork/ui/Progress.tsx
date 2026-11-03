// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Progress indicators (Design Phase 0):
 *
 * - `ProgressDashes`: short dashes at the top of a sheet (lessons, import
 *   wizard): black = done or current, grey = to come.
 * - `ProgressLine`: a thin bar (2 pt purple under the onboarding header,
 *   4 pt ink in the setup checklist).
 * - `Stepper`: the onboarding's uppercase stage labels with chevrons;
 *   current and past stages in ink, future in ink-3.
 */
import { Fragment } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { ChevronRight } from "./icons";
import { useMotionPresets, SPRING } from "./motion";

export function ProgressDashes({
  current,
  total,
  label,
  className = "",
}: {
  /** 1-based index of the current step. */
  current: number;
  total: number;
  /** Accessible text; defaults to "Step n of m". */
  label?: string;
  className?: string;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const text = label ?? t("lessons.common.progress", { step: current, total });
  return (
    <div
      role="img"
      aria-label={text}
      data-testid="wb-progress-dashes"
      className={`flex items-center gap-1.5 ${className}`}
    >
      {Array.from({ length: total }, (_, i) => (
        <span
          key={i}
          className={`h-[3px] w-8 rounded-full transition-colors duration-(--wb-dur-base) ${i < current ? "bg-(--wb-ink)" : "bg-(--wb-line-2)"}`}
        />
      ))}
    </div>
  );
}

export function ProgressLine({
  value,
  label,
  tone = "accent",
  thickness = 2,
  className = "",
}: {
  /** 0..1 */
  value: number;
  /** Accessible name of the progress bar. */
  label: string;
  /** `accent` (purple, flows) or `ink` (checklist). */
  tone?: "accent" | "ink";
  thickness?: 2 | 4;
  className?: string;
}) {
  const m = useMotionPresets();
  const clamped = Math.min(Math.max(value, 0), 1);
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(clamped * 100)}
      className={`w-full overflow-hidden rounded-full bg-(--wb-line) ${className}`}
      style={{ height: thickness }}
    >
      <motion.div
        className={`h-full origin-left rounded-full rtl:origin-right ${tone === "accent" ? "bg-(--wb-accent-2)" : "bg-(--wb-ink)"}`}
        initial={false}
        animate={{ scaleX: clamped }}
        transition={m.reduced ? { duration: 0 } : SPRING.gentle}
      />
    </div>
  );
}

export interface StepperStage {
  id: string;
  label: string;
}

export function Stepper({
  stages,
  current,
  label,
  className = "",
}: {
  stages: StepperStage[];
  /** Id of the current stage. */
  current: string;
  /** Accessible name of the list. */
  label: string;
  className?: string;
}) {
  const index = stages.findIndex((s) => s.id === current);
  return (
    <ol
      aria-label={label}
      data-testid="wb-stepper"
      className={`flex items-center gap-2 ${className}`}
    >
      {stages.map((stage, i) => (
        <Fragment key={stage.id}>
          {i > 0 && (
            <li aria-hidden="true" className="text-(--wb-ink-3)">
              <ChevronRight size={14} className="rtl:-scale-x-100" />
            </li>
          )}
          <li
            aria-current={i === index ? "step" : undefined}
            className={`type-micro ${i <= index ? "text-(--wb-ink)" : "text-(--wb-ink-3)"}`}
          >
            {stage.label}
          </li>
        </Fragment>
      ))}
    </ol>
  );
}
