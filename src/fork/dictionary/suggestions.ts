// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Words worth adding to the dictionary (flowmatch: the banner's suggestion
 * chips): names and terms from recent dictations that are not in it yet.
 * A candidate is a word capitalised in the middle of a sentence ("met
 * Siobhán today") or one with a capital or a digit inside ("iOS", "GPT4"),
 * at least three letters, ranked by how often it was said. Computed on the
 * device from History rows the page already may read; nothing is stored.
 */

const WORD = /[\p{L}\p{N}][\p{L}\p{M}\p{N}'’-]*/gu;
const SENTENCE = /[^.!?\n]+/g;

/** Words that are capitalised for grammar, not because they are names. */
const COMMON = new Set([
  "i",
  "i'm",
  "i've",
  "i'll",
  "i'd",
  "monday",
  "tuesday",
  "wednesday",
  "thursday",
  "friday",
  "saturday",
  "sunday",
  "january",
  "february",
  "march",
  "april",
  "may",
  "june",
  "july",
  "august",
  "september",
  "october",
  "november",
  "december",
  "ok",
  "okay",
]);

export function suggestWords(
  texts: string[],
  known: Iterable<string>,
  max = 4,
): string[] {
  const skip = new Set([...known].map((w) => w.toLocaleLowerCase()));
  const counts = new Map<string, number>();
  for (const text of texts) {
    for (const sentence of text.match(SENTENCE) ?? []) {
      const words = sentence.match(WORD) ?? [];
      words.forEach((word, index) => {
        const letters = [...word];
        if (letters.length < 3) return;
        const inner = letters.slice(1).join("");
        const midCap = index > 0 && /^\p{Lu}/u.test(word);
        const mixed = /\p{Lu}|\p{N}/u.test(inner) && /\p{L}/u.test(word);
        if (!midCap && !mixed) return;
        const key = word.toLocaleLowerCase();
        if (COMMON.has(key) || skip.has(key)) return;
        if (/^\p{Lu}+$/u.test(word) && letters.length > 6) return;
        counts.set(word, (counts.get(word) ?? 0) + 1);
      });
    }
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, max)
    .map(([word]) => word);
}
