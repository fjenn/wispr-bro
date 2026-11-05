#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test OT2-A (docs/fork/MAC-TEST-SCRIPT.md OT2-A), unattended: N rounds
# of the held-take focus cases (take.sh). Starts the S-05 test build with
# only the trigger bound (push-to-talk and double press on Ctrl+Opt by
# default, so the owner's Wispr Bro on fn never answers; WB_OT2A_TRIGGER=fn
# when no other instance runs), stops it at the end.
#
# usage: scripts/wb-ot2a/run.sh <out dir> [rounds=1] [case …]
#   default cases: hold:cmdtab hold:click hold:finder hold:desktop free:cmdtab
# Env: WB_S05_BIN (compiled helpers, see take.sh).
set -u
OUT="${1:?out dir}"
ROUNDS="${2:-1}"
shift 2 2>/dev/null || shift $#
HERE="$(cd "$(dirname "$0")" && pwd)"
CASES=("$@")
[ ${#CASES[@]} -eq 0 ] && CASES=(hold:cmdtab hold:click hold:finder hold:desktop free:cmdtab)
case "${WB_OT2A_TRIGGER:-ctrl-opt}" in
  fn) CHORD=fn ;;
  *) CHORD=ctrl+option ;;
esac
mkdir -p "$OUT/wavs"
WAV="$OUT/wavs/speech.wav"
[ -f "$WAV" ] || say -v Samantha --data-format=LEI16@16000 -o "$WAV" \
  "Please move the meeting to Thursday afternoon and send everyone the new link."

/bin/bash "$HERE/../wb-s05/app.sh" start "" "dictate=$CHORD/hold" "hands_free=$CHORD/double_tap" || exit 1
trap '/bin/bash "$HERE/../wb-s05/app.sh" stop >/dev/null' EXIT

pass=0; fail=0; skip=0
for ((r = 1; r <= ROUNDS; r++)); do
  for c in "${CASES[@]}"; do
    line=$(/bin/bash "$HERE/take.sh" "$c" "$WAV" "$OUT")
    echo "$line" | tee -a "$OUT/results.jsonl"
    case "$line" in
      *'"result": "PASS"'*) pass=$((pass + 1)) ;;
      *'"result": "SKIP'*) skip=$((skip + 1)) ;;
      *) fail=$((fail + 1)) ;;
    esac
  done
done
echo "OT2-A RESULT: $pass pass, $fail fail, $skip skipped" | tee -a "$OUT/results.jsonl"
