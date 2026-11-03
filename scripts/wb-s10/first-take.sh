#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-10.9 (docs/fork/MAC-TEST-SCRIPT.md S-10): the first take after
# a fresh start, while the launch warm-up of the LLM sidecar runs (Mac wave
# 3 finding 3: the warm-up stopped the server just as the take's prewarm got
# it, the take then waited 9 s behind a second cold start; text pasted 19 s
# after the stop).
#
# The warm marker (`Data/llm/warm-*`) is removed first, so the app warms the
# sidecar up at launch as after an install or update. One SIGUSR2 take of a
# pre-recorded WAV through the test microphone (never the speakers) starts
# the moment the cleanup model is ready (when the warm-up starts), into our
# own TextEdit.
#
# usage: APP=<test bundle.app> OUT=<dir> scripts/wb-s10/first-take.sh
#   APP: a debug test bundle (`com.wisprbro.<slice>check`, portable
#   `Contents/MacOS/Data`) whose profile finished onboarding and has the
#   speech and cleanup models (any earlier Mac-wave copy; the model cache is
#   cloned in when missing).
# PASS lines:
#   S-10.9a the text is inserted ≤ 8 s after the take stops
#   S-10.9b no "llama-server stopped" between the warm-up's start and the
#           take's cleanup (the warm-up kept its server for the take)
#   S-10.9c at most one "llama-server ready" before the take's cleanup
#           (no second cold start)
#   S-10.9d the take was pasted into our TextEdit (the insert line names
#           com.apple.TextEdit; the app starts hidden and the take starts
#           only with our TextEdit in front)
set -u
: "${APP:?}" "${OUT:?}"
LOCK="$HOME/wb-bench/gui.lock"
HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/../wb-lib/app.sh"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
DATA="$APP/Contents/MacOS/Data"
LOGS="$DATA/logs"

if [ "${WB_S10_LOCKED:-}" != 1 ]; then
  WB_S10_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$0" "$@"
fi

mkdir -p "$OUT"
pid() { pgrep -f "$EXE" | head -1; }
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
applog() { cat "$LOGS"/*.log 2>/dev/null; }
until_log() { # <pattern> <timeout s>
  local end=$(($(date +%s) + $2))
  while [ "$(date +%s)" -lt "$end" ]; do
    applog | grep -q -- "$1" && return 0
    sleep 0.2
  done
  return 1
}
FAILED=0
check() { # <label> <command...>
  local label="$1"; shift
  if "$@"; then log "PASS $label"; else log "FAIL $label"; FAILED=1; fi
}

[ -n "$(pid)" ] && { log "FAIL the test app is already running"; exit 1; }
# Refuses anything but a test identity's portable data.
wb_unbind_keys "$DATA" || exit 1
wb_clone_models "$DATA" "${WB_MODEL_CACHE:-}"
rm -f "$DATA"/llm/warm-*
mkdir -p "$LOGS"
for f in "$LOGS"/*.log; do [ -f "$f" ] && : >"$f"; done

WAV="$OUT/first-take.wav"
wb_test_mic_wav "Please send the quarterly report to the team by Friday." "$WAV" || exit 1
wb_test_mic "$OUT" "$WAV" || exit 1
printf '' >"$OUT/first-take.txt"
TE=$(wb_textedit_open "$OUT/first-take.txt") || { log "FAIL no TextEdit"; exit 1; }
cleanup() {
  [ -n "$(pid)" ] && kill "$(pid)" 2>/dev/null
  wb_textedit_close "$TE"
}
trap cleanup EXIT

# A debug build writes `../src/bindings.ts` relative to its working
# directory at start: run it from a folder of ours, never the checkout's
# parent (Mac wave 4).
mkdir -p "$OUT/cwd/run"
# Hidden: a launch with the Hub shown takes the front from TextEdit and the
# take pastes into the app itself (Mac wave 4, first run of this driver).
(cd "$OUT/cwd/run" && exec "$EXE" --start-hidden) >"$OUT/app-stdout.log" 2>&1 &
log "launched; waiting for the cleanup model"
front_te() { # our TextEdit in front, checked by pid
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $TE) to true" >/dev/null 2>&1
  sleep 0.3
  [ "$(osascript -e 'tell application "System Events" to get unix id of first process whose frontmost is true' 2>/dev/null)" = "$TE" ]
}
until_log "startup: ready" 120
front_te || { log "FAIL our TextEdit ($TE) is not in front; no take"; exit 1; }
# A warm-up with the GPU shaders already compiled takes ~0.4 s, less than
# two osascript calls: everything slow happens before the model is ready,
# and the take starts at once. (A bundle copied to a new path compiles its
# Metal shaders again, as after an install: a several-second warm-up.)
until_log "llm: .* ready (" 600 || { log "FAIL the cleanup model never became ready"; exit 1; }
kill -USR2 "$(pid)"
log "model ready; the warm-up starts now; take 1 started"
sleep 0.8
wb_mic_play "$WAV"
sleep 0.4
front_te || { kill "$(pid)"; log "FAIL our TextEdit lost the front during the take; app quit before the stop, nothing pasted"; exit 1; }
kill -USR2 "$(pid)"
STOP=$(now)
# The delivery line ("insert: take 1 (Take) pasted via … into <bundle>"),
# not the earlier "insert: take 1 target …" decision line.
if until_log "insert: take 1 (Take) " 60; then
  WAITED=$(perl -e "printf '%.1f', $(now) - $STOP")
else
  WAITED=60
fi
log "inserted ${WAITED} s after the stop"
applog >"$OUT/app.log"

# The lines from the warm-up's start to the take's cleanup.
WINDOW="$(sed -n '/llm: .* ready (/,/cleanup: /p' "$OUT/app.log")"
check "S-10.9a inserted ≤ 8 s after the stop (${WAITED} s)" perl -e "exit !($WAITED <= 8)"
check "S-10.9b the warm-up did not stop the take's server" \
  bash -c '! grep -q "llama-server stopped" <<<"$1"' _ "$WINDOW"
check "S-10.9c no second cold start before the take's cleanup" \
  bash -c '[ "$(grep -c "llama-server ready in" <<<"$1")" -le 1 ]' _ "$WINDOW"
check "S-10.9d the text went to our TextEdit" \
  bash -c 'grep "insert: take 1 (Take) " <<<"$1" | grep -q "into com.apple.TextEdit "' _ "$(applog)"
grep -E "warm-up|llama-server (ready|stopped)|cleanup: |insert: take 1" "$OUT/app.log" | cut -c1-220 | tee -a "$OUT/run.log"
exit "$FAILED"
