#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-05 (docs/fork/MAC-TEST-SCRIPT.md S-05): one dictation through
# the running test app's real record path (SIGUSR2, Handy's toggle →
# microphone → speech model → text pipeline → fork::insert target check →
# paste or card), into windows this script opens. The speech is a WAV the
# app plays as its test microphone (started by scripts/wb-s05/app.sh with
# WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav; wb_mic_load/wb_mic_play in
# scripts/wb-lib/app.sh), never the speakers unless WB_ALLOW_SPEAKERS=1.
#
# Holds ~/wb-bench/gui.lock for the whole take, checks the frontmost app
# before recording, closes only what it opened, and saves and restores the
# whole clipboard around every card case (byte-identical, checked).
#
# usage: scripts/wb-s05/take.sh <case> <wav> <out dir>
#   field:textedit            new TextEdit document → text pasted, no card
#   field:safari:<id>         fields.html#<id> in a new Safari window
#   field:chrome:<id>         fields.html#<id> in a separate Chrome instance
#   card:finder               our own Finder window → card, Copy → clipboard
#   card:desktop              the desktop (an empty spot clicked) → card, ✕
#   card:password             fields.html#password in Safari → refused card
#                             with hidden text, Copy → concealed clipboard
#   switch                    take starts in Chrome, TextEdit is activated
#                             before the stop → text in TextEdit (S-05.4)
#   moved:<ms>                take in TextEdit, stopped, Safari activated
#                             <ms> later → card "focus moved" (S-05.5)
# Env: WB_S05_BIN = folder with the compiled helpers (clip, drive, pill:
#   swiftc -O -o $WB_S05_BIN/<name> scripts/wb-s05/<name>.swift,
#   scripts/wb-s03-pill.swift).
set -u
CASE="$1"; WAV="$2"; OUT="$3"
[ -n "${WB_S05_TRACE:-}" ] && set -x
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
. "$HERE/../wb-lib/app.sh"
wb_test_mic_env "$APP_DIR/Data" || exit 1
BIN="${WB_S05_BIN:?set WB_S05_BIN}"
LOCK="$HOME/wb-bench/gui.lock"
PORT=47815
PROFILE="${TMPDIR:-/tmp}/wb-s05-chrome-profile"
LOG="$APP_DIR/Data/logs/wispr-bro.log"
PAGE="http://127.0.0.1:$PORT/fields.html"
# The card (S-07a's 400 x 58 pt shape since the S-05 rebase): its height,
# and Copy / close centres from its bottom-left corner. The S-05 run used
# S-05's earlier 360 x 132 card; these values are not re-measured yet.
CARD_H=58
CARD_MID=29
COPY_X=322
CLOSE_X=379

if [ "${WB_S05_LOCKED:-}" != 1 ]; then
  WB_S05_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/take.sh" "$@"
fi

mkdir -p "$OUT"
# The take's WAV into the test microphone (refuses the speakers).
wb_mic_load "$WAV" || { echo "{\"case\":\"$CASE\",\"result\":\"NO TEST MIC\"}"; exit 4; }
n=$(($(ls "$OUT" | grep -c '^take-') + 1))
T="$OUT/take-$(printf %03d $n)-${CASE//:/_}"
mkdir -p "$T"
pid=$(/bin/bash "$HERE/app.sh" pid) || { echo "{\"case\":\"$CASE\",\"result\":\"NO APP\"}"; exit 3; }
log_from=$(wc -l <"$LOG")

