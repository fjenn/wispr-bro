#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-06 (docs/fork/MAC-TEST-SCRIPT.md S-06): one recovery check
# against the running portable test app (scripts/wb-s05/app.sh), in windows
# this script opens. Holds ~/wb-bench/gui.lock for the whole case, checks
# the frontmost app before every synthetic event (`post` refuses
# otherwise), closes only what it opened, and saves and restores the whole
# clipboard around every case (byte-identical, checked).
#
# usage: scripts/wb-s06/case.sh <case> <out dir> [wav]
#   paste:textedit        ⌃⌘V in a new TextEdit document → the last dictation
#   paste:held:<ms>       ⌃⌘V with ⌃⌘ still held <ms> after V (RS-05 case):
#                         pasted after the release (< 3 s) or the card (≥ 3 s)
#   paste:chrome          ⌃⌘V into fields.html#textarea in our own Chrome
#   paste:terminal        ⌃⌘V into our own Terminal window (`cat`, no shell)
#   copy:textedit         ⌃⌘C → the clipboard holds the last dictation
#   enter:mouse           the mouse's back button over our TextEdit document
#                         → a Return in the document (Press Enter)
#   undo:bar <wav>        a take (SIGUSR2 + the WAV through the speakers),
#                         Esc → "Cancelled · Undo", Undo clicked on the bar →
#                         the text in the document and in History
#   undo:moved <wav>      as undo:bar, but another document gets focus
#                         before Undo (the ⌃⌥Z hotkey) → the card
#   undo:expire <wav>     as undo:bar, nobody undoes: after 4 s the offer
#                         goes, ⌃⌥Z does nothing, no History row
# Env: WB_S06_BIN = folder with the compiled helpers (post: scripts/wb-s06/
#   post.swift; clip: scripts/wb-s05/clip.swift; pill: scripts/wb-s03-pill.swift).
set -u
CASE="$1"; OUT="$2"; WAV="${3:-}"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
. "$HERE/../wb-lib/app.sh"
BIN="${WB_S06_BIN:?set WB_S06_BIN}"
LOCK="$HOME/wb-bench/gui.lock"
LOG="$APP_DIR/Data/logs/wispr-bro.log"
PORT=47816
PROFILE="${TMPDIR:-/tmp}/wb-s06-chrome-profile"
PAGE="http://127.0.0.1:$PORT/fields.html"
LAST_TEXT="Recovery check: the quick brown fox jumps over the lazy dog."
# The message capsule (360 x 34 pt) and the card (400 x 58 pt), as the
# active bar window's size; Undo's centre from the capsule's left edge
# (measured on the pill page in Chrome with the English text).
MSG_H=360x34
MSG_MID=17
UNDO_X=${WB_S06_UNDO_X:-296}
CARD_H=400x58

if [ "${WB_S06_LOCKED:-}" != 1 ]; then
  WB_S06_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/case.sh" "$@"
fi

mkdir -p "$OUT"
n=$(($(ls "$OUT" | grep -c '^case-') + 1))
T="$OUT/case-$(printf %03d $n)-${CASE//:/_}"
mkdir -p "$T"
pid=$(/bin/bash "$HERE/../wb-s05/app.sh" pid) || { echo "{\"case\":\"$CASE\",\"result\":\"NO APP\"}"; exit 3; }
log_from=$(wc -l <"$LOG")

