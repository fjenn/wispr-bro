#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# S-19a: the S-12a cleanup replay through the shipped path, without a style
# (the RS-07 gates must not regress) and with each style preset (stage 10
# applied to the output; Upbeat also changes the prompt), Light and Medium,
# the owner's languages. Outputs go to $1 (outside the repo).
#
# usage: scripts/wb-s19a/replay.sh <out dir> [none|formal|relaxed|very_relaxed|upbeat ...]
set -u
OUT="$1"; shift
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
mkdir -p "$OUT"
styles="${*:-none upbeat relaxed very_relaxed}"
for style in $styles; do
  (
    cd "$ROOT/src-tauri" || exit 1
    if [ "$style" = none ]; then unset WB_EVAL_STYLE; else export WB_EVAL_STYLE="$style"; fi
    WB_LLAMA_SERVER="$ROOT/src-tauri/target/wb-sidecar/llama-server-aarch64-apple-darwin" \
      WB_LLM_MODEL="$HOME/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf" \
      WB_EVAL_OUT="$OUT/run-$style.jsonl" \
      WB_EVAL_LEVELS=light,medium WB_EVAL_LANGUAGE=owner \
      cargo test --lib fork::llm::eval_run -- --ignored --nocapture >"$OUT/run-$style.log" 2>&1
  )
  (cd "$ROOT" && bun scripts/wb-eval-cleanup.ts --replay "$OUT/run-$style.jsonl" --level all >"$OUT/score-$style.txt" 2>&1)
  echo "$style: $(grep -c . "$OUT/run-$style.jsonl") rows"
done
