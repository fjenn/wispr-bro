#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# S-38 §5.1 latency bench, headless (no GUI, no speakers): TTS takes written
# to files, the shipped recognizer/pipeline/cleanup/transform code on the
# real sidecar (`fork::perf_bench`), then PASS/FAIL per §5.1 row.
#
#   scripts/wb-perf/latency.sh [out dir]
#
# Environment (defaults in brackets):
#   WB_ASR_MODEL     speech model GGUF [~/wb-bench/asr/parakeet-unified-en-0.6b/…Q8_0.gguf]
#   WB_LLM_MODEL     cleanup model GGUF [~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf]
#   WB_LLAMA_SERVER  sidecar [src-tauri/target/wb-sidecar/llama-server-aarch64-apple-darwin,
#                    else the main checkout's, else ~/wb-bench's upstream b11624]
#   WB_PERF_COLD_RUNS  speech-model cold loads [5]
#   WB_PERF_NO_LLM=1   recognizer and pipeline rows only
# Results stay under the out dir [~/wb-bench/results/perf-<date>] (never committed).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${1:-$HOME/wb-bench/results/perf-$(date +%Y%m%d-%H%M%S)}"
mkdir -p "$OUT/audio"
: "${WB_ASR_MODEL:=$HOME/wb-bench/asr/parakeet-unified-en-0.6b/parakeet-unified-en-0.6b-Q8_0.gguf}"
: "${WB_LLM_MODEL:=$HOME/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf}"
if [ -z "${WB_LLAMA_SERVER:-}" ]; then
  for c in "$ROOT/src-tauri/target/wb-sidecar/llama-server-aarch64-apple-darwin" \
           "$HOME/Projects/wispr-bro/src-tauri/target/wb-sidecar/llama-server-aarch64-apple-darwin" \
           "$HOME/wb-bench/llama.cpp/b11624-bin/llama-b11624/llama-server"; do
    if [ -x "$c" ]; then WB_LLAMA_SERVER="$c"; break; fi
  done
fi
bun "$ROOT/scripts/wb-perf/takes.ts" "$OUT/audio"
export WB_ASR_MODEL WB_PERF_AUDIO="$OUT/audio" WB_PERF_OUT="$OUT/run.jsonl"
if [ "${WB_PERF_NO_LLM:-}" != "1" ] && [ -n "${WB_LLAMA_SERVER:-}" ]; then
  export WB_LLAMA_SERVER WB_LLM_MODEL
else
  unset WB_LLAMA_SERVER WB_LLM_MODEL
fi
(cd "$ROOT/src-tauri" && cargo test --lib fork::perf_bench -- --ignored --nocapture)
bun "$ROOT/scripts/wb-perf/latency.ts" "$OUT/run.jsonl" | tee "$OUT/report.md"