front() { "$BIN/drive" front | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["front"])'; }
pillwin() { WB_PID=$pid "$BIN/pill" -e windows; }
# Height of the active bar window (0 when only the resting bar shows).
active_h() {
  pillwin | python3 -I -c 'import json,sys
d=json.loads(sys.stdin.readline())
print(int(max([w["h"] for w in d["windows"] if w["h"]>=30] or [0])))'
}
wait_card() {
  for _ in $(seq 1 60); do
    [ "$(active_h)" = "$CARD_H" ] && return 0
    sleep 0.2
  done
  return 1
}
shot_card() {
  pillwin | python3 -I -c 'import json,sys
d=json.loads(sys.stdin.readline())
w=[w for w in d["windows"] if w["h"]>=30]
w=w[0] if w else None
print("%d,%d,%d,%d"%(w["x"]-24,w["y"]-24,w["w"]+48,w["h"]+48) if w else "")' >"$T/rect"
  [ -s "$T/rect" ] && /usr/sbin/screencapture -x -R "$(cat "$T/rect")" "$T/card.png"
}
record() {
  kill -USR2 "$pid"
  sleep 0.8
  wb_mic_play "$WAV"
  sleep 0.6
}
# The key-up: only if a window of ours still has focus (another tool may
# have brought its own app to the front meanwhile); otherwise the take is
# cancelled, so nothing is pasted into a window this run did not open.
stop_take() {
  local now
  now=$(front)
  if [ "$now" = "$1" ]; then
    kill -USR2 "$pid"
    return 0
  fi
  wb_cancel "$APP_DIR" >/dev/null 2>&1
  echo "focus taken by $now before the stop (expected $1); take cancelled" >"$T/skip"
  return 1
}
open_textedit() {
  doc=$(osascript -e 'tell application "TextEdit"' -e 'activate' -e 'set d to make new document' \
    -e 'return name of d' -e 'end tell')
}
open_safari() {
  osascript -e 'tell application "Safari"' \
    -e "make new document with properties {URL:\"$PAGE#$1\"}" -e 'activate' -e 'end tell' >/dev/null
}
open_chrome() {
  open -na "Google Chrome" --args --user-data-dir="$PROFILE" --no-first-run \
    --no-default-browser-check --new-window "$PAGE#$1"
}
safari_title() {
  osascript -e "tell application \"Safari\" to get name of (first document whose URL contains \"$PORT/fields.html\")"
}
chrome_title() {
  # Our own Chrome instance (the owner's may be running too).
  local cpid
  cpid=$(pgrep -f "Google Chrome --user-data-dir=$PROFILE" | head -1)
  [ -n "$cpid" ] && osascript -e "tell application \"System Events\" to get name of windows of (first process whose unix id is $cpid)"
}
close_all() {
  [ -n "${doc:-}" ] && osascript -e "tell application \"TextEdit\" to close document \"$doc\" saving no" 2>/dev/null
  osascript -e "tell application \"Safari\" to close (every document whose URL contains \"$PORT/fields.html\")" 2>/dev/null
  pkill -f "user-data-dir=$PROFILE" 2>/dev/null
  osascript -e 'tell application "Finder" to close Finder window "wb-s05-finder"' 2>/dev/null
  doc=""
}
clip_save() { "$BIN/clip" save "$T/clip-saved" >"$T/clip-save.json"; }
clip_restore() {
  "$BIN/clip" restore "$T/clip-saved" >"$T/clip-restore.json"
  python3 -I -c 'import json,sys
a=json.load(open(sys.argv[1])); b=json.load(open(sys.argv[2]))
print("identical" if a["digest"]==b["digest"] and a["saved"]==b["restored"] else "DIFFERENT")' \
    "$T/clip-save.json" "$T/clip-restore.json" >"$T/clip-restored"
}
last_text() {
  sqlite3 "$APP_DIR/Data/history.db" "SELECT transcription_text FROM transcription_history ORDER BY id DESC LIMIT 1" >"$T/history-text.txt"
}
finish() {
  # The app log of this take: target decision and outcome (no text).
  tail -n +"$((log_from + 1))" "$LOG" | grep -E "insert: take [0-9]+ |pill: card copied" >"$T/app.log"
  python3 -I "$HERE/summarize.py" "$CASE" "$T"
}

