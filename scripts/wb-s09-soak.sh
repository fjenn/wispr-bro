#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-09.1 (docs/fork/MAC-TEST-SCRIPT.md): N dictations spread over
# five apps (TextEdit, Notes, Mail, Messages, Claude), through the running
# app's real record path. For each take: bring the app to the front, check it
# is frontmost, load a WAV into the app's test microphone, start recording
# with SIGUSR2 (Handy's toggle), wait the WAV's length, stop with SIGUSR2.
# Never the speakers unless WB_ALLOW_SPEAKERS=1. The app's paste method
# should be "none" so nothing is typed into those apps.
#
# Usage: scripts/wb-s09-soak.sh <segments dir> [takes=50]
#   The app must run as `./wispr-bro --start-hidden` (or `./handy`) from
#   src-tauri/target/debug (portable profile, see the Mac test script),
#   started by `scripts/wb-s05/app.sh start` (shortcuts unbound, model cache
#   cloned, WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav). Every take holds
#   ~/wb-bench/gui.lock (shared Mac).
set -u
. "$(cd "$(dirname "$0")" && pwd)/wb-lib/app.sh"
wb_test_mic_env "$(cd "$(dirname "$0")/../src-tauri/target/debug" && pwd)/Data" || exit 1
SEGMENTS="${1:?segments dir with en-sentences-01..12.wav}"
N="${2:-50}"
LOCK="$HOME/wb-bench/gui.lock"
APPS=(TextEdit Notes Mail Messages Claude)
IDS=(com.apple.TextEdit com.apple.Notes com.apple.mail com.apple.MobileSMS com.anthropic.claudefordesktop)

take() {
  local app="$1" wav="$2" expect="$3" pid front
  pid=$(pgrep -f "^./$WB_BIN_RE --start-hidden" | head -1)
  [ -n "$pid" ] || { echo "NO APP"; return 3; }
  open -a "$app"
  sleep 1.5
  front=$(lsappinfo info -only bundleid "$(lsappinfo front)" | sed -E 's/.*="?([^"]*)"?/\1/')
  if [ "$front" != "$expect" ]; then
    echo "SKIP front=$front expected=$expect"
    return 2
  fi
  wb_mic_load "$wav" || { echo "NO TEST MIC"; return 4; }
  kill -USR2 "$pid"
  sleep 0.6
  wb_mic_play "$wav"
  sleep 0.4
  kill -USR2 "$pid"
  sleep 3
  echo "OK $app $(basename "$wav")"
}

if [ "${WB_S09_TAKE:-}" = 1 ]; then
  take "$@"
  exit $?
fi

for ((i = 0; i < N; i++)); do
  a=$((i % 5))
  wav="$SEGMENTS/en-sentences-$(printf "%02d" $(((i % 12) + 1))).wav"
  for try in 1 2 3; do
    WB_S09_TAKE=1 /usr/bin/lockf -k -t 600 "$LOCK" "$0" "${APPS[$a]}" "$wav" "${IDS[$a]}"
    rc=$?
    [ $rc = 0 ] && break
    [ $rc = 4 ] && exit 4
    [ "$try" = 3 ] && echo "GAVE UP take $i"
    sleep 5
  done
  echo "take $i done ($(date +%T))"
done
echo SOAK-END
