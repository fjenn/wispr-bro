// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Typing-test paragraphs (S-29b), one per language, written for Wispr Bro:
 * the same everyday scene in each language, plain words, no names, about
 * 80-90 words (more than most people type in 30 seconds). Latin-script
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

export interface TypingSample {
  language: string;
  text: string;
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
