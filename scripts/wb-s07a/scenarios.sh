#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-07a.1 and S-07a.4 (docs/fork/MAC-TEST-SCRIPT.md), no audio:
# first launches of the portable test build from an empty profile against
# the local Hugging Face stand-in (`hf-mirror.py` on :18077, which this
# script switches between modes), each timed from launch to "selected".
#
# Usage: scripts/wb-s07a/scenarios.sh <dir with wispr-bro (or handy) + portable> <seed settings_store.json> <out dir> <scenario>...
# Scenarios: t50 t10 429 drop resume corrupt realhf
set -u
. "$(cd "$(dirname "$0")" && pwd)/../wb-lib/app.sh"
APP="$1"; SEED="$2"; OUT="$3"; shift 3
M=http://127.0.0.1:18077
LOG="$APP/Data/logs/wispr-bro.log"
mkdir -p "$OUT"
now() { python3 -I -c 'import time;print(time.time())'; }
mode() { curl -s "$M/_mode/$1" > /dev/null; }

fresh() {
  rm -rf "$APP/Data"; mkdir -p "$APP/Data"
  # English only (the OS here prefers en + nl), no model selected.
  python3 -I - "$SEED" "$APP/Data/settings_store.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
d["settings"]["selected_model"] = ""
d["settings"]["spoken_languages"] = ["en"]
json.dump(d, open(sys.argv[2], "w"), indent=2)
PY
}

launch() { # $1 = use mirror (1/0)
  if [ "$1" = 1 ]; then
    ( cd "$APP" && HF_ENDPOINT=$M exec "./$(wb_bin_name "$APP")" --debug --start-hidden ) > /dev/null 2>&1 &
  else
    ( cd "$APP" && exec "./$(wb_bin_name "$APP")" --debug --start-hidden ) > /dev/null 2>&1 &
  fi
  PID=$!
  T0=$(now)
}

wait_for() { # pattern timeout_s
  local end=$(( $(date +%s) + $2 ))
  while [ "$(date +%s)" -lt "$end" ]; do
    [ -f "$LOG" ] && /usr/bin/grep -q -E "$1" "$LOG" && return 0
    sleep 0.5
  done
  return 1
}

since() { python3 -I -c "print(round($(now)-$T0,1))"; }

stop() { kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null; sleep 1; }

report() { # name
  /usr/bin/grep -E "models:" "$LOG" | sed -E 's/^\[[^]]*\]\[([^]]*)\]\[[^]]*\]\[[A-Z]+\] /\1 /' > "$OUT/$1.log"
  echo "== $1"; cat "$OUT/$1.log"
}

for s in "$@"; do
  case "$s" in
    t50|t10)
      [ "$s" = t50 ] && rate=6250000 || rate=1250000
      curl -s "$M/_rate/$rate" > /dev/null
      fresh; mode normal; launch 1
      wait_for "models: selected" 400 && echo "$s: ready after $(since) s" || echo "$s: NOT ready"
      stop; report "$s" ;;
    429)
      fresh; curl -s "$M/_mode/429/20" > /dev/null; launch 1
      wait_for "next attempt in" 120; echo "429: waiting after $(since) s"
      mode normal
      wait_for "models: selected" 300 && echo "429: ready after $(since) s" || echo "429: NOT ready"
      stop; report 429 ;;
    drop)
      fresh; mode drop; launch 1
      wait_for "next attempt in" 120; echo "drop: waiting after $(since) s"
      mode normal
      wait_for "models: selected" 300 && echo "drop: ready after $(since) s" || echo "drop: NOT ready"
      stop; report drop ;;
    resume)
      curl -s "$M/_rate/1250000" > /dev/null
      fresh; mode normal; launch 1
      sleep 50; stop
      part=$(find "$APP/Data" -name "*.sync.part" -exec stat -f %z {} \; 2>/dev/null | head -1)
      echo "resume: quit after 50 s, partial ${part:-0} bytes"
      : > "$OUT/resume-mirror-mark"
      launch 1
      wait_for "models: selected" 300 && echo "resume: ready $(since) s after relaunch" || echo "resume: NOT ready"
      stop; report resume ;;
    corrupt)
      curl -s "$M/_rate/0" > /dev/null
      fresh; mode corrupt; launch 1
      wait_for "download stopped|models: selected" 300
      echo "corrupt: ended after $(since) s"
      on_disk=$(find "$APP/Data" -name "parakeet-tdt_ctc-110m-Q8_0.gguf" | wc -l | tr -d ' ')
      echo "corrupt: model files left on disk: $on_disk"
      stop; mode normal; report corrupt ;;
    realhf)
      fresh; launch 0
      wait_for "models: selected" 600 && echo "realhf: ready after $(since) s" || echo "realhf: NOT ready"
      stop; report realhf ;;
  esac
done
