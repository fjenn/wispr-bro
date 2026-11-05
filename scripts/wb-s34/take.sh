#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-34 (docs/fork/MAC-TEST-SCRIPT.md S-34): one take through the
# running test app's real record path (SIGUSR2 → mic → speech model → text
# pipeline → paste) into scripts/wb-s34/composer.html in a separate Chrome
# instance (never the owner's profile), then a correction typed the way a
# person fixes a word, then Return (the composer sends and empties, as
# Slack's does). Speech is a pre-recorded WAV ("Deploy it to zebra
# today.") that the app plays as its test microphone (started by
# scripts/wb-s05/app.sh with WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav; never
# the speakers unless WB_ALLOW_SPEAKERS=1); the profile's
# test rule turns "zebra" into "Kubernetees" (setup.sh).
# Built on scripts/wb-s20/take.sh: holds ~/wb-bench/gui.lock, records
# only while Chrome is in front, saves and checks the whole clipboard, and
# closes what it opened.
#
# usage: scripts/wb-s34/take.sh <case> <wav> <out dir>
#   fix       select the composer (⌘A), retype its text with "Kubernetees"
#             fixed to "Kubernetes", Return: a suggestion "Kubernetes"
#   tab       the same fix, committed by Tab instead of Return
#   wait      the same fix, no commit: nothing within 30 s (no_commit)
#   card      the "Card number" field: never watched (sensitive)
# Env: WB_S34_BIN = folder with the compiled helpers
#   (swiftc -O -o $WB_S34_BIN/clip scripts/wb-s05/clip.swift).
set -u
CASE="$1"; WAV="$2"; OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
. "$HERE/../wb-lib/app.sh"
wb_test_mic_env "$APP_DIR/Data" || exit 1
BIN="${WB_S34_BIN:?set WB_S34_BIN}"
LOCK="$HOME/wb-bench/gui.lock"
PROFILE="${TMPDIR:-/tmp}/wb-s34-chrome-profile"
PAGE="file://$HERE/composer.html"
LOG="$APP_DIR/Data/logs/wispr-bro.log"
DB="$APP_DIR/Data/fork.db"

if [ "${WB_S34_LOCKED:-}" != 1 ]; then
  WB_S34_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/take.sh" "$@"
fi

mkdir -p "$OUT"
T="$OUT/$(date +%H%M%S)-$CASE"
mkdir -p "$T"
pid=$(/bin/bash "$HERE/../wb-s05/app.sh" pid) || { echo "NO APP" | tee "$T/result"; exit 3; }
log_from=$(wc -l <"$LOG")
front() { lsappinfo info -only bundleid "$(lsappinfo front)" | sed -E 's/.*="?([^"]*)"?/\1/'; }
front_pid() { lsappinfo info -only pid "$(lsappinfo front)" | sed -E 's/[^0-9]//g'; }
# The owner's own Chrome has the same bundle id: only the instance this
# take opened (its pid) counts as "Chrome in front".
ours() { [ -n "$(chrome_pid)" ] && [ "$(front_pid)" = "$(chrome_pid)" ]; }
keys() {
  if ! ours; then echo "SKIP keys ($1): front=$(front) pid $(front_pid)" >&2; return 1; fi
  osascript -e "tell application \"System Events\" to $1"
}
"$BIN/clip" save "$T/clip.saved" >/dev/null
clip_before=$("$BIN/clip" digest)

hash=""; [ "$CASE" = card ] && hash="#card"
open -na "Google Chrome" --args --user-data-dir="$PROFILE" --no-first-run \
  --no-default-browser-check --force-renderer-accessibility --new-window "$PAGE$hash"
sleep 3
chrome_pid() { pgrep -f "Google Chrome --user-data-dir=$PROFILE" | head -1; }
title() {
  osascript -e "tell application \"System Events\" to get name of front window of (first process whose unix id is $(chrome_pid))" 2>/dev/null
}

if ! ours; then
  echo "SKIP front=$(front) pid $(front_pid)" | tee "$T/result"
else
  wb_mic_load "$WAV" || exit 1
  kill -USR2 "$pid"; sleep 0.8; wb_mic_play "$WAV"; sleep 0.6
  if ours; then kill -USR2 "$pid"; else
    wb_cancel "$APP_DIR" >/dev/null 2>&1; echo "focus moved; cancelled" >"$T/skip"; fi
  sleep 8
  pasted=$(title | sed 's/^S34://')
  echo "$pasted" >"$T/pasted"
  case "$CASE" in
    fix|tab|wait)
      fixed=$(printf '%s' "$pasted" | sed 's/Kubernetees/Kubernetes/g')
      sleep 1
      keys 'keystroke "a" using command down'
      # Typed, not pasted: the clipboard stays the owner's.
      keys "keystroke \"$fixed\""
      sleep 1.5
      case "$CASE" in
        fix) keys 'key code 36' ;;   # Return
        tab) keys 'key code 48' ;;   # Tab
        wait) sleep 31 ;;
      esac
      ;;
    card) sleep 2; keys 'key code 36' ;;
  esac
  sleep 2
  title | sed -E 's/^S34( sent)?://' >"$T/text"
  echo "OK" >"$T/result"
fi
kill "$(chrome_pid)" 2>/dev/null
sleep 1
clip_after=$("$BIN/clip" digest)
if [ "$clip_before" = "$clip_after" ]; then echo same >"$T/clip"; else
  "$BIN/clip" restore "$T/clip.saved" >/dev/null; echo "changed (restored)" >"$T/clip"; fi
rm -rf "$T/clip.saved" # the owner's clipboard, never kept
tail -n +"$((log_from + 1))" "$LOG" | /usr/bin/grep -E "learn:|insert:" >"$T/log"
sqlite3 "$DB" "SELECT kind, term, from_text, to_text, status FROM dictionary_entries WHERE source = 'learn'" >"$T/learned"
sqlite3 "$DB" "SELECT from_text, to_text FROM corrections WHERE source = 'learn'" >"$T/corrections"
# Exit criterion: the field's other text never (History keeps the
# dictation itself, as for every take) reaches the database files or the log.
leaks=0
for f in "$DB" "$DB-wal" "$LOG"; do
  [ -f "$f" ] || continue
  for needle in "door code" "4471"; do
    if /usr/bin/grep -a -q "$needle" "$f"; then echo "LEAK $needle in $(basename "$f")" >>"$T/leaks"; leaks=1; fi
  done
done
[ "$leaks" = 0 ] && echo none >"$T/leaks"
echo "$CASE $(cat "$T/result") | clip $(cat "$T/clip") | leaks $(head -1 "$T/leaks") | learned: $(tr '\n' ' ' <"$T/learned") | $(grep -o 'learn: [^"]*' "$T/log" | tail -1)"
