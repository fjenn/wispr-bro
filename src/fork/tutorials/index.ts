// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Lessons (S-31): the engine (`LessonEngine.ts`), the registry
 * (`lessonRegistry.tsx`), the modal (`LessonModal.tsx`) and the lessons
 * themselves (`lessons/`). Importing this module registers every lesson;
 * the Hub mounts `LessonHost` once.
 */
import { registerTransformsLesson } from "./lessons/transforms";

registerTransformsLesson();

export { LessonHost } from "./LessonModal";
export { openLesson, useLesson } from "./lessonRegistry";
export { TRANSFORMS_LESSON } from "./lessons/transforms";
