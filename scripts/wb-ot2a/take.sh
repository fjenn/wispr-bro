#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test OT2-A (docs/fork/MAC-TEST-SCRIPT.md OT2-A): one held take that
# changes focus before the release, through the running test app's hotkey
# engine (a synthetic trigger hold, scripts/wb-s01-keys.swift) and its real
# record path (the test microphone plays a WAV, never the speakers:
# scripts/wb-lib/app.sh wb_mic_load). The app is the S-05 test build
# (scripts/wb-s05/app.sh start with the trigger bound, see run.sh).
#
# Holds ~/wb-bench/gui.lock for the whole take, opens and closes its own
# windows only, and saves and restores the whole clipboard around every
# take (byte-identical, checked).
#
# usage: scripts/wb-ot2a/take.sh <case> <wav> <out dir>
#   hold:cmdtab     our Finder window in front; trigger held; 1 s in,
#                   ⌘-Tab (with the trigger still held) and TextEdit
#                   activated; released after the speech → the text in the
#                   TextEdit document, the take never cancelled
#   hold:click      two TextEdit documents (two instances of our own), A
#                   in front; trigger held; 1 s
#                   in, a click (trigger flags held) into document B →
#                   the text in B, A empty
#   hold:finder     TextEdit in front; trigger held; 1 s in, a click into
#                   our Finder window → nothing pasted, the text on the
#                   clipboard (plain, not restored by the app) and in
#                   History, log "copied to the clipboard (no_text_field)"
#   hold:desktop    as hold:finder, the click on an empty desktop spot
#   free:cmdtab     hands-free (double press), ⌘-Tab + TextEdit, a press
#                   stops → the text in TextEdit (both modes decide alike)
# Env: WB_S05_BIN = compiled helpers: clip, drive (scripts/wb-s05/*.swift),
#   keys (swiftc -O -o $WB_S05_BIN/keys scripts/wb-s01-keys.swift), te
#   (swiftc -O -o $WB_S05_BIN/te scripts/wb-lib/te.swift). TextEdit is
#   always an instance of the take's own, by pid (never `tell application`).
#   WB_OT2A_TRIGGER = ctrl-opt (default; the owner's Wispr Bro listens on
#   fn) or fn (only when no other Wispr Bro runs; checked).
set -u
CASE="$1"; WAV="$2"; OUT="$3"
[ -n "${WB_OT2A_TRACE:-}" ] && set -x
HERE="$(cd "$(dirname "$0")" && pwd)"
S05="$HERE/../wb-s05"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
. "$HERE/../wb-lib/app.sh"
wb_test_mic_env "$APP_DIR/Data" || exit 1
BIN="${WB_S05_BIN:?set WB_S05_BIN}"
LOCK="$HOME/wb-bench/gui.lock"
LOG="$APP_DIR/Data/logs/wispr-bro.log"
TRIGGER="${WB_OT2A_TRIGGER:-ctrl-opt}"
FINDER_DIR="${TMPDIR:-/tmp}/wb-ot2a-finder"

if [ "${WB_OT2A_LOCKED:-}" != 1 ]; then
  WB_OT2A_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/take.sh" "$@"
fi

result() { echo "{\"case\":\"$CASE\",\"result\":\"$1\"}"; }
case "$TRIGGER" in
  ctrl-opt) DOWN="mods down ctrl,option"; UP="mods up ctrl,option" ;;
  fn)
    # fn would also reach the owner's app: only with no other instance.
    for p in $(pgrep -f "$WB_BIN_RE"); do
      lsof -a -p "$p" -d cwd -Fn 2>/dev/null | /usr/bin/grep -qx "n$APP_DIR" && continue
      result "SKIP: another Wispr Bro runs (pid $p); use WB_OT2A_TRIGGER=ctrl-opt"
      exit 0
    done
    DOWN="fn down"; UP="fn up"
    ;;
  *) echo "WB_OT2A_TRIGGER: ctrl-opt or fn" >&2; exit 2 ;;
esac

mkdir -p "$OUT"
wb_mic_load "$WAV" || { result "NO TEST MIC"; exit 4; }
n=$(($(ls "$OUT" | grep -c '^take-') + 1))
T="$OUT/take-$(printf %03d $n)-${CASE//:/_}"
mkdir -p "$T"
pid=$(/bin/bash "$S05/app.sh" pid) || { result "NO APP"; exit 3; }
log_from=$(wc -l <"$LOG")
speech_ms=$(python3 -I -c 'import sys; print(int(float(sys.argv[1])*1000))' "$(wb_wav_seconds "$WAV")")

