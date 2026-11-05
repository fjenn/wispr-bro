#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-07b.8 (docs/fork/MAC-TEST-SCRIPT.md): the Mac wave 1 model loop
# (finding 12) must not come back. No audio, no key events: the evidence the
# real loop produced is seeded into fork.db, then the app is launched three
# times through `run.sh relaunch` against the local Hugging Face stand-in.
#
#   1. seed 10 slow warm takes for the selected unified-en model (factor
#      18x RS-09's p50, spread over 90 min, as the busy wave measured);
#   2. launch 1: the policy steps down once (`measured slower than its
#      class`), downloads the light model, swaps, deletes unified-en;
#   3. launches 2 and 3: `→ Stay`, no `downloading a better speech model`,
#      `measured_factor` kept in model_policy_state, unified-en not back.
#
# Precondition: `scripts/wb-s07b/run.sh <app> <seed> <out> fresh en` ran to
# `swapped` (unified-en selected and loaded); the app is not running.
#
# Usage: scripts/wb-s07b/loop.sh <dir with wispr-bro (or handy) + portable> <out dir>
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/../wb-lib/app.sh"
APP="$1"; OUT="$2"
DB="$APP/Data/fork.db"
mkdir -p "$OUT"
[ -f "$DB" ] || { echo "no $DB: run run.sh fresh en first"; exit 2; }
if wb_running_in "$APP"; then echo "quit the app first"; exit 2; fi

UNIFIED=$(sqlite3 "$DB" "SELECT handy_id FROM speech_models WHERE handy_id LIKE '%unified-en%' AND deleted_at IS NULL LIMIT 1")
[ -n "$UNIFIED" ] || { echo "unified-en is not on disk"; exit 2; }
P50=$(python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["models"]["parakeet-unified-en-0.6b"]["p50Ms"])' \
  "$HERE/../../src-tauri/src/fork/models/policy.fork.json")
NOW=$(( $(date +%s) * 1000 ))
for i in 0 1 2 3 4 5 6 7 8 9; do
  sqlite3 "$DB" "INSERT INTO asr_speed (handy_id, audio_ms, keyup_ms, at) VALUES ('$UNIFIED', 6000, $(( P50 * 18 )), $(( NOW - (9 - i) * 600000 )))"
done
sqlite3 "$DB" "DELETE FROM model_policy_state WHERE key = 'measured_factor'"
echo "seeded 10 takes at $(( P50 * 18 )) ms for $UNIFIED"

pass=1
LOG="$APP/Data/logs/wispr-bro.log"
downloads() { /usr/bin/grep -c "downloading a better speech model" "$LOG" 2>/dev/null || true; }
for n in 1 2 3; do
  "$HERE/run.sh" "$APP" /dev/null "$OUT/launch$n" relaunch en 600 > "$OUT/launch$n.txt" 2>&1
  pid=$(cat "$OUT/launch$n/pid" 2>/dev/null)
  sleep 20 # a late re-pick would start within this window
  [ -n "$pid" ] && kill "$pid" 2>/dev/null
  sleep 5
  case $n in
    1)
      /usr/bin/grep -q "measured slower than its class" "$OUT/launch1/models.log" || { echo "FAIL launch 1: no slow verdict"; pass=0; }
      /usr/bin/grep -q "models: swapped" "$OUT/launch1/models.log" || { echo "FAIL launch 1: no step down"; pass=0; }
      D1=$(downloads)
      ;;
    *)
      if /usr/bin/grep -q "downloading a better speech model" "$OUT/launch$n/models.log"; then
        echo "FAIL launch $n: downloaded again"; pass=0
      fi
      /usr/bin/grep -q -E "→ Stay" "$OUT/launch$n/models.log" || { echo "FAIL launch $n: no Stay"; pass=0; }
      ;;
  esac
  echo "launch $n: $(/usr/bin/grep -E '^result' "$OUT/launch$n.txt")"
done
[ "$(downloads)" = "${D1:-x}" ] || { echo "FAIL: a download started after launch 1"; pass=0; }
sqlite3 "$DB" "SELECT value FROM model_policy_state WHERE key = 'measured_factor'" | /usr/bin/grep -q "@" \
  || { echo "FAIL: measured_factor not kept"; pass=0; }
if sqlite3 "$DB" "SELECT 1 FROM speech_models WHERE handy_id = '$UNIFIED' AND deleted_at IS NULL" | /usr/bin/grep -q 1; then
  echo "FAIL: unified-en came back"; pass=0
fi
sqlite3 "$DB" "SELECT handy_id, deleted_at IS NOT NULL FROM speech_models; SELECT key, value FROM model_policy_state;" | tee "$OUT/db.txt"
[ "$pass" = 1 ] && echo "PASS S-07b.8" || { echo "FAIL S-07b.8"; exit 1; }
