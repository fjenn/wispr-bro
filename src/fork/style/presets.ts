// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The presets of each group and their before/after examples (S-19a, R2
 * D-23, D-24, D-25). Our own sample texts, not Wispr Flow's.
 *
 * The examples are English on purpose (styles apply to English dictations
 * only), so they are data here, not UI strings: the page marks them
 * `lang="en"` and every locale shows the same text. Each "after" is what
 * the app inserts for the "before": Light cleanup, then the preset's rules
 * (`fork/text/style_rules.rs`, whose test `page_examples_match_the_rules`
 * reads this file and checks every "after" against the rules).
 */
import type { StyleGroup, StylePreset } from "../bindings";

export const PRESETS: Record<StyleGroup, readonly StylePreset[]> = {
  personal: ["formal", "relaxed", "very_relaxed"],
  work: ["formal", "relaxed", "upbeat"],
  email: ["formal", "relaxed", "upbeat"],
  other: ["formal", "relaxed", "upbeat"],
};

export interface GroupExample {
  /** What was said. */
  before: string;
  /** What each of the group's presets inserts. */
  after: Partial<Record<StylePreset, string>>;
}

export const EXAMPLES: Record<StyleGroup, GroupExample> = {
  personal: {
    before:
      "hey sam um are you free tonight i'm thinking pizza uh around eight my treat of course",
    after: {
      formal:
        "Hey Sam, are you free tonight? I'm thinking pizza, around eight. My treat, of course.",
      relaxed:
        "Hey Sam are you free tonight? I'm thinking pizza around eight. My treat of course",
      very_relaxed:
        "hey Sam are you free tonight? i'm thinking pizza around eight. my treat of course",
    },
  },
  work: {
    before:
      "hi team uh the release is out thanks everyone for the late nights great work",
    after: {
      formal:
        "Hi team, the release is out. Thanks, everyone, for the late nights. Great work.",
      relaxed:
        "Hi team the release is out. Thanks everyone for the late nights. Great work",
      upbeat:
        "Hi team, the release is out. Thanks, everyone, for the late nights. Great work!",
    },
  },
  email: {
    before:
      "hi priya new line thanks the contract looks good i'll sign it today and um send it back tomorrow best mary",
    after: {
      formal:
        "Hi Priya,\n\nThanks, the contract looks good. I'll sign it today, and send it back tomorrow.\n\nBest,\nMary",
      relaxed:
        "Hi Priya,\n\nThanks the contract looks good. I'll sign it today and send it back tomorrow.\n\nBest,\nMary",
      upbeat:
        "Hi Priya,\n\nThanks, the contract looks good. I'll sign it today, and send it back tomorrow!\n\nBest,\nMary",
    },
  },
  other: {
    before:
      "um so far the new onboarding is live and feedback is good next up the pricing page",
    after: {
      formal:
        "So far, the new onboarding is live, and feedback is good. Next up, the pricing page.",
      relaxed:
        "So far the new onboarding is live and feedback is good. Next up the pricing page",
      upbeat:
        "So far, the new onboarding is live, and feedback is good. Next up, the pricing page!",
    },
  },
};

/**
 * Style › Cleanup's sample (flowmatch): one spoken sentence and what each
 * cleanup level makes of it. English data like the examples above; our own
 * text. None keeps the fillers and the false start, Light removes them,
 * Medium also cuts the wordy opener (`style_rules.rs`
 * `cleanup_examples_differ` checks they stay apart).
 */
export const CLEANUP_EXAMPLES: Record<"none" | "light" | "medium", string> = {
  none: "so um what I wanted to say is uh the new onboarding goes live Thursday, no wait, Friday and like the early feedback is really good",
  light:
    "What I wanted to say is the new onboarding goes live Friday, and the early feedback is really good.",
  medium:
    "The new onboarding goes live Friday, and early feedback is really good.",
};
