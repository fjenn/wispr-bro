#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-36 (docs/fork/MAC-TEST-SCRIPT.md S-36): one take through the
# running test app's real record path (SIGUSR2 → context read at record
# start → mic → speech model → text pipeline → cleanup → paste) into a
# Cursor window this script opens on a throwaway workspace, in a separate
# Cursor profile (never the owner's) with screen-reader mode on
# ("editor.accessibilitySupport": "on"). Speech is a pre-recorded WAV
# that the app plays as its test microphone (started by scripts/wb-s05/app.sh
# with WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav; never the speakers unless
# WB_ALLOW_SPEAKERS=1). Built on
# scripts/wb-s20/take.sh: holds ~/wb-bench/gui.lock, records only while
# the expected app is in front, saves and checks the whole clipboard, and
# closes what it opened.
#
# usage: scripts/wb-s36/take.sh <case> <wav> <out dir>
#   editor    user_service.ts open, caret on an empty last line; the text
#             lands in the buffer (read back by saving the file)
#   chat      index.ts and user_service.ts open in tabs; Cursor's chat
#             input focused (⌘L); the chat text is read back by ⌘A ⌘C
#   textedit  a TextEdit document that names index.ts (not a listed
#             editor: nothing may be tagged or respelled)
# Env: WB_S36_BIN = folder with the compiled helpers
#   (swiftc -O -o $WB_S36_BIN/clip scripts/wb-s05/clip.swift).
set -u
CASE="$1"; WAV="$2"; OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
. "$HERE/../wb-lib/app.sh"
wb_test_mic_env "$APP_DIR/Data" || exit 1
BIN="${WB_S36_BIN:?set WB_S36_BIN}"
LOCK="$HOME/wb-bench/gui.lock"
PROFILE="${TMPDIR:-/tmp}/wb-s36-cursor-profile"
WS="${TMPDIR:-/tmp}/wb-s36-workspace"
LOG="$APP_DIR/Data/logs/wispr-bro.log"
CURSOR=com.todesktop.230313mzl4w4u92

if [ "${WB_S36_LOCKED:-}" != 1 ]; then
  WB_S36_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/take.sh" "$@"
fi

mkdir -p "$OUT"
T="$OUT/$(date +%H%M%S)-$CASE"
mkdir -p "$T"
pid=$(/bin/bash "$HERE/../wb-s05/app.sh" pid) || { echo "NO APP" | tee "$T/result"; exit 3; }
log_from=$(wc -l <"$LOG")
front() { lsappinfo info -only bundleid "$(lsappinfo front)" | sed -E 's/.*="?([^"]*)"?/\1/'; }
# Keystrokes go to the frontmost app: only ever into the window this take
# opened (the user may have switched apps during the wait).
front_pid() { lsappinfo info -only pid "$(lsappinfo front)" | sed -E 's/[^0-9]//g'; }
# The owner's own Cursor has the same bundle id: for Cursor only the
# instance this take opened (its main pid) counts as in front.
cursor_pid() {
  local p
  for p in $(pgrep -x Cursor); do
    ps -o command= -p "$p" | /usr/bin/grep -qF -- "--user-data-dir=$PROFILE" && { echo "$p"; return 0; }
  done
  return 1
}
in_front() {
  [ "$(front)" = "$expect" ] || return 1
  [ "$opened" != cursor ] && return 0
  [ "$(front_pid)" = "$(cursor_pid)" ]
}
keys() {
  if ! in_front; then echo "SKIP keys ($1): front=$(front) pid $(front_pid)" >&2; return 1; fi
  osascript -e "tell application \"System Events\" to $1"
}
"$BIN/clip" save "$T/clip.saved" >/dev/null
clip_before=$("$BIN/clip" digest)

# A fresh workspace and Cursor profile for every take. The profile must be
# removed, not only rewritten: Cursor restores the last session's windows
# and tabs from it, and Mac wave 1's editor take then went into a restored
# index.ts (34 chars, 1 code name, `identifiers=0`) instead of
# user_service.ts. WB_S36_PROFILE keeps a prepared profile (signed out but
# past the welcome / Log In screens) when set; it is never the owner's.
if [ -n "${WB_S36_PROFILE:-}" ]; then
  rm -rf "$PROFILE"; cp -R "$WB_S36_PROFILE" "$PROFILE"
  rm -rf "$PROFILE/User/workspaceStorage" "$PROFILE/Backups"