front() { "$BIN/keys" -e front | python3 -I -c 'import json,sys; print(json.loads(sys.stdin.readline())["front"])'; }
front_pid() { "$BIN/te" front | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["pid"])'; }
# Each "document" is a TextEdit instance of this take's own (a file opened
# with `open -n`), addressed by its pid: `tell application "TextEdit"` would
# reach the owner's TextEdit and its documents if it runs.
new_doc() { # <name> → pid
  : >"$T/$1.txt"
  wb_textedit_open "$T/$1.txt"
}
doc_text() { "$BIN/te" text "$1" 2>/dev/null; }
# Centre of that instance's window (global, top-left).
doc_point() { "$BIN/te" point "$1"; }
finder_open() {
  mkdir -p "$FINDER_DIR"
  osascript -e 'tell application "Finder"' -e 'activate' \
    -e "set w to make new Finder window to (POSIX file \"$FINDER_DIR\" as alias)" \
    -e 'set bounds of w to {200, 200, 700, 600}' -e 'end tell' >/dev/null
}
close_all() {
  for d in ${docA:-} ${docB:-}; do wb_textedit_close "$d"; done
  osascript -e 'tell application "Finder" to close (every Finder window whose name is "wb-ot2a-finder")' 2>/dev/null
}

docA=""; docB=""
"$BIN/clip" save "$T/clip-saved" >"$T/clip-save.json"
case "$CASE" in
  hold:cmdtab | free:cmdtab) docA=$(new_doc a); sleep 1; finder_open; expect=com.apple.finder ;;
  hold:click) docB=$(new_doc b); sleep 1; docA=$(new_doc a); expect=com.apple.TextEdit ;;
  hold:finder | hold:desktop) finder_open; sleep 1; docA=$(new_doc a); expect=com.apple.TextEdit ;;
  *) echo "unknown case $CASE" >&2; exit 2 ;;
esac
sleep 2
front_ok() {
  [ "$(front)" = "$expect" ] || return 1
  [ "$expect" != com.apple.TextEdit ] || [ "$(front_pid)" = "$docA" ]
}
if [ -z "$docA" ] || ! front_ok; then
  echo "front=$(front) pid=$(front_pid) expected=$expect ${docA:+pid $docA}" >"$T/skip"
  close_all
  python3 -I "$HERE/summarize.py" "$CASE" "$T"
  exit 0
fi

# The switch, 1 s into the take, with the trigger still held.
pre=1000
post=$((speech_ms - pre + 700))
[ "$post" -lt 300 ] && post=300
case "$CASE" in
  *:cmdtab) SWITCH="mods down command; key tab; mods up command; activate $docA" ;;
  hold:click) SWITCH="click $(doc_point "$docB")" ;;
  hold:finder) SWITCH="click 450 400" ;;
  hold:desktop) SWITCH="" ;;
esac
if [ "$CASE" = hold:desktop ]; then
  # drive desktop clicks an uncovered spot and waits for Finder.
  "$BIN/keys" -e "$DOWN; wait $pre; flags" >"$T/keys.jsonl"
  "$BIN/drive" desktop >"$T/desktop.json"
  "$BIN/keys" -e "wait $post; flags; $UP" >>"$T/keys.jsonl"
elif [ "$CASE" = free:cmdtab ]; then
  # Double press locks; a later press stops.
  "$BIN/keys" -e "$DOWN; wait 80; $UP; wait 120; $DOWN; wait 60; $UP; wait $pre; $SWITCH; wait $post; $DOWN; wait 80; $UP" \
    >"$T/keys.jsonl"
else
  "$BIN/keys" -e "$DOWN; wait $pre; flags; $SWITCH; wait $post; flags; $UP" >"$T/keys.jsonl"
fi
echo "$(front)" >"$T/front-at-release"
sleep 6

[ -n "$docA" ] && doc_text "$docA" >"$T/docA.txt"
[ -n "$docB" ] && doc_text "$docB" >"$T/docB.txt"
sqlite3 "$APP_DIR/Data/history.db" \
  "SELECT transcription_text FROM transcription_history ORDER BY id DESC LIMIT 1" >"$T/history-text.txt"
"$BIN/clip" ours "$T/history-text.txt" >"$T/clip-ours.json"
"$BIN/clip" restore "$T/clip-saved" >"$T/clip-restore.json"
python3 -I -c 'import json,sys
a=json.load(open(sys.argv[1])); b=json.load(open(sys.argv[2]))
print("identical" if a["digest"]==b["digest"] and a["saved"]==b["restored"] else "DIFFERENT")' \
  "$T/clip-save.json" "$T/clip-restore.json" >"$T/clip-restored"
close_all
# The take's app log: hotkey effects (debug build), target and outcome.
tail -n +"$((log_from + 1))" "$LOG" \
  | grep -E "insert: take [0-9]+ |fork hotkeys: (Effect|StuckReleased)|copied to the clipboard" >"$T/app.log"
python3 -I "$HERE/summarize.py" "$CASE" "$T"
