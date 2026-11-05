// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Typing-test paragraphs (S-29b), one per language, written for Wispr Bro:
 * the same everyday scene in each language, plain words, no names, about
 * 80-90 words (more than most people type in 30 seconds). Each also has
 * the practice email's subject and suggested sentence (rc9: the practice
 * follows the user's languages, not the app's). Latin-script
 * languages only: a test typed through an input method (Chinese, Japanese)
 * would measure the input method, not the typist.
 */
import de from "./de.json";
import en from "./en.json";
import es from "./es.json";
import fr from "./fr.json";
import it from "./it.json";
import nl from "./nl.json";
import pt from "./pt.json";

export interface PracticeSample {
  subject: string;
  /** What the composer suggests saying. */
  suggestion: string;
}

export interface TypingSample {
  language: string;
  text: string;
  practice: PracticeSample;
}

export const TYPING_SAMPLES: readonly TypingSample[] = [
  en,
  nl,
  fr,
  de,
  es,
  it,
  pt,
];

const base = (code: string) => code.toLowerCase().split(/[-_]/)[0];

export function sampleFor(language: string): TypingSample {
  return (
    TYPING_SAMPLES.find((s) => s.language === base(language)) ??
    TYPING_SAMPLES[0]
  );
}

/**
 * The test's first language: the first spoken language with a sample, else
 * the app's language, else English.
 */
export function defaultSampleLanguage(
  spoken: readonly string[],
  uiLanguage: string,
): string {
  for (const code of [...spoken, uiLanguage]) {
    const found = TYPING_SAMPLES.find((s) => s.language === base(code));
    if (found) return found.language;
  }
  return "en";
}

/**
 * The practice email's language (rc9): the first of the user's languages
 * the first speech model hears (so the take runs on it at once) that has a
 * practice text, else the first of the user's languages that has one, else
 * the app's language, else English.
 */
export function practiceSampleFor(
  starterLanguages: readonly string[],
  spoken: readonly string[],
  uiLanguage: string,
): TypingSample {
  for (const code of [...starterLanguages, ...spoken, uiLanguage]) {
    const found = TYPING_SAMPLES.find((s) => s.language === base(code));
    if (found) return found;
  }
  return TYPING_SAMPLES[0];
}
