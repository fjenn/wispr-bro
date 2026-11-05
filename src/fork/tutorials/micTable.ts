// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The microphone quiz's recommendation rules (S-32, B-43, P-1): our own
 * table, no product names. Four kinds of microphone, rated 1-3 on what the
 * results table shows (mic quality, whispering around others, whispering at
 * home) plus a price band and whether it travels with you, and one score
 * per kind from the four answers. The highest score is the "Best match".
 *
 * Why the ratings (also in `docs/fork/user/microphones.md`):
 * - built-in: always there on a laptop, but far from the mouth, so a whisper
 *   has to be louder and the room is picked up too;
 * - wired headset: the mic sits next to the mouth and the cable has no
 *   radio link, so the quietest whisper comes through, anywhere;
 * - wireless earbuds: close to the ear rather than the mouth, and the radio
 *   link lowers the mic's quality while it is in use (it can also drop);
 * - desk or USB mic: the best sound at one desk when you sit close, but it
 *   stays on the desk and a room around you comes through with it.
 *
 * Score (pure, unit-tested over every combination in `micTable.test.ts`):
 *   quality
 *   + around others × (shared 2, mixed 1, private 0; unanswered 1)
 *   + at home × (private 2, mixed 1, shared 0; unanswered 1)
 *   + moving between places: +2 for a mic that travels, −4 for one that
 *     does not; one desk: +1 for the desk mic
 *   + 2 for the kind the user finds comfortable
 *   + 2 for a mic they have, else − the price (included 0, low ½, medium 1)
 * A built-in mic they do not have (a Mac without one) is never the best
 * match. Ties go to the earlier kind in `MIC_KINDS`.
 */

export const MIC_KINDS = [
  "wired_headset",
  "desk_usb",
  "wireless_earbuds",
  "built_in",
] as const;
export type MicKind = (typeof MIC_KINDS)[number];

export const WORKPLACES = ["private", "mixed", "shared"] as const;
export type Workplace = (typeof WORKPLACES)[number];

export const MOVES = ["one_desk", "different_places"] as const;
export type Moves = (typeof MOVES)[number];

export type Rating = 1 | 2 | 3;
export type Price = "included" | "low" | "medium";

export interface MicTraits {
  quality: Rating;
  aroundOthers: Rating;
  atHome: Rating;
  price: Price;
  /** Goes with you between places. */
  portable: boolean;
}

export const MIC_TRAITS: Readonly<Record<MicKind, MicTraits>> = {
  built_in: {
    quality: 1,
    aroundOthers: 1,
    atHome: 2,
    price: "included",
    portable: true,
  },
  wired_headset: {
    quality: 3,
    aroundOthers: 3,
    atHome: 3,
    price: "low",
    portable: true,
  },
  wireless_earbuds: {
    quality: 2,
    aroundOthers: 2,
    atHome: 2,
    price: "medium",
    portable: true,
  },
  desk_usb: {
    quality: 3,
    aroundOthers: 2,
    atHome: 3,
    price: "medium",
    portable: false,
  },
};

export interface QuizAnswers {
  /** "Pick a comfortable microphone" (one). */
  comfortable: MicKind | null;
  /** "Where do you usually work?" */
  workplace: Workplace | null;
  /** "Do you move between places?" */
  moves: Moves | null;
  /** "Which microphones do you have?" (any). */
  owned: readonly MicKind[];
}

/** Before any answer: most Macs people dictate on are laptops. */
export const DEFAULT_ANSWERS: QuizAnswers = {
  comfortable: null,
  workplace: null,
  moves: null,
  owned: ["built_in"],
};

const PRICE_COST: Record<Price, number> = { included: 0, low: 0.5, medium: 1 };

function weights(workplace: Workplace | null): {
  others: number;
  home: number;
} {
  switch (workplace) {
    case "private":
      return { others: 0, home: 2 };
    case "mixed":
      return { others: 1, home: 1 };
    case "shared":
      return { others: 2, home: 0 };
    default:
      return { others: 1, home: 1 };
  }
}

export function micScore(kind: MicKind, answers: QuizAnswers): number {
  const traits = MIC_TRAITS[kind];
  const w = weights(answers.workplace);
  let score =
    traits.quality + traits.aroundOthers * w.others + traits.atHome * w.home;
  if (answers.moves === "different_places") {
    score += traits.portable ? 2 : -4;
  } else if (answers.moves === "one_desk" && !traits.portable) {
    score += 1;
  }
  if (answers.comfortable === kind) score += 2;
  score += answers.owned.includes(kind) ? 2 : -PRICE_COST[traits.price];
  return score;
}

/** A built-in mic counts only when the Mac has one. */
export function canRecommend(kind: MicKind, answers: QuizAnswers): boolean {
  return kind !== "built_in" || answers.owned.includes("built_in");
}

export interface MicRow extends MicTraits {
  kind: MicKind;
  owned: boolean;
  score: number;
  best: boolean;
}

/** The results table: every kind in `MIC_KINDS` order, one best match. */
export function recommend(answers: QuizAnswers): {
  rows: MicRow[];
  best: MicKind;
} {
  let best: MicKind = MIC_KINDS[0];
  let bestScore = -Infinity;
  for (const kind of MIC_KINDS) {
    if (!canRecommend(kind, answers)) continue;
    const score = micScore(kind, answers);
    if (score > bestScore) {
      best = kind;
      bestScore = score;
    }
  }
  const rows = MIC_KINDS.map((kind) => ({
    kind,
    ...MIC_TRAITS[kind],
    owned: answers.owned.includes(kind),
    score: micScore(kind, answers),
    best: kind === best,
  }));
  return { rows, best };
}
