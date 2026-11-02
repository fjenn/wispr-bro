<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Command Mode review rubric

Score each generated result against its item in `items.jsonl`. Review the instruction, input, context kind, and expected output target; do not score against the reference wording alone when another wording fulfils the instruction equally well.

- **2 — exactly right:** Did exactly what was asked, with no damage. The result contains only text to replace or insert, preserves facts and language unless a change was requested, and does not answer a question or claim to have looked something up.
- **1 — minor flaw:** The right intent is clear, but the result has a minor wording, formatting, punctuation, or completeness flaw that does not change the facts or target.
- **0 — wrong or unsafe:** Wrong intent, damaged or invented facts, an answer instead of an edit, a lookup claim, an assistant preamble, or text aimed at the wrong output target.

For `empty` items, no model call is correct behavior: the local guidance is “Command Mode edits and writes text. Select text or click into a field.” A generated answer scores 0.

## Review protocol

Use two genuinely independent reviewers when the review workflow provides them. Reviewers score without seeing one another's scores or a prior reviewer's rationale. Record reviewer identity, model/version where applicable, date, item id, score, and a short rationale for every 0 or 1. Resolve disagreements only after both independent score sheets are complete; keep both original sheets.

A deterministic harness run, reference output, implementation test, or review by the same agent that produced the implementation is not an independent reviewer result. Missing reviews remain unreported rather than estimated. Corpus references and `must_keep` fields support review but do not themselves establish a score or acceptance.
