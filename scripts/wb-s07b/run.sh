#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-07b (docs/fork/MAC-TEST-SCRIPT.md): the silent model policy on
# the portable test build (identity com.wisprbro.s07bcheck) against the
# local Hugging Face stand-in (`scripts/wb-s07a/hf-mirror.py`, default :18177, `S07B_MIRROR`).
#
#   fresh <langs>   empty Data/, Handy's spoken_languages = <langs> (comma
#                   list), launch: starter → ready → policy pick in the
#                   background → swap → superseded model deleted
#   relaunch <langs> keep Data/, change spoken_languages, launch: re-pick
#
# Each launch waits for `models: swapped` or `policy … → Stay` and prints the
# models lines of the log, the files in the model cache and the fork.db
# rows. Takes: `scripts/wb-s07a/take.sh <pid> <aiff>` while it runs (holds
# the GUI lock for the take only). No key events are posted.
#
# Usage: scripts/wb-s07b/run.sh <dir with wispr-bro (or handy) + portable> <seed settings_store.json> <out dir> fresh|relaunch <langs> [timeout_s]
set -u
. "$(cd "$(dirname "$0")" && pwd)/../wb-lib/app.sh"
APP="$1"; SEED="$2"; OUT="$3"; WHAT="$4"; LANGS="$5"; TIMEOUT="${6:-900}"
M="${S07B_MIRROR:-http://127.0.0.1:18177}"
LOG="$APP/Data/logs/wispr-bro.log"
mkdir -p "$OUT"

settings() { # $1 = source settings_store.json, $2 = target
  python3 -I - "$1" "$2" "$LANGS" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
s = d["settings"]
s["spoken_languages"] = [l for l in sys.argv[3].split(",") if l]
json.dump(d, open(sys.argv[2], "w"), indent=2)
PY
}

case "$WHAT" in
  fresh)
    rm -rf "$APP/Data"; mkdir -p "$APP/Data"
    settings "$SEED" "$APP/Data/settings_store.json"
    python3 -I -c 'import json,sys;p=sys.argv[1];d=json.load(open(p));d["settings"]["selected_model"]="";json.dump(d,open(p,"w"),indent=2)' "$APP/Data/settings_store.json"
    ;;
  relaunch)
    cp "$APP/Data/settings_store.json" "$OUT/settings.before.json"
    settings "$OUT/settings.before.json" "$APP/Data/settings_store.json"
    ;;
  *) echo "fresh|relaunch"; exit 2 ;;
esac
LINES0=$( [ -f "$LOG" ] && wc -l < "$LOG" || echo 0 )
T0=$(date +%s)
( cd "$APP" && HF_ENDPOINT=$M exec "./$(wb_bin_name "$APP")" --debug --start-hidden ) > "$OUT/stdout.log" 2>&1 &
PID=$!
echo "pid $PID"
echo "$PID" > "$OUT/pid"
end=$(( T0 + TIMEOUT ))
result=timeout
while [ "$(date +%s)" -lt "$end" ]; do
  if [ -f "$LOG" ]; then
    new=$(tail -n +"$((LINES0 + 1))" "$LOG")
    if echo "$new" | /usr/bin/grep -q "models: swapped"; then
      # Give the cleanup a moment.
      sleep 3; result=swapped; break
    fi
    if echo "$new" | /usr/bin/grep -q -E "models: policy .*→ Stay"; then
      result=stay; break
    fi
  fi
  sleep 1
done
echo "result $result after $(( $(date +%s) - T0 )) s"
tail -n +"$((LINES0 + 1))" "$LOG" \
  | /usr/bin/grep -E "models:|fork tray: model items" \
  | sed -E 's/^\[[^]]*\]//' > "$OUT/models.log"
cat "$OUT/models.log"
echo "--- model files"
find "$APP/Data/huggingface" -name "*.gguf" -type l -o -name "*.gguf" -type f 2>/dev/null \
  | sed "s|$APP/Data/||" | tee "$OUT/files.txt"
echo "--- fork.db"
sqlite3 "$APP/Data/fork.db" \
  "SELECT handy_id, origin, loaded_at IS NOT NULL, superseded_at IS NOT NULL, deleted_at IS NOT NULL FROM speech_models; SELECT key, value FROM model_policy_state;" \
  | tee "$OUT/db.txt"
echo "PID=$PID (still running; kill it when done)"
