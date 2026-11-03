#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# One S-19b take against a synthetic browser page in an isolated test window.
# The app (scripts/wb-s05/app.sh start) plays the WAV as its test microphone
# (WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav; wb_mic_load/wb_mic_play), never
# the speakers unless WB_ALLOW_SPEAKERS=1.
# usage: scripts/wb-s19b/take.sh <chrome|safari> <url> <wav> <out dir>
set -u
BROWSER="$1"; URL="$2"; WAV="$3"; OUT="$4"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
APP_DIR="$ROOT/src-tauri/target/debug"
. "$HERE/../wb-lib/app.sh"
wb_test_mic_env "$APP_DIR/Data" || exit 1
BIN="${WB_S19B_BIN:?set WB_S19B_BIN}"
PROFILE="${TMPDIR:-/tmp}/wb-s19b-chrome-profile"
LOG="$APP_DIR/Data/logs/wispr-bro.log"
if [ "${WB_S19B_LOCKED:-}" != 1 ]; then
  WB_S19B_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$HOME/wb-bench/gui.lock" /bin/bash "$0" "$@"
fi
mkdir -p "$OUT"
T="$OUT/$(date +%H%M%S)-$BROWSER"
mkdir -p "$T"
pid=$(/bin/bash "$ROOT/scripts/wb-s05/app.sh" pid) || { echo "NO APP" | tee "$T/result"; exit 3; }
log_from=$(wc -l <"$LOG")
"$BIN/clip" save "$T/clip.saved" >/dev/null
clip_before=$("$BIN/clip" digest)
case "$BROWSER" in
  chrome)
    open -na "Google Chrome" --args --user-data-dir="$PROFILE" --no-first-run \
      --no-default-browser-check --host-resolver-rules="MAP mail.google.com 127.0.0.1, MAP app.slack.com 127.0.0.1" \
      --new-window "$URL"
    expect=com.google.Chrome
    ;;
  safari)
    osascript -e 'on run argv' -e 'tell application "Safari"' -e 'activate' \
      -e 'make new document with properties {URL:(item 1 of argv)}' -e 'end tell' -e 'end run' "$URL"
    expect=com.apple.Safari
    ;;
  *) echo "unknown browser $BROWSER" >&2; exit 2 ;;
esac
sleep 5
front() { lsappinfo info -only bundleid "$(lsappinfo front)" | sed -E 's/.*="?([^"]*)"?/\1/'; }
f=$(front)
if [ "$f" != "$expect" ]; then
  echo "SKIP front=$f" | tee "$T/result"
elif ! wb_mic_load "$WAV"; then
  echo "NO TEST MIC" | tee "$T/result"
else
  kill -USR2 "$pid"; sleep 0.8; wb_mic_play "$WAV"; sleep 0.6
  if [ "$(front)" = "$expect" ]; then kill -USR2 "$pid"; else wb_cancel "$APP_DIR" >/dev/null 2>&1; fi
  sleep 8
  echo OK >"$T/result"
fi
case "$BROWSER" in
  chrome) pkill -f "Google Chrome --user-data-dir=$PROFILE" 2>/dev/null ;;
  safari) osascript -e 'tell application "Safari" to close front document' 2>/dev/null ;;
esac
sleep 1
clip_after=$("$BIN/clip" digest)
if [ "$clip_before" = "$clip_after" ]; then echo same >"$T/clip"; else
  "$BIN/clip" restore "$T/clip.saved" >/dev/null; echo "changed (restored)" >"$T/clip"
fi
rm -rf "$T/clip.saved"
tail -n +"$((log_from + 1))" "$LOG" | /usr/bin/grep -E "context:|insert:|text:" >"$T/log"
printf '%s %s | clip %s | %s\n' "$BROWSER" "$(cat "$T/result")" "$(cat "$T/clip")" "$(grep 'context: take started' "$T/log" | tail -1)"