post() { WB_FRONT="$EXPECT" "$BIN/post" "$@"; }
front() { "$BIN/post" front | /usr/bin/python3 -I -c 'import json,sys; print(json.load(sys.stdin)["front"])'; }
pillwin() { WB_PID=$pid "$BIN/pill" -e windows; }
active_h() {
  pillwin | /usr/bin/python3 -I -c 'import json,sys
d=json.loads(sys.stdin.readline())
w=[w for w in d["windows"] if w["h"]>=30]
print("%dx%d"%(w[0]["w"],w[0]["h"]) if w else "0")'
}
wait_h() { # <height> [tries]
  for _ in $(seq 1 "${2:-40}"); do
    [ "$(active_h)" = "$1" ] && return 0
    sleep 0.1
  done
  return 1
}
shot_bar() {
  pillwin | /usr/bin/python3 -I -c 'import json,sys
d=json.loads(sys.stdin.readline())
w=[w for w in d["windows"] if w["h"]>=30]
w=w[0] if w else None
print("%d,%d,%d,%d"%(w["x"]-24,w["y"]-24,w["w"]+48,w["h"]+48) if w else "")' >"$T/rect"
  /usr/bin/grep -q , "$T/rect" && /usr/sbin/screencapture -x -R "$(cat "$T/rect")" "$T/$1.png"
}
clip_save() { "$BIN/clip" save "$T/clip-saved" >"$T/clip-save.json"; }
clip_restore() {
  "$BIN/clip" restore "$T/clip-saved" >"$T/clip-restore.json"
  /usr/bin/python3 -I -c 'import json,sys
a=json.load(open(sys.argv[1])); b=json.load(open(sys.argv[2]))
print("identical" if a["digest"]==b["digest"] and a["saved"]==b["restored"] else "DIFFERENT")' \
    "$T/clip-save.json" "$T/clip-restore.json" >"$T/clip-restored"
}
open_textedit() {
  doc=$(osascript -e 'tell application "TextEdit"' -e 'activate' -e 'set d to make new document' \
    -e 'return name of d' -e 'end tell')
  sleep 1.5
}
doc_text() {
  osascript -e "tell application \"TextEdit\" to get text of document \"$1\"" 2>/dev/null
}
open_chrome() {
  (cd "$HERE/../wb-s05" && /usr/bin/python3 -I -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1 &)
  sleep 0.5
  open -na "Google Chrome" --args --user-data-dir="$PROFILE" --no-first-run \
    --no-default-browser-check --new-window "$PAGE#$1"
  sleep 4
}
open_terminal() {
  # A window of our own running `cat` into a file (no shell reads it).
  : >"$T/terminal.txt"
  term_id=$(osascript -e 'tell application "Terminal"' -e 'activate' \
    -e "set t to do script \"exec cat > '$T/terminal.txt'\"" \
    -e 'return id of (first window whose tabs contains t)' -e 'end tell')
  sleep 2
}
record_and_cancel() { # a take through the speakers, then Esc in our window
  kill -USR2 "$pid"
  sleep 0.8
  afplay "$WAV"
  sleep 0.4
  if [ "$(front)" != "$EXPECT" ]; then
    wb_cancel "$APP_DIR" >/dev/null 2>&1
    echo "focus taken before the cancel" >"$T/skip"
    return 1
  fi
  post key escape >"$T/esc.json"
}
close_all() {
  [ -n "${doc:-}" ] && osascript -e "tell application \"TextEdit\" to close document \"$doc\" saving no" 2>/dev/null
  [ -n "${doc2:-}" ] && osascript -e "tell application \"TextEdit\" to close document \"$doc2\" saving no" 2>/dev/null
  pkill -f "user-data-dir=$PROFILE" 2>/dev/null
  pkill -f "http.server $PORT --bind 127.0.0.1" 2>/dev/null
  if [ -n "${term_id:-}" ]; then
    pkill -f "cat > $T/terminal.txt" 2>/dev/null
    sleep 0.3
    osascript -e "tell application \"Terminal\" to close window id $term_id" 2>/dev/null
  fi
  doc=""; doc2=""; term_id=""
}
finish() {
  close_all
  tail -n +"$((log_from + 1))" "$LOG" | grep -E "recovery: |insert: take [0-9]+ |pill: " >"$T/app.log"
  /usr/bin/python3 -I "$HERE/summarize.py" "$CASE" "$T" "$LAST_TEXT"
}

doc=""; doc2=""; term_id=""
EXPECT=com.apple.TextEdit
case "$CASE" in
  paste:chrome) open_chrome textarea; EXPECT=com.google.Chrome ;;
  paste:terminal) open_terminal; EXPECT=com.apple.Terminal ;;
  *) open_textedit ;;
esac
f=$(front)
if [ "$f" != "$EXPECT" ]; then
  echo "front=$f expected=$EXPECT" >"$T/skip"
  finish
  exit 0
