# Transforms eval (OT2-D, OT2-H)

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

## OT2-H: Qwen3.5-2B, plain prompt (M5 Max under load, 2026-11-04)

13 items test the editor's options (`rules`, `examples`, `replacements` in the
item; ids `opt-*`). `WB_XFORM_LITERAL=off` runs the model alone (exact edits
become instruction lines); `WB_EVAL_IDS=a,b` runs a subset.

| Run                                | Custom (31) | Built-in (6) | Options (13) | p50 / p95    |
| ---------------------------------- | ----------- | ------------ | ------------ | ------------ |
| shipped path (exact edits by code) | 28          | 4 (Qwen)     | 13           | 308 / 670 ms |
| model only, 12-item subset         | 7/10        | 1/2          | (in custom)  | -            |

Model only, both letter swaps fail (the owner's "A and E → 0 and 1" timed
out at 2.8 s, "every o → 0" came back unchanged), so exact edits stay code.

Built-ins stay on Muesli (their OT2-D prompt and guards): 6/6 (one Polish item timed out once at load ~190 and passed on the rerun) on
Muesli; on Qwen3.5-2B they were 4/6.
