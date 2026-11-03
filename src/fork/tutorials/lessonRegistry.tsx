// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The lesson registry (S-31, B-40, B-45). A feature describes its lesson as
 * data (`Lesson`: steps that render themselves, an optional How it works,
 * the feature's "Turn on X" and its re-entry points) and registers it from
 * its own module, as `lessons/transforms.tsx` does. Registering also wires
 * the re-entry points, so a lesson needs nothing else:
 *
 * - Help › Essentials gets an entry (`help`; S-26b's `setHelpLessons`);
 * - the setup checklist gets an item (`checklist`; S-30's registry) that
 *   opens the lesson and ticks once its practice step succeeded (when the
 *   lesson closes, so S-30's chaining never pulls the user out mid-lesson)
 *   or the item's own `isDone` says so;
 * - pages ask `useLesson(id)` for their banner buttons and "Start
 *   tutorial" (S-22's Transforms banner: Try it / How it works).
 *
 * One lesson is open at a time (`useLessonStore`); `LessonModal.tsx` shows
 * it.
 */
import { useSyncExternalStore, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import i18next from "i18next";
import { create } from "zustand";
import { setHelpLessons } from "../hub/HelpPopover";
import {
  registerChecklistItem,
  type ChecklistItem,
} from "../hub/checklistRegistry";
import { FORK_NAMESPACE } from "../i18n";
import type { LessonMode, LessonStepShape } from "./LessonEngine";

/** What a step's `render` can use. */
export interface LessonStepContext {
  lesson: Lesson;
  mode: LessonMode;
  /** The step reported success already. */
  done: boolean;
  /** Reports success (enables Continue on a gated step). */
  complete: () => void;
  next: () => void;
  close: () => void;
}

export interface LessonStep extends LessonStepShape {
  /** The step's heading (`fork` key). A step may change it, see `heading`. */
  titleKey: string;
  /** One line under the heading. */
  bodyKey?: string;
  /** Heading and line once the step reported success. */
  doneTitleKey?: string;
  doneBodyKey?: string;
  /** Values for the heading/body strings (shortcut labels…). */
  values?: () => Record<string, string>;
  /** Primary button label instead of Continue (`fork` key). */
  continueKey?: string;
  render: (ctx: LessonStepContext) => ReactNode;
}

/** The feature's own switch at the end of Try it ("Turn on X"). */
export interface LessonFinish {
  labelKey: string;
  /** Label when the feature is already on (the button then just closes). */
  onLabelKey: string;
  isOn: () => boolean;
  /** Turns the feature on; false keeps the lesson open (it failed). */
  turnOn: () => Promise<boolean>;
  /** After the lesson finished (e.g. open the feature's page). */
  after?: () => void;
}

export interface Lesson {
  /** Stable id (`a-z0-9_-`), also the test id suffix. */
  id: string;
  icon: LucideIcon;
  /** Dialog title (`fork` key). */
  titleKey: string;
  steps: readonly LessonStep[];
  howItWorks?: readonly LessonStep[];
  finish?: LessonFinish;
  /** Help › Essentials entry (`fork` key for its label). */
  help?: { labelKey: string; order: number };
  /** Setup checklist item; `run` and ticking come from the registry. */
  checklist?: Omit<ChecklistItem, "run">;
}

let lessons: readonly Lesson[] = [];
const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function syncHelp(): void {
  const entries = lessons
    .filter((lesson) => lesson.help)
    .sort((a, b) => a.help!.order - b.help!.order);
  setHelpLessons(
    entries.length === 0
      ? null
      : () =>
          entries.map((lesson) => ({
            id: `lesson-${lesson.id}`,
            label: i18next.t(lesson.help!.labelKey, { ns: FORK_NAMESPACE }),
            icon: lesson.icon,
            onSelect: () => openLesson(lesson.id, "try"),
          })),
  );
}

/** Adds (or replaces, by id) a lesson. Returns a function that removes it. */
export function registerLesson(lesson: Lesson): () => void {
  lessons = [...lessons.filter((l) => l.id !== lesson.id), lesson];
  const offChecklist = lesson.checklist
    ? registerChecklistItem({
        ...lesson.checklist,
        run: () => openLesson(lesson.id, "try"),
      })
    : undefined;
  syncHelp();
  emit();
  return () => {
    const before = lessons.length;
    lessons = lessons.filter((l) => l !== lesson);
    offChecklist?.();
    if (lessons.length !== before) {
      syncHelp();
      emit();
    }
  };
}

export function getLesson(id: string): Lesson | undefined {
  return lessons.find((lesson) => lesson.id === id);
}

export function registeredLessons(): readonly Lesson[] {
  return lessons;
}

/** The lesson, once registered (re-renders when it is). */
export function useLesson(id: string): Lesson | undefined {
  return useSyncExternalStore(
    subscribe,
    () => getLesson(id),
    () => getLesson(id),
  );
}

interface OpenLesson {
  id: string;
  mode: LessonMode;
  /** Increases on every open, so reopening starts over. */
  seq: number;
}

export const useLessonStore = create<{ open: OpenLesson | null }>()(() => ({
  open: null,
}));

let seq = 0;

/** Opens a lesson on Try it or How it works (unknown ids do nothing). */
export function openLesson(id: string, mode: LessonMode = "try"): void {
  if (!getLesson(id)) {
    console.warn(`tutorials: no lesson "${id}"`);
    return;
  }
  seq += 1;
  useLessonStore.setState({ open: { id, mode, seq } });
}

export function closeLesson(): void {
  useLessonStore.setState({ open: null });
}
