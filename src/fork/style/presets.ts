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
      "hey sam um are you around this weekend we could grab dinner on saturday if you're free",
    after: {
      formal:
        "Hey Sam, are you around this weekend? We could grab dinner on Saturday, if you're free.",
      relaxed:
        "Hey Sam are you around this weekend? We could grab dinner on Saturday if you're free",
      very_relaxed:
        "hey Sam are you around this weekend? we could grab dinner on Saturday if you're free",
    },
  },
  work: {
    before:
      "hi team uh the release notes are ready for review thanks everyone for the quick turnaround",
    after: {
      formal:
        "Hi team, the release notes are ready for review. Thanks, everyone, for the quick turnaround.",
      relaxed:
        "Hi team the release notes are ready for review. Thanks everyone for the quick turnaround",
      upbeat:
        "Hi team, the release notes are ready for review. Thanks, everyone, for the quick turnaround!",
    },
  },
  email: {
    before:
      "hi priya thanks for sending the contract over i'll review it and get back to you by friday",
    after: {
      formal:
        "Hi Priya, thanks for sending the contract over. I'll review it and get back to you by Friday.",
      relaxed:
        "Hi Priya thanks for sending the contract over. I'll review it and get back to you by Friday",
      upbeat:
        "Hi Priya, thanks for sending the contract over. I'll review it and get back to you by Friday!",
    },
  },
  other: {
    before:
      "um the new onboarding flow is live and early feedback looks good next up is the pricing page",
    after: {
      formal:
        "The new onboarding flow is live, and early feedback looks good. Next up is the pricing page.",
      relaxed:
        "The new onboarding flow is live and early feedback looks good. Next up is the pricing page",
      upbeat:
        "The new onboarding flow is live, and early feedback looks good. Next up is the pricing page!",
    },
  },
};

/**
 * Style › Cleanup's sample (flowmatch): one spoken sentence and what each
 * cleanup level makes of it. English data like the examples above; our own
 * text.
 */
export const CLEANUP_EXAMPLES: Record<"none" | "light" | "medium", string> = {
  none: "so um I think we should uh move the call to Thursday, no wait, Friday afternoon if that's okay with everyone",
  light:
    "So I think we should move the call to Friday afternoon, if that's okay with everyone.",
  medium:
    "I think we should move the call to Friday afternoon, if that works for everyone.",
};
