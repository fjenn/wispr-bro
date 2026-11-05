#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# S-38 soak: starts the test app for soak.py. With the app closed, its
# profile gets every global shortcut unbound (takes go by
# `--toggle-transcription`; Mac wave 2 finding 7) and a cloned model cache
# when it has none (scripts/wb-lib/profile.py), then the app starts hidden
# with its test microphone, WISPRBRO_TEST_MIC_WAV=<out dir>/test-mic.wav
# (fork/audio/test_mic.rs: test identities and debug builds only, never
# com.wisprbro.desktop). soak.py copies each take's WAV into that file, so
# no loopback driver (BlackHole) and no speakers are needed.
#
# usage: scripts/wb-soak/start.sh [--hardware] [--dry-run] <app bundle> <out dir>
#   --hardware  S-38.9: the real microphone and fn (dictate) kept, no test mic
#   --dry-run   print what it would do; change nothing, start nothing
# Prints `pid <pid>` and `mic <path>` (soak.py --pid, --mic).
set -u
. "$(cd "$(dirname "$0")" && pwd)/../wb-lib/app.sh"
HARDWARE=0
DRY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --hardware) HARDWARE=1; shift ;;
    --dry-run) DRY=1; shift ;;
    *) break ;;
  esac
done
APP="${1:?usage: start.sh [--hardware] [--dry-run] <app bundle> <out dir>}"
OUT="${2:?out dir}"
PLIST="$APP/Contents/Info.plist"
[ -f "$PLIST" ] || { echo "not an app bundle: $APP" >&2; exit 2; }
ID=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$PLIST")
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$PLIST")"
case "$ID" in
  com.wisprbro.*check) ;;
  *) echo "refusing: $ID is not a test identity (com.wisprbro.<slice>check)" >&2; exit 1 ;;
esac
if [ -f "$APP/Contents/MacOS/portable" ]; then
  DATA="$APP/Contents/MacOS/Data"
else
  DATA="$HOME/Library/Application Support/$ID"
fi
mkdir -p "$OUT"
MIC="$(cd "$OUT" && pwd)/test-mic.wav"
KEEP=""
[ $HARDWARE = 1 ] && KEEP=dictate

if [ $DRY = 1 ]; then
  echo "dry run: $ID ($EXE)"
  echo "dry run: unbind every shortcut in $DATA${KEEP:+ except $KEEP}"
  [ -d "$DATA/huggingface" ] || echo "dry run: clone the model cache into $DATA/huggingface"
  if [ $HARDWARE = 1 ]; then echo "dry run: start on the real microphone"; else echo "dry run: start with WISPRBRO_TEST_MIC_WAV=$MIC"; fi
  exit 0
fi

if pgrep -f "$EXE" >/dev/null; then
  echo "already running: quit $ID first (the profile is changed with the app closed)" >&2
  exit 1
fi
wb_unbind_keys "$DATA" "$KEEP" || exit 1
[ -d "$DATA/huggingface" ] || wb_clone_models "$DATA" "${WB_MODEL_CACHE:-}" || exit 1
if [ $HARDWARE = 1 ]; then
  nohup "$EXE" --start-hidden >"$OUT/app-stdout.log" 2>&1 &
else
  WISPRBRO_TEST_MIC_WAV="$MIC" nohup "$EXE" --start-hidden >"$OUT/app-stdout.log" 2>&1 &
fi
pid=""
for _ in $(seq 1 60); do
  pid=$(pgrep -f "$EXE --start-hidden" | head -1)
  [ -n "$pid" ] && break
  sleep 0.5
done
[ -n "$pid" ] || { echo "did not start" >&2; exit 1; }
echo "pid $pid"
[ $HARDWARE = 1 ] || echo "mic $MIC"
