<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# rc9: does Style and Auto cleanup do what the page says?

Owner report (rc9): the three style cards of each group look the same; Light
and Medium look the same; `style: Formal for other (no change)` in the log.
Run 2026-11-05 on branch `fix/styleex`, M-series Mac, no GUI, no microphone.

## Setup

- Model: bundled cleanup model Muesli 2B v12 Q4_K_M (APFS clone of the bench
  copy in the scratchpad; nothing committed), llama.cpp b11624 `llama-server`.
- Path: `fork::llm::eval_run` (the shipped `post_process::clean`: gate,
  prompt builder, guards, deadlines), then stage 10 (`style_rules::apply`)
  through `eval_restyle`.
- Inputs: 12 realistic English takes (fillers, false starts, "no wait"
  corrections, greetings, sign-offs, chat slang), plus the full S-12a set
  (`eval/cleanup/items.jsonl`, 271 items, `WB_EVAL_LANGUAGE=owner`).

## Styles (deterministic, stage 10)

| Setting                     | Changed the text (12 takes, after Light / Medium / None) | As described?                                                                                                                                  |
| --------------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Formal (all groups)         | 0 / 36                                                   | Yes, by design: Formal is the cleaned text as it is ("Caps + punctuation"). The log now says `Formal keeps the cleaned text`, not "no change". |
| Casual                      | 11/12 Light, 11/12 Medium, 6/12 None                     | Yes: commas and the final period go. Unchanged only where the take has neither (chat slang with no punctuation, raw lower-case None takes).    |
| Very casual (Personal)      | same counts                                              | Yes: Casual plus lower-case sentence starts ("hey Sam are you around this weekend? we could…").                                                |
| Upbeat (Work, Email, Other) | same counts                                              | Yes: the final period becomes "!"; a question keeps "?".                                                                                       |

Group → preset selection and the English-only / AI-chat skips are covered by
`post_process` unit tests (`decide_style`); the page already says "Styles
apply to dictations in English. Cleanup works in every language."

## Cleanup levels (model)

| Level                                                           | Result                                                                                                                                  | As described?                              |
| --------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| None                                                            | Raw take, fillers and false starts kept                                                                                                 | Yes                                        |
| Light                                                           | Fillers, stutters and "no wait" corrections removed, punctuation fixed (11/12; 1 guard fallback)                                        | Yes                                        |
| Medium (before)                                                 | Same text as Light on 10/12 takes and on 255/271 S-12a items                                                                            | **No**: Muesli ignored the Medium fragment |
| Medium (rc9 prompt, shipped)                                    | Same as Light on 252/271; gates: damage 0.0 %, answer 0.0 %, filler 95.0 % (was 90.0 %), language 100 % (was 98.3 %), guard catch 100 % | Still mostly Light; no regressions         |
| Medium with a Medium line in the user turn (tried, not shipped) | Same as Light on only 190/271, but damage 1.5 %, guard catch 90 %, more answered instructions                                           | Fails the S-12a gates                      |

Shipped for Medium: a stronger fragment with two examples, and a Medium
reply the guards reject now falls back to Light's cleanup instead of the raw
take (`rejected_medium_falls_back_to_light`). An eval case
(`disfluent-030`, the page's sample) holds the Light/Medium gap: on it
Muesli's Medium gives "The new onboarding goes live Friday and the early
feedback is really good", while Light was guard-rejected (content words
"wait", "like") and fell back to the raw take.

**Open:** making Medium really different on Muesli needs a prompt or model
change that passes the S-12a gates. That is a separate eval-gated task.

## Page examples

The examples are rewritten so each card differs visibly. `page_examples_match_the_rules`
runs the real rules on each group's Formal text, and
`cleanup_examples_differ` checks that None keeps fillers, Light drops them,
and Medium is shorter than Light by at least two words.
