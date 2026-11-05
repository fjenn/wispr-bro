#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-07b.4 (docs/fork/MAC-TEST-SCRIPT.md): bring the running test
# app's window to the front (a second launch hands over to the running
# instance), open the fork debug panel with ⌘⇧D only if that window is in
# front, scroll to the Speech models section and capture that window alone.
# Holds ~/wb-bench/gui.lock.
#
# Usage: scripts/wb-s07b/panel.sh <app dir> <app pid> <window-id binary> <out png>
set -u
. "$(cd "$(dirname "$0")" && pwd)/../wb-lib/app.sh"
APP="$1"; PID="$2"; WINID="$3"; OUT="$4"
if [ "${S07B_LOCKED:-}" != 1 ]; then
  S07B_LOCKED=1 exec /usr/bin/lockf -k -t 600 "$HOME/wb-bench/gui.lock" "$0" "$@"
fi
( cd "$APP" && "./$(wb_bin_name "$APP")" > /dev/null 2>&1 ) &
sleep 3
# Bring our own process forward (an accessory app is not activated by the
# hand-over).
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $PID) to true" > /dev/null
sleep 1
FRONT_PID=$(osascript -e 'tell application "System Events" to get unix id of first process whose frontmost is true')
if [ "$FRONT_PID" != "$PID" ]; then
  echo "front pid is $FRONT_PID, not $PID: no key events"
  exit 1
fi
osascript -e 'tell application "System Events" to keystroke "d" using {command down, shift down}'
sleep 2
read -r ID _ <<< "$("$WINID" "$PID")"
screencapture -o -x -l "$ID" "$OUT"
echo "captured window $ID to $OUT"
