#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-11 (docs/fork/MAC-TEST-SCRIPT.md): one dictation into a window
# this script creates, through the running test app's real record path:
# SIGUSR2 (Handy's toggle) → microphone → speech model → text pipeline →
# delivery. The audio is a WAV played through the speakers into the mic.
# Holds ~/wb-bench/gui.lock for the whole take (shared Mac), checks the
# frontmost app before recording, and closes only what it opened.
#
# usage: scripts/wb-s11/take.sh <textedit|safari|chrome> <wav> <out file>
#   textedit  a new TextEdit document (native text view, ⌘V);
#             <out> gets the document text
#   safari    compose.html in a new Safari window (WebKit contenteditable,
#             like a mail editor, ⌘V); needs `python3 -m http.server 47811`
#             in scripts/wb-s11 (run.sh starts it); <out> gets the title
#   chrome    chat.html in a separate Chrome instance with its own profile
#             (Enter sends, Shift+Enter is a new line; Chrome 155/156 is
#             S-04's typing path); <out> gets the window title
# The app runs as `./handy --start-hidden` from src-tauri/target/debug
# (portable profile, see the Mac test script).
set -u
TARGET="$1"; WAV="$2"; OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
LOCK="$HOME/wb-bench/gui.lock"
PORT=47811
PROFILE="${TMPDIR:-/tmp}/wb-s11-chrome-profile"

if [ "${WB_S11_LOCKED:-}" != 1 ]; then
  WB_S11_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/take.sh" "$@"
fi

batt=$(pmset -g batt | tail -1)
pct=$(echo "$batt" | sed -E 's/.*[[:space:]]([0-9]+)%.*/\1/')
if [ "$pct" -lt 15 ] && echo "$batt" | grep -q discharging; then
  echo "PAUSE battery $pct% discharging" | tee "$OUT"
  exit 4
fi

# The app's main process: the first `handy --start-hidden` whose working
# directory is our debug build (the LLM watchdog is a fork with the same
# command line, and other test apps may be running).
pid=""
for p in $(pgrep -f "handy --start-hidden"); do
  if [ -z "$pid" ] && lsof -a -p "$p" -d cwd -Fn 2>/dev/null | grep -qx "n$APP_DIR"; then
    pid=$p
  fi
done
[ -n "$pid" ] || { echo "NO APP" | tee "$OUT"; exit 3; }

front() { lsappinfo info -only bundleid "$(lsappinfo front)" | sed -E 's/.*="?([^"]*)"?/\1/'; }

doc=""
case "$TARGET" in
  textedit)
    expect=com.apple.TextEdit
    doc=$(osascript -e 'tell application "TextEdit"' -e 'activate' -e 'set d to make new document' \
      -e 'return name of d' -e 'end tell')
    ;;
  safari)
    expect=com.apple.Safari
    osascript -e 'tell application "Safari"' \
      -e "make new document with properties {URL:\"http://127.0.0.1:$PORT/compose.html\"}" \
      -e 'activate' -e 'end tell' >/dev/null
    ;;
  chrome)
    expect=com.google.Chrome
    open -na "Google Chrome" --args --user-data-dir="$PROFILE" --no-first-run \
      --no-default-browser-check --new-window "file://$HERE/chat.html"
    ;;
  *)
    echo "unknown target $TARGET" >&2
    exit 2
    ;;
esac
sleep 4
f=$(front)
if [ "$f" != "$expect" ]; then
  echo "SKIP front=$f expected=$expect" | tee "$OUT"
else
  kill -USR2 "$pid"
  sleep 0.8
  afplay "$WAV"
  sleep 0.6
  kill -USR2 "$pid"
  sleep 9
  case "$TARGET" in
    textedit) osascript -e "tell application \"TextEdit\" to get text of document \"$doc\"" >"$OUT" ;;
    safari)
      osascript -e "tell application \"Safari\" to get name of (first document whose URL contains \"$PORT/compose.html\")" >"$OUT"
      ;;
    chrome)
      osascript -e 'tell application "System Events" to get name of front window of (first process whose frontmost is true)' >"$OUT"
      ;;
  esac
  echo "OK $TARGET $(basename "$WAV")"
fi

# Close only what this take opened.
case "$TARGET" in
  textedit) osascript -e "tell application \"TextEdit\" to close document \"$doc\" saving no" ;;
  safari) osascript -e "tell application \"Safari\" to close (every document whose URL contains \"$PORT/compose.html\")" ;;
  chrome) pkill -f "user-data-dir=$PROFILE" ;;
esac
exit 0
