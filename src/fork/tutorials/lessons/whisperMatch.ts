// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The whisper practice's success check (S-32, B-42): the take is compared
 * with the sentence the user was asked to whisper, word by word. Continue
 * enables when at least 70 % of the sentence's words came through
 * (`WHISPER_PASS`); below that the step shows tips (get closer, raise the
 * input volume) and the next take tries again.
 *
 * Words are compared without case, accents or punctuation ("I'm" = "im"),
 * each heard word counts once (a repeated word must be heard twice), and
 * extra words cost nothing (a take that adds "um, so" still passes). Scripts
 * written without spaces (Chinese, Japanese) are compared per character.
 *
 * `scripts/wb-s32/wordmatch.py` mirrors these rules for the Mac driver;
 * change both together.
 */

/** Share of the sentence's words that must be heard. */
export const WHISPER_PASS = 0.7;

const UNSPACED = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]/u;

/** The words of a text, normalised for comparison. */
export function matchWords(text: string): string[] {
  const plain = text
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/['’ʼ]/g, "");
  const runs = plain.match(/[\p{L}\p{N}]+/gu) ?? [];
  return runs.flatMap((run) => (UNSPACED.test(run) ? [...run] : [run]));
}

/** Share (0..1) of `target`'s words found in `heard`. */
export function wordMatch(target: string, heard: string): number {
  const want = matchWords(target);
  if (want.length === 0) return 0;
  const have = new Map<string, number>();
  for (const word of matchWords(heard)) {
    have.set(word, (have.get(word) ?? 0) + 1);
  }
  let found = 0;
  for (const word of want) {
    const left = have.get(word) ?? 0;
    if (left > 0) {
      found += 1;
      have.set(word, left - 1);
    }
  }
  return found / want.length;
}

/** The take is close enough to the sentence to continue. */
export function whisperPasses(target: string, heard: string): boolean {
  return wordMatch(target, heard) >= WHISPER_PASS;
}
