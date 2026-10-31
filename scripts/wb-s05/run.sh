#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-05 (docs/fork/MAC-TEST-SCRIPT.md S-05), unattended: N rounds
# of the S-05 cases through the running test app (scripts/wb-s05/app.sh).
# Each take holds ~/wb-bench/gui.lock on its own (take.sh), so other GUI
# runs interleave between takes. Prints one JSON line per take and a total.
#
# usage: scripts/wb-s05/run.sh <out dir> [rounds=1] [case …]
#   default cases: the 0-false-card fields (TextEdit, Safari input /
#   textarea / contenteditable / search, Chrome input / textarea /
#   contenteditable), the cards (Finder, desktop, password), S-05.4
#   (switch) and S-05.5 (moved:40).
# Env: WB_S05_BIN (compiled helpers, see take.sh).
set -u
[ -n "${WB_S05_TRACE:-}" ] && set -x
OUT="${1:?out dir}"
ROUNDS="${2:-1}"
shift 2 2>/dev/null || shift $#
HERE="$(cd "$(dirname "$0")" && pwd)"
CASES=("$@")
if [ ${#CASES[@]} -eq 0 ]; then
  CASES=(field:textedit field:safari:input field:safari:textarea field:safari:editable
    field:safari:search field:chrome:input field:chrome:textarea field:chrome:editable
    card:finder card:desktop card:password switch moved:40)
fi
mkdir -p "$OUT/wavs"
WAV="$OUT/wavs/speech.wav"
[ -f "$WAV" ] || say -v Samantha --data-format=LEI16@16000 -o "$WAV" \
  "Please send the report to the team before lunch, and copy me on the reply."

(cd "$HERE" && exec python3 -I -m http.server 47815 --bind 127.0.0.1 >/dev/null 2>&1) &
server=$!
trap 'kill $server 2>/dev/null' EXIT
sleep 1

pass=0
fail=0
skip=0
for ((r = 1; r <= ROUNDS; r++)); do
  for c in "${CASES[@]}"; do
    line=$(/bin/bash "$HERE/take.sh" "$c" "$WAV" "$OUT")
    echo "$line" | tee -a "$OUT/results.jsonl"
    case "$line" in
      *'"result": "PASS"'*) pass=$((pass + 1)) ;;
      *'"result": "SKIP"'*) skip=$((skip + 1)) ;;
      *) fail=$((fail + 1)) ;;
    esac
  done
done
echo "S-05 RESULT: $pass pass, $fail fail, $skip skipped" | tee -a "$OUT/results.jsonl"
