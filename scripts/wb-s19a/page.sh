#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-19a.1/.4: drives the running test app's own Hub window (never
# another app's): shows it (second launch), runs ui.swift commands in it and
# takes a window screenshot. Holds ~/wb-bench/gui.lock for the whole run;
# ui.swift posts events only while the test app is frontmost.
#
# usage: scripts/wb-s19a/page.sh <ui binary> <out.png> ['<ui.swift commands>']
#   <ui binary>  scripts/wb-s16/ui.swift compiled with swiftc (or `swift
#                scripts/wb-s16/ui.swift`)
set -u
UI="$1"; PNG="$2"; CMDS="${3:-}"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
. "$HERE/../wb-lib/app.sh"
LOCK="$HOME/wb-bench/gui.lock"
if [ "${WB_S19A_LOCKED:-}" != 1 ]; then
  WB_S19A_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/page.sh" "$@"
fi
pid=""
for p in $(pgrep -f "$WB_BIN_RE --start-hidden"); do
  if [ -z "$pid" ] && lsof -a -p "$p" -d cwd -Fn 2>/dev/null | grep -qx "n$APP_DIR"; then pid=$p; fi
done
[ -n "$pid" ] || { echo "NO APP"; exit 3; }
# A second launch shows the running instance's window and exits.
wb_cli "$APP_DIR"
sleep 2
"$UI" "$pid" "front; wait 600${CMDS:+; $CMDS}" || exit $?
sleep 0.8
wid=$(swift "$HERE/../wb-window-id.swift" "$pid" 2>/dev/null | cut -d' ' -f1)
[ -n "$wid" ] || { echo "NO WINDOW"; exit 4; }
screencapture -x -o -l "$wid" "$PNG" && echo "shot $PNG (window $wid)"
