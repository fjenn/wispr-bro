#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-07a (docs/fork/MAC-TEST-SCRIPT.md): one first launch of the
# portable test build from an empty Data/ dir, timed from launch to
# "speech model ready", optionally with a take (`take.sh`) N seconds after
# launch. The mirror (`hf-mirror.py`) must already run when HF_ENDPOINT is
# set; leave HF_ENDPOINT unset to download from huggingface.co.
#
# Usage: scripts/wb-s07a/run.sh <dir with wispr-bro (or handy) + portable> <out dir> [take_at_s] [aiff]
# Env:   HF_ENDPOINT, S07A_KEEP_DATA=1 (do not wipe Data/), S07A_TIMEOUT (s, 400)
set -u
. "$(cd "$(dirname "$0")" && pwd)/../wb-lib/app.sh"
APP="$1"; OUT="$2"; TAKE_AT="${3:-}"; AUDIO="${4:-}"
HERE="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$OUT"
[ "${S07A_KEEP_DATA:-}" = 1 ] || rm -rf "$APP/Data"
LOG="$APP/Data/logs/wispr-bro.log"
CB0=$(pbpaste | shasum -a 256 | cut -c1-16)
T0=$(python3 -I -c 'import time;print(time.time())')
( cd "$APP" && exec "./$(wb_bin_name "$APP")" --debug ) > "$OUT/stdout.log" 2>&1 &
PID=$!
echo "pid $PID t0 $T0"
deadline=$(( $(date +%s) + ${S07A_TIMEOUT:-400} ))
took=0
while [ "$(date +%s)" -lt "$deadline" ]; do
  now=$(python3 -I -c 'import time;print(time.time())')
  if [ -n "$TAKE_AT" ] && [ $took = 0 ] && python3 -I -c "import sys;sys.exit(0 if $now-$T0>=$TAKE_AT else 1)"; then
    "$HERE/take.sh" "$PID" "$AUDIO"; took=1
    sleep 1.5
    # Only a strip around the bar (never the whole screen).
    WB_PID=$PID "${WBPILL:-/tmp/wbpill}" -e "shot $OUT/setting-up.png" > /dev/null 2>&1 || true
  fi
  if [ -f "$LOG" ] && /usr/bin/grep -q "models: selected" "$LOG"; then
    t=$(python3 -I -c "import time;print(round(time.time()-$T0,1))")
    echo "ready after ${t}s"
    break
  fi
  sleep 0.5
done
# Leave time for a queued take to reach the card.
if [ $took = 1 ]; then
  sleep 8
  WB_PID=$PID "${WBPILL:-/tmp/wbpill}" -e "shot $OUT/card.png" > /dev/null 2>&1 || true
fi
CB1=$(pbpaste | shasum -a 256 | cut -c1-16)
echo "clipboard before $CB0 after $CB1 $([ "$CB0" = "$CB1" ] && echo unchanged || echo CHANGED)"
/usr/bin/grep -E "models:|pill: (Retry|card)|probe" "$LOG" | sed -E 's/^\[[^]]*\]//' > "$OUT/models.log"
cat "$OUT/models.log"
echo "PID=$PID (still running; kill it when done)"
