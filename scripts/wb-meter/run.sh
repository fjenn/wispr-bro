#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Owner test 1, the microphone meter (MAC-TEST-SCRIPT "OT1.2"): plays a
# café-noise-then-speech WAV through the test microphone of a portable debug
# build with a test identity (com.wisprbro.metercheck), resumed at
# onboarding's mic test (the meter starts by itself), and records the
# meter's level sequence from the app's trace log. Never the speakers, never
# the real microphone, never the owner's app or data.
#
# usage: EXE=<src-tauri/target/debug/wispr-bro> OUT=<dir> run.sh [wav]
#   EXE   a debug build made with the metercheck identity:
#         echo '{"identifier":"com.wisprbro.metercheck","productName":"Wispr Bro Meter Check","plugins":{"updater":{"active":false}}}' > <scratch>/meter.conf.json
#         bun run tauri build --debug --no-bundle --config src-tauri/tauri.fork.conf.json --config <scratch>/meter.conf.json
#   QUIET=1  grade the speech as quiet speech / a whisper (summary.py --quiet)
#   wav   default: <OUT>/cafe-speech.wav, made by make-wav.py (4 s of café
#         noise, speech from `say` over the noise, 3 s of noise, 2 s silence)
# Holds ~/wb-bench/gui.lock.
set -u
: "${EXE:?}" "${OUT:?}"
HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/../wb-lib/app.sh"
LOCK="$HOME/wb-bench/gui.lock"
mkdir -p "$OUT"
if [ "${WB_METER_LOCKED:-}" != 1 ] && [ -d "$(dirname "$LOCK")" ]; then
  WB_METER_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$LOCK" /bin/bash "$0" "$@"
fi

DIR="$(cd "$(dirname "$EXE")" && pwd)"
DATA="$DIR/Data"
WAV="${1:-$OUT/cafe-speech.wav}"
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
pid() { pgrep -f "^$DIR/$(basename "$EXE")" | head -1; }

[ -s "$WAV" ] || python3 -I "$HERE/make-wav.py" "$WAV" || exit 1

# One copy only: this build, from its own folder.
if [ -n "$(pid)" ]; then
  log "FAIL: a copy of $EXE is already running (pid $(pid))"
  exit 1
fi

# Portable profile next to the exe (never ~/Library/Application Support).
printf "Handy Portable Mode\n" > "$DIR/portable"
mkdir -p "$DATA"
wb_unbind_keys "$DATA" >/dev/null
wb_clone_models "$DATA" >/dev/null
python3 -I - "$DATA" <<'EOF'
import json, os, sys
data = sys.argv[1]
if not data.rstrip("/").endswith("/target/debug/Data"):
    sys.exit("refusing: not a portable debug profile")
path = os.path.join(data, "fork-settings.json")
store = json.load(open(path))
s = store.setdefault("settings", {})
s["onboarding"] = {"status": "in_progress", "current_step": "mic_test"}
s["update_checks_enabled"] = False
json.dump(store, open(path, "w"), indent=2)
EOF
rm -rf "$DATA/logs"

export WISPRBRO_TEST_MIC_WAV="$DATA/test-mic.wav"
wb_mic_load "$WAV" || exit 1
secs="$(wb_wav_seconds "$WAV")"

(cd "$DIR" && "$DIR/$(basename "$EXE")" --debug >/dev/null 2>&1 &)
for _ in $(seq 1 120); do [ -n "$(pid)" ] && break; sleep 0.5; done
[ -n "$(pid)" ] || { log "FAIL: the app did not start"; exit 1; }
log "started pid $(pid)"

applog() { cat "$DATA"/logs/*.log 2>/dev/null; }
for _ in $(seq 1 60); do applog | grep -q "test microphone plays" && break; sleep 0.5; done
if ! applog | grep -q "test microphone plays"; then
  log "FAIL: the WAV microphone is not active; stopping before any audio"
  kill "$(pid)"
  exit 1
fi
if applog | grep -q "identifier.*com\.wisprbro\.desktop\b"; then
  log "FAIL: product identity"
  kill "$(pid)"
  exit 1
fi
log "WAV microphone active: $(applog | grep -m1 'test microphone plays' | sed 's/.*fork audio: //')"

for _ in $(seq 1 60); do applog | grep -q "mic test started" && break; sleep 0.5; done
applog | grep -q "mic test started" || { log "FAIL: the mic test did not start"; kill "$(pid)"; exit 1; }
sleep "$(python3 -c "print(${secs:-15} + 1)")"
kill "$(pid)"
for _ in $(seq 1 40); do [ -z "$(pid)" ] && break; sleep 0.5; done
[ -n "$(pid)" ] && kill -9 "$(pid)"

applog | grep "fork audio: meter " > "$OUT/levels.txt"
log "levels: $(wc -l < "$OUT/levels.txt" | tr -d ' ') readings in $OUT/levels.txt"
python3 -I "$HERE/summary.py" "$OUT/levels.txt" "$WAV.json" ${QUIET:+--quiet} | tee -a "$OUT/run.log"
