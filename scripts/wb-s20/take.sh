#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-20 (docs/fork/MAC-TEST-SCRIPT.md S-20): one take through the
# running test app's real record path (SIGUSR2, Handy's toggle → context
# read at record start → microphone → speech model → text pipeline →
# cleanup → paste), into a window this script opens. Speech is a WAV played
# through the speakers into the mic (`none`: a silent take, which still
# makes the context read).
#
# Holds ~/wb-bench/gui.lock for the whole take, checks the frontmost app
# before recording and before the stop (cancels otherwise), closes only
# what it opened, and saves and restores the whole clipboard (byte-
# identical, checked with scripts/wb-s05/clip.swift).
#
# usage: scripts/wb-s20/take.sh <case> <wav|none> <out dir>
#   textedit   a new TextEdit document holding a message that names the
#              people; the caret on a new line after it
#   chrome     scripts/wb-s20/thread.html in a separate Chrome instance,
#              the thread's reply box focused
#   card       the same page with its "Card number" field focused
#   terminal   a new Terminal window running `cat >/dev/null`
# Env: WB_S20_BIN = folder with the compiled helpers
#   (swiftc -O -o $WB_S20_BIN/clip scripts/wb-s05/clip.swift).
set -u
CASE="$1"; WAV="$2"; OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
. "$HERE/../wb-lib/app.sh"
BIN="${WB_S20_BIN:?set WB_S20_BIN}"
LOCK="$HOME/wb-bench/gui.lock"
PROFILE="${TMPDIR:-/tmp}/wb-s20-chrome-profile"
PAGE="file://$HERE/thread.html"
LOG="$APP_DIR/Data/logs/wispr-bro.log"

if [ "${WB_S20_LOCKED:-}" != 1 ]; then
  WB_S20_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/take.sh" "$@"
fi

mkdir -p "$OUT"
T="$OUT/$(date +%H%M%S)-$CASE"
mkdir -p "$T"
pid=$(/bin/bash "$HERE/../wb-s05/app.sh" pid) || { echo "NO APP" | tee "$T/result"; exit 3; }
log_from=$(wc -l <"$LOG")

front() { lsappinfo info -only bundleid "$(lsappinfo front)" | sed -E 's/.*="?([^"]*)"?/\1/'; }
"$BIN/clip" save "$T/clip.saved" >/dev/null
clip_before=$("$BIN/clip" digest)

opened=""
expect=""
case "$CASE" in
  textedit)
    doc=$(osascript -e 'tell application "TextEdit"' -e 'activate' \
      -e 'set d to make new document' \
      -e 'set text of d to "From Siobhán Kowalczyk: the launch checklist is ready. Thibault Moreau and Zebulonix Quarterbridge review it next." & return & return' \
      -e 'return name of d' -e 'end tell')
    osascript -e 'tell application "System Events" to key code 125 using command down' # ⌘↓: caret after the text (a synthetic End leaves the fn flag set)
    opened=textedit; expect=com.apple.TextEdit
    ;;
  chrome|card)
    hash=""; [ "$CASE" = card ] && hash="#card"
    open -na "Google Chrome" --args --user-data-dir="$PROFILE" --no-first-run \
      --no-default-browser-check --force-renderer-accessibility --new-window "$PAGE$hash"
    opened=chrome; expect=com.google.Chrome
    ;;
  terminal)
    osascript -e 'tell application "Terminal"' -e 'activate' \
      -e 'do script "echo S20 context check; cat >/dev/null"' -e 'end tell' >/dev/null
    opened=terminal; expect=com.apple.Terminal
    ;;
  *) echo "unknown case $CASE" >&2; exit 2 ;;
esac
sleep 3
chrome_pid() { pgrep -f "Google Chrome --user-data-dir=$PROFILE" | head -1; }
chrome_title() {
  osascript -e "tell application \"System Events\" to get name of front window of (first process whose unix id is $(chrome_pid))" 2>/dev/null
}

f=$(front)
if [ "$f" != "$expect" ]; then
  echo "SKIP front=$f" | tee "$T/result"
else
  kill -USR2 "$pid"
  sleep 0.8
  if [ "$WAV" != none ]; then afplay "$WAV"; else sleep 1.5; fi
  sleep 0.6
  if [ "$(front)" = "$expect" ]; then
    kill -USR2 "$pid"
  else
    wb_cancel "$APP_DIR" >/dev/null 2>&1
    echo "focus moved; cancelled" >"$T/skip"
  fi
  sleep 9
  case "$opened" in
    textedit) osascript -e "tell application \"TextEdit\" to get text of document \"$doc\"" >"$T/text" ;;
    chrome) chrome_title | sed 's/^S20://' >"$T/text" ;;
    terminal) : >"$T/text" ;;
  esac
  echo "OK" >"$T/result"
fi
case "$opened" in
  textedit) osascript -e "tell application \"TextEdit\" to close document \"$doc\" saving no" ;;
  chrome) kill "$(chrome_pid)" 2>/dev/null ;;
  terminal) osascript -e 'tell application "Terminal" to close (every window whose name contains "cat")' 2>/dev/null ;;
esac
sleep 1
clip_after=$("$BIN/clip" digest)
if [ "$clip_before" = "$clip_after" ]; then echo same >"$T/clip"; else
  "$BIN/clip" restore "$T/clip.saved" >/dev/null; echo "changed (restored)" >"$T/clip"; fi
rm -rf "$T/clip.saved" # the owner's clipboard, never kept
tail -n +"$((log_from + 1))" "$LOG" | /usr/bin/grep -E "context:|cleanup:|text:|insert:" >"$T/log"
echo "$CASE $(cat "$T/result") | clip $(cat "$T/clip") | $(cat "$T/text" 2>/dev/null | tr '\n' ' ')"
