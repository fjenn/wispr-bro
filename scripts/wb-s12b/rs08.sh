#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# RS-08 (R5 §7): Apple Foundation Models through the shipped cleanup path.
# Needs macOS 26 with Apple Intelligence on. Writes the runs to
# eval/cleanup/reports/rs08/ (local results, gitignored) and scores them with
# the eval scorer. The shipped settings (fork::llm::apple: permissive
# guardrails, plain reply) are the default; the variants change one each.
# Usage: scripts/wb-s12b/rs08.sh [quality|app|guardrails|guided|latency|all]
set -euo pipefail
cd "$(dirname "$0")/../.."
root=$(pwd)
out="$root/eval/cleanup/reports/rs08"
mkdir -p "$out"
what=${1:-all}

run_eval() { # name, levels, then env assignments
  local name=$1 levels=$2
  shift 2
  (cd src-tauri && env WB_EVAL_ENGINE=apple WB_EVAL_LANGUAGE=owner \
    WB_EVAL_OUT="$out/$name.jsonl" WB_EVAL_LEVELS="$levels" "$@" \
    cargo test --lib fork::llm::eval_run -- --ignored --nocapture) 2>&1 | tail -3
  bun scripts/wb-eval-cleanup.ts --replay "$out/$name.jsonl" \
    --level "$([[ $levels == light ]] && echo light || echo all)" \
    --out "$out/$name" 2>&1 | tail -12
}

# Quality without the latency budget (shipped settings).
if [[ $what == quality || $what == all ]]; then
  run_eval quality-permissive-plain light,medium \
    WB_EVAL_NO_DEADLINE=1 WB_DEBUG_CLEANUP_DEADLINE_MS=20000
fi
# What the app does: R5 deadlines, prewarm 1 s before each take.
if [[ $what == app || $what == all ]]; then
  run_eval app-permissive-plain light WB_EVAL_APPLE_PREWARM_MS=1000
fi
# Refusals with the default guardrails.
if [[ $what == guardrails || $what == all ]]; then
  run_eval quality-default-plain light WB_APPLE_GUARDRAILS=default \
    WB_EVAL_NO_DEADLINE=1 WB_DEBUG_CLEANUP_DEADLINE_MS=20000
fi
# Guided generation of {text} instead of a plain reply.
if [[ $what == guided || $what == all ]]; then
  run_eval quality-permissive-guided light,medium WB_APPLE_GUIDED=1 \
    WB_EVAL_NO_DEADLINE=1 WB_DEBUG_CLEANUP_DEADLINE_MS=20000
fi
# Cold / prewarmed / warm latency on 50 words, and a 60-request burst.
if [[ $what == latency || $what == all ]]; then
  (cd src-tauri && WB_RS08_OUT="$out/latency-${WB_RS08_TAG:-plain}.json" \
    cargo test --lib fork::llm::rs08 -- --ignored --nocapture) 2>&1 | grep "rs08:" | tail -40
fi