fi
clip_save

case "$CASE" in
  paste:textedit)
    post chord ctrl,command v 0 >"$T/post.json"
    sleep 2
    doc_text "$doc" >"$T/field.txt"
    ;;
  paste:held:*)
    hold="${CASE##*:}"
    post chord ctrl,command v "$hold" >"$T/post.json"
    sleep 2.5
    doc_text "$doc" >"$T/field.txt"
    if wait_h "$CARD_H" 20; then echo yes >"$T/card-shown"; shot_bar card; fi
    ;;
  paste:chrome)
    post chord ctrl,command v 0 >"$T/post.json"
    sleep 2.5
    # fields.html puts what each field holds in the title (our instance).
    cpid=$(pgrep -f "Google Chrome --user-data-dir=$PROFILE" | head -1)
    [ -n "$cpid" ] && osascript -e "tell application \"System Events\" to get name of windows of (first process whose unix id is $cpid)" >"$T/field.txt"
    ;;
  paste:terminal)
    post chord ctrl,command v 0 >"$T/post.json"
    sleep 1
    # `cat` writes a line once Return arrives; the pasted text has none, so
    # end the line ourselves in our own window.
    post key return >/dev/null
    sleep 1
    cp "$T/terminal.txt" "$T/field.txt"
    ;;
  copy:textedit)
    post chord ctrl,command c 0 >"$T/post.json"
    sleep 1
    printf '%s' "$LAST_TEXT" >"$T/last-text.txt"
    "$BIN/clip" ours "$T/last-text.txt" >"$T/clip-ours.json" 2>/dev/null
    ;;
  enter:mouse)
    osascript -e "tell application \"TextEdit\" to set text of document \"$doc\" to \"line one\"" >/dev/null
    sleep 0.5
    # The pointer over our document: its window's centre.
    pt=$(osascript -e 'tell application "System Events" to tell process "TextEdit"' \
      -e 'set {x, y} to position of front window' -e 'set {w, h} to size of front window' \
      -e 'return ((x + w / 2) as integer) & " " & ((y + h / 2) as integer)' -e 'end tell' | tr -d ',')
    post mouse 3 $pt >"$T/post.json"
    sleep 1
    doc_text "$doc" >"$T/field.txt"
    ;;
  undo:bar)
    record_and_cancel || { finish; exit 0; }
    if wait_h "$MSG_H" 40; then
      echo yes >"$T/offer-shown"
      shot_bar offer
      WB_PID=$pid "$BIN/pill" -e "clickat $UNDO_X $MSG_MID" >"$T/click.json"
      sleep 4
      doc_text "$doc" >"$T/field.txt"
      shot_bar after
    fi
    ;;
  undo:moved)
    record_and_cancel || { finish; exit 0; }
    if wait_h "$MSG_H" 40; then
      echo yes >"$T/offer-shown"
      # Another document of ours takes focus, then ⌃⌥Z.
      doc2=$(osascript -e 'tell application "TextEdit" to set d to make new document' \
        -e 'tell application "TextEdit" to return name of d')
      sleep 0.8
      post chord ctrl,option z 0 >"$T/post.json"
      if wait_h "$CARD_H" 60; then echo yes >"$T/card-shown"; shot_bar card; fi
      doc_text "$doc" >"$T/field.txt"
      doc_text "$doc2" >"$T/field2.txt"
      # ✕ on the card (its right end).
      WB_PID=$pid "$BIN/pill" -e "clickat 379 29" >/dev/null
      sleep 0.6
    fi
    ;;
  undo:expire)
    record_and_cancel || { finish; exit 0; }
    if wait_h "$MSG_H" 40; then
      echo yes >"$T/offer-shown"
      sleep 4.5
      echo "$(active_h)" >"$T/after-h"
      post chord ctrl,option z 0 >"$T/post.json"
      sleep 1.5
      doc_text "$doc" >"$T/field.txt"
    fi
    ;;
  *) echo "unknown case $CASE" >&2; finish; exit 2 ;;
esac
clip_restore
finish