else
  rm -rf "$PROFILE"
fi
rm -rf "$WS"; mkdir -p "$WS" "$PROFILE/User"
cat >"$WS/user_service.ts" <<'TS'
export interface User {
  user_id: string;
  displayName: string;
}

export function getUserById(user_id: string): User | undefined {
  return undefined;
}

TS
printf 'export * from "./user_service";\n' >"$WS/index.ts"
cat >"$PROFILE/User/settings.json" <<'JSON'
{
  "editor.accessibilitySupport": "on",
  "window.restoreWindows": "none",
  "security.workspace.trust.enabled": false,
  "workbench.startupEditor": "none",
  "files.autoSave": "off",
  "update.mode": "none",
  "telemetry.telemetryLevel": "off"
}
JSON

opened=""; expect=""
case "$CASE" in
  editor|chat)
    extra=""; [ "$CASE" = chat ] && extra="$WS/index.ts"
    open -na Cursor --args --user-data-dir="$PROFILE" --extensions-dir="$PROFILE/ext" \
      --new-window "$WS" $extra --goto "$WS/user_service.ts:9:1"
    opened=cursor; expect=$CURSOR
    sleep 8
    [ "$CASE" = chat ] && { keys 'keystroke "l" using command down'; sleep 2; }
    ;;
  textedit)
    doc=$(osascript -e 'tell application "TextEdit"' -e 'activate' \
      -e 'set d to make new document' \
      -e 'set text of d to "Files: index.ts and user_service.ts" & return & return' \
      -e 'return name of d' -e 'end tell')
    opened=textedit; expect=com.apple.TextEdit
    # ⌘↓, not End: a synthetic End (key code 119) carries the fn flag and
    # leaves it set in the session's modifier state, so the app then waits
    # for "held modifiers" and shows a card instead of pasting.
    keys 'key code 125 using command down'
    sleep 2
    ;;
  *) echo "unknown case $CASE" >&2; exit 2 ;;
esac

f=$(front)
if ! in_front; then
  echo "SKIP front=$f pid $(front_pid)" | tee "$T/result"
else
  wb_mic_load "$WAV" || exit 1
  kill -USR2 "$pid"
  sleep 0.8
  wb_mic_play "$WAV"
  sleep 0.6
  if in_front; then
    kill -USR2 "$pid"
  else
    wb_cancel "$APP_DIR" >/dev/null 2>&1
    echo "focus moved; cancelled" >"$T/skip"
  fi
  sleep 9
  # Evidence: the window the take went into, before it is read back.
  if [ -x "$BIN/wbwin" ]; then
    for wp in $(pgrep -f "user-data-dir=$PROFILE") $(pgrep -x TextEdit); do
      w=$("$BIN/wbwin" "$wp" 2>/dev/null | cut -d' ' -f1)
      [ -n "$w" ] && { screencapture -x -o -l "$w" "$T/window.png"; break; }
    done
  fi
  case "$CASE" in
    editor) keys 'keystroke "s" using command down'; sleep 1; tail -n 2 "$WS/user_service.ts" >"$T/text" ;;
    chat) keys 'keystroke "a" using command down'; keys 'keystroke "c" using command down'; sleep 0.5
      pbpaste >"$T/text" ;;
    textedit) osascript -e "tell application \"TextEdit\" to get text of document \"$doc\"" >"$T/text" ;;
  esac
  echo "OK" >"$T/result"
fi
case "$opened" in
  cursor) pkill -f "Cursor.*--user-data-dir=$PROFILE" 2>/dev/null; sleep 1
    pkill -f "user-data-dir=$PROFILE" 2>/dev/null ;;
  textedit) osascript -e "tell application \"TextEdit\" to close document \"$doc\" saving no" ;;
esac
sleep 1
clip_after=$("$BIN/clip" digest)
if [ "$clip_before" = "$clip_after" ]; then echo same >"$T/clip"; else
  "$BIN/clip" restore "$T/clip.saved" >/dev/null; echo "changed (restored)" >"$T/clip"; fi
rm -rf "$T/clip.saved" # the owner's clipboard, never kept
tail -n +"$((log_from + 1))" "$LOG" | /usr/bin/grep -E "context:|cleanup:|text:|insert:" >"$T/log"
echo "$CASE $(cat "$T/result") | clip $(cat "$T/clip") | $(tr '\n' ' ' <"$T/text" 2>/dev/null)"
