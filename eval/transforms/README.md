# Transforms eval (OT2-D)

Realistic transforms with pass criteria checked by code (no LLM judge).
31 custom transforms (letter swaps, word and name replacement, upper /
lower / title case, removals, translation into three languages, formal,
friendly, confident, bullet and numbered lists, grammar, fillers,
shortening, summaries, e-mail, digits, American spelling, redaction, emoji)
and 6 built-in runs (Polish, Prompt Engineer).

| File                                    | What                                   |
| --------------------------------------- | -------------------------------------- |
| `items.jsonl`                           | one item per line (schema below)       |
| `src-tauri/src/fork/transforms/eval.rs` | scorer, item self-tests and the runner |

Each custom item is created through the editor's own path
(`store::validate` + `store::create`, then `definition_by_id`) and run
through `run::generate_definition`, the function behind the shortcut, the
editor's Preview, Retry and auto-apply, on the real `llama-server` sidecar:

```text
WB_LLAMA_SERVER=<sidecar> WB_LLM_MODEL=<muesli gguf> WB_EVAL_OUT=<run.jsonl> \
  [WB_XFORM_RULES=new|legacy] \
  cargo test --lib fork::transforms::eval -- --ignored --nocapture
```

`new` (default) gives each custom transform the rules "Create new" gives
today (none); `legacy` gives it rc3's pre-checked rules (Shorter, Clearer
wording, Better order, Keep my tone), i.e. a transform made before OT2-D.

## Item schema

`id`, `kind` (`custom` | `builtin`), `prompt` (custom) or `builtin`
(`polish` | `prompt_engineer`), `input`, `reference` (an output that must
pass; the self-test also checks that returning the input fails), `checks`,
optional `unchanged_ok` (the transform may answer "Nothing to change").

Checks: `equals`, `contains`, `not_contains`, `contains_ci`,
`not_contains_ci`, `regex`, `not_regex`, `keeps` / `keeps_ci` (`values`),
`changed`, `no_emoji`, `has_emoji`, `bullets` (`min`), `max_sentences`,
`ratio` (output/input characters, `min`/`max`), `language` (whatlang over
en, es, de, fr, pt, it, nl). An item passes when the run returns text and
every check passes.

## Results on Muesli 2B v12 (M5 Max, 2026-11-04)

| Code                           | Custom (31) | Built-in (6) |
| ------------------------------ | ----------- | ------------ |
| before (`e35fe044`), rc3 rules | 4           | 5            |
| OT2-D, new transforms          | 25          | 6            |
| OT2-D, transforms made in rc3  | 21          | 6            |

What still fails is the model, not the path: tone and length changes
("more formal" keeps "Hey", hedging removal, one sentence, half as long,
one-sentence summary) and American spelling. Those runs now end with
"Couldn't apply this transform" instead of pasting the same text.
