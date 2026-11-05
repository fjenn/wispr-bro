#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-16 (docs/fork/MAC-TEST-SCRIPT.md): one dictation into a new
# TextEdit document this script creates, through the running test app's
# real record path: SIGUSR2 (Handy's toggle) → microphone → speech model →
# text pipeline (dictionary replacements) → delivery (⌘V). The audio is a
# macOS TTS WAV the app plays as its test microphone. Holds
# ~/wb-bench/gui.lock for the whole take, checks the frontmost app before
# recording, and closes only the document it opened.
# The app plays the WAV as its test microphone (started by
# scripts/wb-s05/app.sh with WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav;
# wb_mic_load/wb_mic_play in scripts/wb-lib/app.sh): never the speakers
# unless WB_ALLOW_SPEAKERS=1.
#
# usage: scripts/wb-s16/take.sh <app pid> <wav> <out file>
#   <out> gets the document text.
set -u
PID="$1"; WAV="$2"; OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
. "$HERE/../wb-lib/app.sh"
wb_test_mic_env "$APP_DIR/Data" || exit 1
LOCK="$HOME/wb-bench/gui.lock"

if [ "${WB_S16_LOCKED:-}" != 1 ]; then
  WB_S16_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/take.sh" "$@"
fi

kill -0 "$PID" 2>/dev/null || { echo "NO APP" | tee "$OUT"; exit 3; }
front() { lsappinfo info -only bundleid "$(lsappinfo front)" | sed -E 's/.*="?([^"]*)"?/\1/'; }

doc=$(osascript -e 'tell application "TextEdit"' -e 'activate' -e 'set d to make new document' \
  -e 'return name of d' -e 'end tell')
sleep 3
f=$(front)
if [ "$f" != com.apple.TextEdit ]; then
  echo "SKIP front=$f" | tee "$OUT"
elif ! wb_mic_load "$WAV"; then
  echo "NO TEST MIC" | tee "$OUT"
else
  kill -USR2 "$PID"
  sleep 0.8
  wb_mic_play "$WAV"
  sleep 0.6
  kill -USR2 "$PID"
  sleep 9
  osascript -e "tell application \"TextEdit\" to get text of document \"$doc\"" >"$OUT"
  echo "OK $(cat "$OUT")"
fi
osascript -e "tell application \"TextEdit\" to close document \"$doc\" saving no"
exit 0
