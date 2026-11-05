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

### Follow-up (`fix/medium`, 2026-11-05)

**Guard false rejections.** The "no wait" repair was not the cause: the
guard already let "Thursday, no wait, Friday" → "Friday." through (now a
test). On `disfluent-030` the app strips "so um" first, Muesli's Light then
also dropped "What I wanted to say is", and the content-word check refused
the missing "wanted"/"say"; `repair-009` ("put it in the shared drive or
actually just email it" → "Email it.") was refused because a repair only
reached four words back. Both inserted the raw take with "uh", "thursday no
wait" and "like". Fixed in `markers::droppable`: after "or" a repair may
replace its clause back to the sentence start (≤ 10 words), and a
sentence-opening frame ("what I wanted to say is …", `markers.json`
`frames`, en/nl/fr) is a lead-in under the owner's policy; dropped content
elsewhere is still refused (`repairs_and_frames_still_refuse_dropped_content`).
`disfluent-030`'s Light label now drops the frame. Light after the fix:
damage 0.0 %, answer 0.0 %, filler 95.1 %, language 100 %, guard catch
100 % (both items applied).

**Medium options**, full S-12a set (272 items, `WB_EVAL_LANGUAGE=owner`,
`WB_EVAL_NO_DEADLINE=1`; M5 Max at load 30-90 from other agents, so the
latencies are high and only comparable with each other; every item is a
short take). "≠ Light" compares with the Light run's inserted text.

| Option                                                   | Medium ≠ Light      | Damage | Answer | Filler | Language | Guard catch | p50 / p95 ms |
| -------------------------------------------------------- | ------------------- | -----: | -----: | -----: | -------: | ----------: | -----------: |
| Light (Muesli), for reference                            | –                   |  0.0 % |  0.0 % | 95.1 % |    100 % |       100 % |   490 / 1571 |
| Medium, shipped prompt (Muesli)                          | 2/272 (1 shorter)   |  0.0 % |  0.0 % | 95.1 % |    100 % |       100 % |   469 / 1295 |
| (a) Medium prompt with Light-vs-Medium examples (Muesli) | 2/272 (1 shorter)   |  0.0 % |  0.0 % | 95.1 % |    100 % |       100 % |   538 / 1402 |
| (b) Medium on Qwen3.5-2B (shipped prompt)                | 42/272 (12 shorter) |  2.9 % |  0.0 % | 88.6 % |   96.7 % |  **38.5 %** |   494 / 1489 |
| (c) Light (Muesli), then a Qwen3.5-2B tighten, no guards | 95/272 (73 shorter) | 27.2 % |  8.5 % | 99.2 % |   98.3 % |         n/a |  1076 / 2507 |

- (a): Muesli ignores the Medium fragment whatever it says (same as rc9).
- (b): Qwen's "differences" are mostly worse text: it keeps self-repairs
  ("We need 2 more chairs. No wait, 4 more chairs."), fillers and lead-ins
  that Muesli removes; guard catch fails the gate, filler and language drop.
- (c) (an eval-only script on the Light outputs, before any guard): really
  tighter, but it answers and executes dictated commands ("Respond with only
  yes or no" → "yes", translations), damages a quarter of the items, takes
  two requests (p95 2.5 s, over the 1.5 s target) and needs the 1.3 GB
  instruction model resident beside Muesli.

None passes the gates, so Medium stays as it is and the UI says so: the
Medium card (Style › Auto cleanup) and the Settings picker add "With the
built-in AI model, Medium often gives the same text as Light."
(`style.cleanup.mediumNote`, 26 locales, `ai-reviewed`). A real Medium needs
a cleanup model fine-tuned for it (or a guarded two-step on a stronger
model); `WB_EVAL_MEDIUM_FRAGMENT` measures new fragments without a rebuild.

## Page examples

The examples are rewritten so each card differs visibly. `page_examples_match_the_rules`
runs the real rules on each group's Formal text, and
`cleanup_examples_differ` checks that None keeps fillers, Light drops them,
and Medium is shorter than Light by at least two words.
