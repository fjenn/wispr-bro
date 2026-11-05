#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# S-19a: each style preset applied (stage 10) to a finished S-12a replay run
# without a style, then scored with the RS-07 gates. No preset changes the
# cleanup prompt, so this is what a styled take inserts, free of model
# timing noise (`fork::llm::eval_run::eval_restyle`).
#
# usage: scripts/wb-s19a/restyle.sh <run.jsonl> <out dir>
set -u
IN="$1"; OUT="$2"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
mkdir -p "$OUT"
for style in formal relaxed very_relaxed upbeat; do
  (cd "$ROOT/src-tauri" && WB_EVAL_IN="$IN" WB_EVAL_STYLE="$style" WB_EVAL_OUT="$OUT/run-$style.jsonl" \
    cargo test --lib fork::llm::eval_run::eval_restyle -- --ignored --nocapture >"$OUT/restyle-$style.log" 2>&1)
  (cd "$ROOT" && bun scripts/wb-eval-cleanup.ts --replay "$OUT/run-$style.jsonl" --level all >"$OUT/score-$style.txt" 2>&1)
  echo "$style: $(grep -E '^(light|medium):' "$OUT/score-$style.txt" | tr '\n' ' ')"
done