doc=""
expect=""
case "$CASE" in
  field:textedit) open_textedit; expect=com.apple.TextEdit ;;
  field:safari:*) open_safari "${CASE##*:}"; expect=com.apple.Safari ;;
  field:chrome:*) open_chrome "${CASE##*:}"; expect=com.google.Chrome ;;
  card:password) open_safari password; expect=com.apple.Safari ;;
  card:finder)
    mkdir -p "${TMPDIR:-/tmp}/wb-s05-finder"
    osascript -e 'tell application "Finder"' -e 'activate' \
      -e "make new Finder window to (POSIX file \"${TMPDIR:-/tmp}/wb-s05-finder\" as alias)" \
      -e 'end tell' >/dev/null
    expect=com.apple.finder
    ;;
  card:desktop) expect=com.apple.finder ;;
  switch) open_textedit; sleep 1; open_chrome input; expect=com.google.Chrome ;;
  moved:*) open_safari input; sleep 2; open_textedit; expect=com.apple.TextEdit ;;
  *) echo "unknown case $CASE" >&2; exit 2 ;;
esac
sleep 3
[ "$CASE" = card:desktop ] && "$BIN/drive" desktop >"$T/desktop.json"
f=$(front)
if [ "$f" != "$expect" ]; then
  echo "front=$f expected=$expect" >"$T/skip"
  close_all
  finish
  exit 0
fi

case "$CASE" in
  card:*)
    clip_save
    record
    if stop_take "$expect" && wait_card; then
      echo yes >"$T/card-shown"
      shot_card
      if [ "$CASE" = card:desktop ]; then
        # ✕ (a click on our own bar). Esc is observed by the hotkey
        # engine's tap (a test identity runs it since S-07a; the S-05 run
        # predates that, so Esc was covered by the unit tests only).
        WB_PID=$pid "$BIN/pill" -e "clickat $CLOSE_X $CARD_MID" >/dev/null
        sleep 0.8
        echo "$(active_h)" >"$T/after-close-h"
      else
        last_text
        WB_PID=$pid "$BIN/pill" -e "clickat $COPY_X $CARD_MID" >/dev/null
        sleep 0.5
        "$BIN/clip" ours "$T/history-text.txt" >"$T/clip-ours.json"
        shot_card
        mv "$T/card.png" "$T/card-copied.png" 2>/dev/null
        sleep 2
        echo "$(active_h)" >"$T/after-copy-h"
      fi
    fi
    sleep 1
    clip_restore
    ;;
  moved:*)
    kill -USR2 "$pid"
    sleep 0.8
    wb_mic_play "$WAV"
    sleep 0.6
    if [ "$(front)" != "$expect" ]; then
      wb_cancel "$APP_DIR" >/dev/null 2>&1
      echo "focus taken before the stop; take cancelled" >"$T/skip"
    else
      "$BIN/drive" stopswitch "$pid" "${CASE##*:}" com.apple.Safari >"$T/switch.json"
    fi
    wait_card && echo yes >"$T/card-shown" && shot_card
    sleep 1
    safari_title >"$T/safari.txt"
    osascript -e "tell application \"TextEdit\" to get text of document \"$doc\"" >"$T/textedit.txt"
    ;;
  switch)
    kill -USR2 "$pid"
    sleep 0.8
    wb_mic_play "$WAV"
    "$BIN/drive" activate com.apple.TextEdit >"$T/switch.json"
    sleep 0.6
    stop_take com.apple.TextEdit
    sleep 4
    osascript -e "tell application \"TextEdit\" to get text of document \"$doc\"" >"$T/textedit.txt"
    chrome_title >"$T/chrome.txt"
    ;;
  field:*)
    record
    stop_take "$expect"
    sleep 4
    [ "$(active_h)" = "$CARD_H" ] && echo yes >"$T/card-shown"
    case "$CASE" in
      field:textedit) osascript -e "tell application \"TextEdit\" to get text of document \"$doc\"" >"$T/result.txt" ;;
      field:safari:*) safari_title >"$T/result.txt" ;;
      field:chrome:*) chrome_title >"$T/result.txt" ;;
    esac
    ;;
esac
close_all
# Dismiss any card left over (✕ is not needed: the next take replaces it).
finish
exit 0
